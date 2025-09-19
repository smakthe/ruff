use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::events::{AppEvent, EventBus, SubscriptionHandle};
use crate::plugin::{
    registry::CommandRegistry,
    security::{PluginSandbox, SecurityPolicy},
    slash_commands::SlashCommandRegistry,
    traits::{Plugin, PluginContext},
    ui_extensions::UIExtensionManager,
    PluginConfig, PluginId, PluginMetadata, PluginStatus,
};
use crate::RuffError;

/// Manages all plugins in the application
pub struct PluginManager {
    /// Loaded plugins
    plugins: Arc<RwLock<HashMap<PluginId, PluginInstance>>>,
    /// Plugin configurations
    configs: HashMap<PluginId, PluginConfig>,
    /// Command registry for plugin commands
    command_registry: Arc<RwLock<CommandRegistry>>,
    /// Slash command registry for plugin slash commands
    slash_command_registry: Arc<RwLock<SlashCommandRegistry>>,
    /// UI extension manager for plugin UI components
    ui_extension_manager: Arc<UIExtensionManager>,
    /// Event bus for plugin communication
    event_bus: Arc<EventBus>,
    /// Security policy for plugins
    security_policy: SecurityPolicy,
    /// Plugin directory path
    plugin_dir: PathBuf,
    /// Event subscription handles
    event_handles: HashMap<PluginId, SubscriptionHandle>,
}

/// Instance of a loaded plugin
struct PluginInstance {
    plugin: Box<dyn Plugin>,
    metadata: PluginMetadata,
    status: PluginStatus,
    sandbox: PluginSandbox,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(event_bus: Arc<EventBus>, plugin_dir: PathBuf) -> Self {
        let ui_extension_manager = Arc::new(UIExtensionManager::new(Arc::clone(&event_bus)));
        
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            configs: HashMap::new(),
            command_registry: Arc::new(RwLock::new(CommandRegistry::new())),
            slash_command_registry: Arc::new(RwLock::new(SlashCommandRegistry::new())),
            ui_extension_manager,
            event_bus,
            security_policy: SecurityPolicy::default(),
            plugin_dir,
            event_handles: HashMap::new(),
        }
    }

    /// Set the security policy for plugins
    pub fn set_security_policy(&mut self, policy: SecurityPolicy) {
        self.security_policy = policy;
    }

    /// Load a plugin from a file or directory
    pub async fn load_plugin(
        &mut self,
        plugin_id: PluginId,
        mut plugin: Box<dyn Plugin>,
    ) -> Result<(), RuffError> {
        // Get plugin metadata
        let metadata = plugin.metadata().clone();

        // Validate plugin ID matches
        if metadata.id != plugin_id {
            return Err(RuffError::Plugin {
                plugin_name: plugin_id,
                message: "Plugin ID mismatch".to_string(),
            });
        }

        // Get or create plugin configuration
        let mut config = self.configs.get(&plugin_id).cloned().unwrap_or_default();

        // Check if plugin is enabled
        if !config.enabled {
            return Err(RuffError::Plugin {
                plugin_name: plugin_id,
                message: "Plugin is disabled".to_string(),
            });
        }

        // If no permissions are configured, grant the requested permissions
        if config.permissions.is_empty() {
            config.permissions = metadata.permissions.clone();
        }

        // Create sandbox with security policy
        let sandbox = PluginSandbox::new(
            plugin_id.clone(),
            self.security_policy.clone(),
            config.permissions.clone(),
        );

        // Validate metadata against sandbox
        sandbox.validate_metadata(&metadata)?;

        // Create plugin context
        let context = PluginContext::new(
            plugin_id.clone(),
            Arc::clone(&self.event_bus),
            config.permissions.clone(),
            config.settings.clone(),
        );

        // Initialize the plugin
        plugin.initialize(context).await.map_err(|e| RuffError::Plugin {
            plugin_name: plugin_id.clone(),
            message: format!("Plugin initialization failed: {}", e),
        })?;

        // Register UI extensions if the plugin provides them
        let ui_extensions = plugin.get_ui_extensions();
        for extension in ui_extensions {
            if let Err(e) = self.ui_extension_manager.register_extension(plugin_id.clone(), extension).await {
                eprintln!("Failed to register UI extension from plugin {}: {}", plugin_id, e);
            }
        }

        // Register slash commands if the plugin provides them
        let slash_commands = plugin.get_slash_commands();
        if !slash_commands.is_empty() {
            let mut slash_registry = self.slash_command_registry.write().await;
            for command in slash_commands {
                if let Err(e) = slash_registry.register_command(plugin_id.clone(), command) {
                    eprintln!("Failed to register slash command from plugin {}: {}", plugin_id, e);
                }
            }
        }

        // Create plugin instance
        let instance = PluginInstance {
            plugin,
            metadata: metadata.clone(),
            status: PluginStatus::Loaded,
            sandbox,
        };

        // Subscribe to events
        let plugin_id_clone = plugin_id.clone();
        let plugins_ref = Arc::clone(&self.plugins);
        let handle = self.event_bus.subscribe_async(move |event| {
            let plugin_id = plugin_id_clone.clone();
            let plugins = Arc::clone(&plugins_ref);
            
            async move {
                let plugins_guard = plugins.read().await;
                if let Some(instance) = plugins_guard.get(&plugin_id) {
                    if let Err(e) = instance.plugin.handle_event(&event).await {
                        eprintln!("Plugin {} event handler error: {}", plugin_id, e);
                    }
                }
                Ok(())
            }
        }).await;

        // Store the plugin
        {
            let mut plugins = self.plugins.write().await;
            plugins.insert(plugin_id.clone(), instance);
        }

        // Store event handle
        self.event_handles.insert(plugin_id.clone(), handle);

        // Update the config in the manager
        self.configs.insert(plugin_id.clone(), config);

        // Publish plugin loaded event
        self.event_bus.publish(AppEvent::PluginLoaded(plugin_id.clone())).await
            .map_err(|e| RuffError::Plugin {
                plugin_name: plugin_id.clone(),
                message: format!("Failed to publish plugin loaded event: {}", e),
            })?;

        Ok(())
    }

    /// Unload a plugin
    pub async fn unload_plugin(&mut self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Remove from plugins map
        let instance = {
            let mut plugins = self.plugins.write().await;
            plugins.remove(plugin_id)
        };

        if let Some(mut instance) = instance {
            // Shutdown the plugin
            if let Err(e) = instance.plugin.shutdown().await {
                eprintln!("Plugin {} shutdown error: {}", plugin_id, e);
            }

            // Unregister slash commands
            {
                let mut slash_registry = self.slash_command_registry.write().await;
                slash_registry.unregister_plugin_commands(plugin_id);
            }

            // Unregister UI extensions
            if let Err(e) = self.ui_extension_manager.unregister_plugin_extensions(plugin_id).await {
                eprintln!("Failed to unregister UI extensions for plugin {}: {}", plugin_id, e);
            }

            // Update status
            instance.status = PluginStatus::Unloaded;

            // Unsubscribe from events
            if let Some(handle) = self.event_handles.remove(plugin_id) {
                self.event_bus.unsubscribe(handle).await;
            }

            // Publish plugin unloaded event
            self.event_bus.publish(AppEvent::PluginUnloaded(plugin_id.clone())).await
                .map_err(|e| RuffError::Plugin {
                    plugin_name: plugin_id.clone(),
                    message: format!("Failed to publish plugin unloaded event: {}", e),
                })?;

            Ok(())
        } else {
            Err(RuffError::Plugin {
                plugin_name: plugin_id.clone(),
                message: "Plugin not found".to_string(),
            })
        }
    }

    /// Reload a plugin
    pub async fn reload_plugin(&mut self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Get the current plugin to reload
        let plugin = {
            let plugins = self.plugins.read().await;
            plugins.get(plugin_id).map(|instance| instance.metadata.clone())
        };

        if plugin.is_some() {
            // Unload the plugin first
            self.unload_plugin(plugin_id).await?;
            
            // TODO: In a real implementation, we would reload the plugin from disk
            // For now, we just return an error indicating this needs to be implemented
            Err(RuffError::Plugin {
                plugin_name: plugin_id.clone(),
                message: "Plugin reloading from disk not yet implemented".to_string(),
            })
        } else {
            Err(RuffError::Plugin {
                plugin_name: plugin_id.clone(),
                message: "Plugin not found".to_string(),
            })
        }
    }

    /// Get a list of all loaded plugins
    pub async fn get_loaded_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.values().map(|instance| instance.metadata.clone()).collect()
    }

    /// Get plugin status
    pub async fn get_plugin_status(&self, plugin_id: &PluginId) -> Option<PluginStatus> {
        let plugins = self.plugins.read().await;
        plugins.get(plugin_id).map(|instance| instance.status.clone())
    }

    /// Check if a plugin is loaded
    pub async fn is_plugin_loaded(&self, plugin_id: &PluginId) -> bool {
        let plugins = self.plugins.read().await;
        plugins.contains_key(plugin_id)
    }

    /// Execute a plugin command
    pub async fn execute_plugin_command(
        &self,
        plugin_id: &PluginId,
        command: &str,
        args: &[String],
    ) -> Result<crate::plugin::traits::CommandResult, RuffError> {
        let plugins = self.plugins.read().await;
        
        if let Some(instance) = plugins.get(plugin_id) {
            instance.plugin.handle_command(command, args).await
                .map_err(|e| RuffError::Plugin {
                    plugin_name: plugin_id.clone(),
                    message: format!("Command execution failed: {}", e),
                })
        } else {
            Err(RuffError::Plugin {
                plugin_name: plugin_id.clone(),
                message: "Plugin not found".to_string(),
            })
        }
    }

    /// Get the UI extension manager
    pub fn get_ui_extension_manager(&self) -> Arc<UIExtensionManager> {
        Arc::clone(&self.ui_extension_manager)
    }

    /// Get the command registry
    pub fn get_command_registry(&self) -> Arc<RwLock<CommandRegistry>> {
        Arc::clone(&self.command_registry)
    }

    /// Get the slash command registry
    pub fn get_slash_command_registry(&self) -> Arc<RwLock<SlashCommandRegistry>> {
        Arc::clone(&self.slash_command_registry)
    }

    /// Execute a slash command
    pub async fn execute_slash_command(
        &self,
        input: &str,
        context: &PluginContext,
    ) -> Result<crate::plugin::traits::CommandResult, RuffError> {
        let slash_registry = self.slash_command_registry.read().await;
        slash_registry.execute_command(input, context).await
            .map_err(|e| RuffError::Plugin {
                plugin_name: context.plugin_id.clone(),
                message: format!("Slash command execution failed: {}", e),
            })
    }

    /// Set plugin configuration
    pub fn set_plugin_config(&mut self, plugin_id: PluginId, config: PluginConfig) {
        self.configs.insert(plugin_id, config);
    }

    /// Get plugin configuration
    pub fn get_plugin_config(&self, plugin_id: &PluginId) -> Option<&PluginConfig> {
        self.configs.get(plugin_id)
    }

    /// Enable a plugin
    pub async fn enable_plugin(&mut self, plugin_id: &PluginId) -> Result<(), RuffError> {
        if let Some(config) = self.configs.get_mut(plugin_id) {
            config.enabled = true;
            Ok(())
        } else {
            // Create default config and enable
            let mut config = PluginConfig::default();
            config.enabled = true;
            self.configs.insert(plugin_id.clone(), config);
            Ok(())
        }
    }

    /// Disable a plugin
    pub async fn disable_plugin(&mut self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Unload if currently loaded
        if self.is_plugin_loaded(plugin_id).await {
            self.unload_plugin(plugin_id).await?;
        }

        // Update config
        if let Some(config) = self.configs.get_mut(plugin_id) {
            config.enabled = false;
        } else {
            let mut config = PluginConfig::default();
            config.enabled = false;
            self.configs.insert(plugin_id.clone(), config);
        }

        Ok(())
    }

    /// Run health checks on all loaded plugins
    pub async fn health_check(&self) -> HashMap<PluginId, bool> {
        let plugins = self.plugins.read().await;
        let mut results = HashMap::new();

        for (plugin_id, instance) in plugins.iter() {
            let is_healthy = instance.plugin.health_check().await.unwrap_or(false);
            results.insert(plugin_id.clone(), is_healthy);
        }

        results
    }

    /// Get plugin statistics
    pub async fn get_stats(&self) -> PluginManagerStats {
        let plugins = self.plugins.read().await;
        let command_registry = self.command_registry.read().await;
        
        let mut loaded_count = 0;
        let mut error_count = 0;
        let mut disabled_count = 0;

        for instance in plugins.values() {
            match instance.status {
                PluginStatus::Loaded => loaded_count += 1,
                PluginStatus::Error(_) => error_count += 1,
                PluginStatus::Disabled => disabled_count += 1,
                _ => {}
            }
        }

        let slash_command_registry = self.slash_command_registry.read().await;
        
        PluginManagerStats {
            total_plugins: self.configs.len(),
            loaded_plugins: loaded_count,
            error_plugins: error_count,
            disabled_plugins: disabled_count,
            command_registry_stats: command_registry.get_stats(),
            slash_command_registry_stats: slash_command_registry.get_stats(),
        }
    }

    /// Shutdown all plugins
    pub async fn shutdown(&mut self) -> Result<(), RuffError> {
        let plugin_ids: Vec<PluginId> = {
            let plugins = self.plugins.read().await;
            plugins.keys().cloned().collect()
        };

        for plugin_id in plugin_ids {
            if let Err(e) = self.unload_plugin(&plugin_id).await {
                eprintln!("Error unloading plugin {}: {}", plugin_id, e);
            }
        }

        Ok(())
    }
}

/// Statistics about the plugin manager
#[derive(Debug, Clone)]
pub struct PluginManagerStats {
    pub total_plugins: usize,
    pub loaded_plugins: usize,
    pub error_plugins: usize,
    pub disabled_plugins: usize,
    pub command_registry_stats: crate::plugin::registry::CommandRegistryStats,
    pub slash_command_registry_stats: crate::plugin::slash_commands::SlashCommandRegistryStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::traits::{CommandResult, PluginResult};
    use crate::plugin_metadata;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestPlugin {
        metadata: PluginMetadata,
        initialized: Arc<AtomicBool>,
        shutdown: Arc<AtomicBool>,
    }

    impl TestPlugin {
        fn new(id: &str) -> Self {
            Self {
                metadata: plugin_metadata! {
                    id: id,
                    name: "Test Plugin",
                    version: "1.0.0",
                    description: "A test plugin",
                    author: "Test Author",
                    permissions: [ReadSessions, Network],
                },
                initialized: Arc::new(AtomicBool::new(false)),
                shutdown: Arc::new(AtomicBool::new(false)),
            }
        }

        fn is_initialized(&self) -> bool {
            self.initialized.load(Ordering::SeqCst)
        }

        fn is_shutdown(&self) -> bool {
            self.shutdown.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.metadata
        }

        async fn initialize(&mut self, _context: PluginContext) -> PluginResult<()> {
            self.initialized.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn shutdown(&mut self) -> PluginResult<()> {
            self.shutdown.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn handle_command(&self, command: &str, args: &[String]) -> PluginResult<CommandResult> {
            Ok(CommandResult::success_with_message(format!(
                "Test command '{}' executed with args: {:?}",
                command, args
            )))
        }

        fn get_slash_commands(&self) -> Vec<crate::plugin::traits::SlashCommand> {
            vec![]
        }
    }

    #[tokio::test]
    async fn test_plugin_loading() {
        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = PathBuf::from("/tmp/plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let plugin = TestPlugin::new("test-plugin");
        let plugin_id = plugin.metadata.id.clone();
        
        // Load the plugin
        let result = manager.load_plugin(plugin_id.clone(), Box::new(plugin)).await;
        assert!(result.is_ok());

        // Check if plugin is loaded
        assert!(manager.is_plugin_loaded(&plugin_id).await);

        // Check plugin status
        let status = manager.get_plugin_status(&plugin_id).await;
        assert_eq!(status, Some(PluginStatus::Loaded));
    }

    #[tokio::test]
    async fn test_plugin_unloading() {
        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = PathBuf::from("/tmp/plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let plugin = TestPlugin::new("test-plugin");
        let plugin_id = plugin.metadata.id.clone();
        
        // Load and then unload the plugin
        manager.load_plugin(plugin_id.clone(), Box::new(plugin)).await.unwrap();
        let result = manager.unload_plugin(&plugin_id).await;
        assert!(result.is_ok());

        // Check if plugin is unloaded
        assert!(!manager.is_plugin_loaded(&plugin_id).await);
    }

    #[tokio::test]
    async fn test_plugin_command_execution() {
        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = PathBuf::from("/tmp/plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let plugin = TestPlugin::new("test-plugin");
        let plugin_id = plugin.metadata.id.clone();
        
        // Load the plugin
        manager.load_plugin(plugin_id.clone(), Box::new(plugin)).await.unwrap();

        // Execute a command
        let result = manager.execute_plugin_command(
            &plugin_id,
            "test-command",
            &["arg1".to_string(), "arg2".to_string()]
        ).await;

        assert!(result.is_ok());
        let command_result = result.unwrap();
        assert!(command_result.success);
        assert!(command_result.message.is_some());
    }

    #[tokio::test]
    async fn test_plugin_configuration() {
        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = PathBuf::from("/tmp/plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let plugin_id = "test-plugin".to_string();
        let mut config = PluginConfig::default();
        config.enabled = false;

        // Set plugin configuration
        manager.set_plugin_config(plugin_id.clone(), config);

        // Get plugin configuration
        let retrieved_config = manager.get_plugin_config(&plugin_id);
        assert!(retrieved_config.is_some());
        assert!(!retrieved_config.unwrap().enabled);
    }

    #[tokio::test]
    async fn test_plugin_enable_disable() {
        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = PathBuf::from("/tmp/plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let plugin_id = "test-plugin".to_string();

        // Enable plugin
        manager.enable_plugin(&plugin_id).await.unwrap();
        let config = manager.get_plugin_config(&plugin_id).unwrap();
        assert!(config.enabled);

        // Disable plugin
        manager.disable_plugin(&plugin_id).await.unwrap();
        let config = manager.get_plugin_config(&plugin_id).unwrap();
        assert!(!config.enabled);
    }

    #[tokio::test]
    async fn test_plugin_health_check() {
        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = PathBuf::from("/tmp/plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let plugin = TestPlugin::new("test-plugin");
        let plugin_id = plugin.metadata.id.clone();
        
        // Load the plugin
        manager.load_plugin(plugin_id.clone(), Box::new(plugin)).await.unwrap();

        // Run health check
        let health_results = manager.health_check().await;
        assert_eq!(health_results.get(&plugin_id), Some(&true));
    }

    #[tokio::test]
    async fn test_plugin_stats() {
        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = PathBuf::from("/tmp/plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let plugin = TestPlugin::new("test-plugin");
        let plugin_id = plugin.metadata.id.clone();
        
        // Load the plugin
        manager.load_plugin(plugin_id.clone(), Box::new(plugin)).await.unwrap();

        // Get stats
        let stats = manager.get_stats().await;
        assert_eq!(stats.loaded_plugins, 1);
        assert_eq!(stats.total_plugins, 1);
    }

    #[tokio::test]
    async fn test_plugin_shutdown() {
        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = PathBuf::from("/tmp/plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let plugin = TestPlugin::new("test-plugin");
        let plugin_id = plugin.metadata.id.clone();
        
        // Load the plugin
        manager.load_plugin(plugin_id.clone(), Box::new(plugin)).await.unwrap();

        // Shutdown all plugins
        let result = manager.shutdown().await;
        assert!(result.is_ok());

        // Check if plugin is unloaded
        assert!(!manager.is_plugin_loaded(&plugin_id).await);
    }
}

impl PluginManager {
    /// Create a new plugin manager with default configuration
    pub async fn new_default() -> Result<Self, RuffError> {
        let plugin_dir = dirs::data_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("ruff")
            .join("plugins");
        
        let event_bus = Arc::new(EventBus::new());
        Ok(Self::new(event_bus, plugin_dir))
    }

    /// List all plugins with optional filtering
    pub async fn list_plugins(&self, enabled_only: bool) -> Result<Vec<PluginListItem>, RuffError> {
        let plugins = self.plugins.read().await;
        let mut items = Vec::new();
        
        for (_id, instance) in plugins.iter() {
            if enabled_only && !matches!(instance.status, PluginStatus::Loaded) {
                continue;
            }
            
            items.push(PluginListItem {
                metadata: instance.metadata.clone(),
                status: instance.status.clone(),
            });
        }
        
        Ok(items)
    }

    /// Install a plugin from source
    pub async fn install_plugin(&mut self, _source: &str, _force: bool) -> Result<PluginInstallResult, RuffError> {
        // This would download/copy the plugin and install it
        // For now, return a placeholder result
        Ok(PluginInstallResult {
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
        })
    }

    /// Uninstall a plugin
    pub async fn uninstall_plugin(&mut self, _plugin_id: &str) -> Result<(), RuffError> {
        // This would remove the plugin from the system
        // For now, just return success
        Ok(())
    }

    /// Enable a plugin
    pub async fn enable_plugin_cli(&mut self, _plugin_id: &str) -> Result<(), RuffError> {
        // This would enable the plugin in configuration
        // For now, just return success
        Ok(())
    }

    /// Disable a plugin
    pub async fn disable_plugin_cli(&mut self, _plugin_id: &str) -> Result<(), RuffError> {
        // This would disable the plugin in configuration
        // For now, just return success
        Ok(())
    }

    /// Get plugin information
    pub async fn get_plugin_info(&self, plugin_id: &str) -> Result<PluginInfo, RuffError> {
        let plugins = self.plugins.read().await;
        let instance = plugins.get(plugin_id)
            .ok_or_else(|| RuffError::Plugin {
                plugin_name: plugin_id.to_string(),
                message: "Plugin not found".to_string(),
            })?;
        
        Ok(PluginInfo {
            metadata: instance.metadata.clone(),
            status: instance.status.clone(),
        })
    }

    /// Update a plugin
    pub async fn update_plugin(&mut self, plugin_id: &str) -> Result<PluginUpdateResult, RuffError> {
        // This would update the plugin to the latest version
        // For now, return a placeholder result
        Ok(PluginUpdateResult {
            plugin_id: plugin_id.to_string(),
            old_version: "1.0.0".to_string(),
            new_version: "1.1.0".to_string(),
        })
    }

    /// Update all plugins
    pub async fn update_all_plugins(&mut self) -> Result<Vec<PluginUpdateResult>, RuffError> {
        // This would update all plugins
        // For now, return empty results
        Ok(vec![])
    }

    /// Validate a plugin
    pub async fn validate_plugin(&self, _target: &str) -> Result<PluginValidationResult, RuffError> {
        // This would validate the plugin file/directory
        // For now, return a successful validation
        Ok(PluginValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        })
    }
}

/// Plugin list item for CLI display
#[derive(Debug, Clone)]
pub struct PluginListItem {
    pub metadata: PluginMetadata,
    pub status: PluginStatus,
}

/// Plugin install result for CLI
#[derive(Debug, Clone)]
pub struct PluginInstallResult {
    pub name: String,
    pub version: String,
}

/// Plugin information for CLI
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub metadata: PluginMetadata,
    pub status: PluginStatus,
}

/// Plugin update result for CLI
#[derive(Debug, Clone)]
pub struct PluginUpdateResult {
    pub plugin_id: String,
    pub old_version: String,
    pub new_version: String,
}

/// Plugin validation result for CLI
#[derive(Debug, Clone)]
pub struct PluginValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}