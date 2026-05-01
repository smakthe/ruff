use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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
use crate::EnhancedError;

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
    #[allow(dead_code)] // Enforces permissions during load; retained for future runtime checks.
    sandbox: PluginSandbox,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct InstalledPluginState {
    enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct InstalledPluginStateFile {
    plugins: HashMap<PluginId, InstalledPluginState>,
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
    ) -> Result<(), EnhancedError> {
        // Get plugin metadata
        let metadata = plugin.metadata().clone();

        // Validate plugin ID matches
        if metadata.id != plugin_id {
            return Err(EnhancedError::unknown("Plugin".to_string()));
        }

        // Get or create plugin configuration
        let mut config = self.configs.get(&plugin_id).cloned().unwrap_or_default();

        // Check if plugin is enabled
        if !config.enabled {
            return Err(EnhancedError::unknown("Plugin".to_string()));
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
        plugin
            .initialize(context)
            .await
            .map_err(|e| EnhancedError::plugin(format!("Plugin initialization failed: {}", e)))?;

        // Register UI extensions if the plugin provides them
        let ui_extensions = plugin.get_ui_extensions();
        for extension in ui_extensions {
            if let Err(e) = self
                .ui_extension_manager
                .register_extension(plugin_id.clone(), extension)
                .await
            {
                eprintln!(
                    "Failed to register UI extension from plugin {}: {}",
                    plugin_id, e
                );
            }
        }

        // Register slash commands if the plugin provides them
        let slash_commands = plugin.get_slash_commands();
        if !slash_commands.is_empty() {
            let mut slash_registry = self.slash_command_registry.write().await;
            for command in slash_commands {
                if let Err(e) = slash_registry.register_command(plugin_id.clone(), command) {
                    eprintln!(
                        "Failed to register slash command from plugin {}: {}",
                        plugin_id, e
                    );
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
        let handle = self
            .event_bus
            .subscribe_async(move |event| {
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
            })
            .await;

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
        self.event_bus
            .publish(AppEvent::PluginLoaded(plugin_id.clone()))
            .await
            .map_err(|e| {
                EnhancedError::plugin(format!("Failed to publish plugin loaded event: {}", e))
            })?;

        Ok(())
    }

    /// Unload a plugin
    pub async fn unload_plugin(&mut self, plugin_id: &PluginId) -> Result<(), EnhancedError> {
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
            if let Err(e) = self
                .ui_extension_manager
                .unregister_plugin_extensions(plugin_id)
                .await
            {
                eprintln!(
                    "Failed to unregister UI extensions for plugin {}: {}",
                    plugin_id, e
                );
            }

            // Update status
            instance.status = PluginStatus::Unloaded;

            // Unsubscribe from events
            if let Some(handle) = self.event_handles.remove(plugin_id) {
                self.event_bus.unsubscribe(handle).await;
            }

            // Publish plugin unloaded event
            self.event_bus
                .publish(AppEvent::PluginUnloaded(plugin_id.clone()))
                .await
                .map_err(|e| {
                    EnhancedError::plugin(format!("Failed to publish plugin unloaded event: {}", e))
                })?;

            Ok(())
        } else {
            Err(EnhancedError::plugin(format!(
                "Plugin not found: {}",
                plugin_id
            )))
        }
    }

    /// Reload a plugin
    pub async fn reload_plugin(&mut self, plugin_id: &PluginId) -> Result<(), EnhancedError> {
        // Get the current plugin to reload
        let plugin = {
            let plugins = self.plugins.read().await;
            plugins
                .get(plugin_id)
                .map(|instance| instance.metadata.clone())
        };

        if plugin.is_some() {
            // Unload the plugin first
            self.unload_plugin(plugin_id).await?;

            Err(EnhancedError::plugin(format!(
                "Plugin '{}' was unloaded, but runtime reload requires a plugin factory or dynamic plugin runtime",
                plugin_id
            )))
        } else {
            Err(EnhancedError::plugin(format!(
                "Plugin not found: {}",
                plugin_id
            )))
        }
    }

    /// Get a list of all loaded plugins
    pub async fn get_loaded_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .map(|instance| instance.metadata.clone())
            .collect()
    }

    /// Get plugin status
    pub async fn get_plugin_status(&self, plugin_id: &PluginId) -> Option<PluginStatus> {
        let plugins = self.plugins.read().await;
        plugins
            .get(plugin_id)
            .map(|instance| instance.status.clone())
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
    ) -> Result<crate::plugin::traits::CommandResult, EnhancedError> {
        let plugins = self.plugins.read().await;

        if let Some(instance) = plugins.get(plugin_id) {
            instance
                .plugin
                .handle_command(command, args)
                .await
                .map_err(|e| {
                    EnhancedError::plugin(format!("Plugin command execution failed: {}", e))
                })
        } else {
            Err(EnhancedError::unknown("Plugin".to_string()))
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
    ) -> Result<crate::plugin::traits::CommandResult, EnhancedError> {
        let slash_registry = self.slash_command_registry.read().await;
        slash_registry
            .execute_command(input, context)
            .await
            .map_err(|e| EnhancedError::plugin(format!("Slash command execution failed: {}", e)))
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
    pub async fn enable_plugin(&mut self, plugin_id: &PluginId) -> Result<(), EnhancedError> {
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
    pub async fn disable_plugin(&mut self, plugin_id: &PluginId) -> Result<(), EnhancedError> {
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
    pub async fn shutdown(&mut self) -> Result<(), EnhancedError> {
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

        async fn handle_command(
            &self,
            command: &str,
            args: &[String],
        ) -> PluginResult<CommandResult> {
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
        let result = manager
            .load_plugin(plugin_id.clone(), Box::new(plugin))
            .await;
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
        manager
            .load_plugin(plugin_id.clone(), Box::new(plugin))
            .await
            .unwrap();
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
        manager
            .load_plugin(plugin_id.clone(), Box::new(plugin))
            .await
            .unwrap();

        // Execute a command
        let result = manager
            .execute_plugin_command(
                &plugin_id,
                "test-command",
                &["arg1".to_string(), "arg2".to_string()],
            )
            .await;

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
        manager
            .load_plugin(plugin_id.clone(), Box::new(plugin))
            .await
            .unwrap();

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
        manager
            .load_plugin(plugin_id.clone(), Box::new(plugin))
            .await
            .unwrap();

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
        manager
            .load_plugin(plugin_id.clone(), Box::new(plugin))
            .await
            .unwrap();

        // Shutdown all plugins
        let result = manager.shutdown().await;
        assert!(result.is_ok());

        // Check if plugin is unloaded
        assert!(!manager.is_plugin_loaded(&plugin_id).await);
    }

    #[tokio::test]
    async fn test_plugin_cli_install_list_disable_enable_uninstall() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let plugin_source = temp_dir.path().join("source-plugin");
        std::fs::create_dir_all(&plugin_source).unwrap();
        std::fs::write(
            plugin_source.join("plugin.json"),
            serde_json::json!({
                "id": "local-plugin",
                "name": "Local Plugin",
                "version": "1.2.3",
                "description": "Local plugin fixture",
                "author": "Test",
                "homepage": null,
                "repository": null,
                "license": null,
                "dependencies": [],
                "permissions": ["ReadSessions"],
                "min_ruff_version": "0.1.0"
            })
            .to_string(),
        )
        .unwrap();

        let event_bus = Arc::new(EventBus::new());
        let plugin_dir = temp_dir.path().join("plugins");
        let mut manager = PluginManager::new(event_bus, plugin_dir);

        let install_result = manager
            .install_plugin(plugin_source.to_str().unwrap(), false)
            .await
            .unwrap();
        assert_eq!(install_result.name, "Local Plugin");
        assert_eq!(install_result.version, "1.2.3");

        let plugins = manager.list_plugins(false).await.unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].metadata.id, "local-plugin");
        assert_eq!(plugins[0].status, PluginStatus::Unloaded);

        manager.disable_plugin_cli("local-plugin").await.unwrap();
        let plugins = manager.list_plugins(false).await.unwrap();
        assert_eq!(plugins[0].status, PluginStatus::Disabled);

        manager.enable_plugin_cli("local-plugin").await.unwrap();
        let plugins = manager.list_plugins(true).await.unwrap();
        assert_eq!(plugins.len(), 1);

        let info = manager.get_plugin_info("local-plugin").await.unwrap();
        assert_eq!(info.metadata.version, "1.2.3");

        let validation = manager.validate_plugin("local-plugin").await.unwrap();
        assert!(validation.is_valid);

        manager.uninstall_plugin("local-plugin").await.unwrap();
        assert!(manager.list_plugins(false).await.unwrap().is_empty());
    }
}

impl PluginManager {
    /// Create a new plugin manager with default configuration
    pub async fn new_default() -> Result<Self, EnhancedError> {
        let plugin_dir = dirs::config_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("ruff")
            .join("plugins");

        let event_bus = Arc::new(EventBus::new());
        Ok(Self::new(event_bus, plugin_dir))
    }

    /// List all plugins with optional filtering
    pub async fn list_plugins(
        &self,
        enabled_only: bool,
    ) -> Result<Vec<PluginListItem>, EnhancedError> {
        let installed_plugins = self.scan_installed_plugins()?;
        if !installed_plugins.is_empty() {
            return Ok(installed_plugins
                .into_iter()
                .filter(|item| !enabled_only || !matches!(item.status, PluginStatus::Disabled))
                .collect());
        }

        let plugins = self.plugins.read().await;
        let mut items = Vec::new();

        for (_id, instance) in plugins.iter() {
            if enabled_only && matches!(instance.status, PluginStatus::Disabled) {
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
    pub async fn install_plugin(
        &mut self,
        source: &str,
        force: bool,
    ) -> Result<PluginInstallResult, EnhancedError> {
        if source.starts_with("http://") || source.starts_with("https://") {
            return Err(EnhancedError::network(
                "URL plugin installation is not supported yet; install from a local plugin directory or manifest file"
            ));
        }

        let source_path = PathBuf::from(source);
        if !source_path.exists() {
            return Err(EnhancedError::storage(format!(
                "Plugin source does not exist: {}",
                source
            )));
        }

        fs::create_dir_all(&self.plugin_dir).map_err(|e| {
            EnhancedError::storage(format!("Failed to create plugin directory: {}", e))
        })?;

        let metadata = self.load_metadata_from_source(&source_path)?;
        let install_path = self.plugin_install_path(&metadata.id);

        if install_path.exists() {
            if !force {
                return Err(EnhancedError::plugin(format!(
                    "Plugin '{}' is already installed. Use --force to overwrite it.",
                    metadata.id
                )));
            }
            if install_path.is_dir() {
                fs::remove_dir_all(&install_path).map_err(|e| {
                    EnhancedError::storage(format!(
                        "Failed to remove existing plugin directory: {}",
                        e
                    ))
                })?;
            } else {
                fs::remove_file(&install_path).map_err(|e| {
                    EnhancedError::storage(format!("Failed to remove existing plugin file: {}", e))
                })?;
            }
        }

        if source_path.is_dir() {
            self.copy_dir_recursive(&source_path, &install_path)?;
        } else {
            fs::create_dir_all(&install_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to create plugin install directory: {}", e))
            })?;
            let file_name = source_path
                .file_name()
                .ok_or_else(|| EnhancedError::storage("Invalid plugin source filename"))?;
            fs::copy(&source_path, install_path.join(file_name)).map_err(|e| {
                EnhancedError::storage(format!("Failed to copy plugin file: {}", e))
            })?;
        }

        let mut state = self.load_installed_state()?;
        state
            .plugins
            .insert(metadata.id.clone(), InstalledPluginState { enabled: true });
        self.save_installed_state(&state)?;

        Ok(PluginInstallResult {
            name: metadata.name,
            version: metadata.version,
        })
    }

    /// Uninstall a plugin
    pub async fn uninstall_plugin(&mut self, plugin_id: &str) -> Result<(), EnhancedError> {
        if self.is_plugin_loaded(&plugin_id.to_string()).await {
            self.unload_plugin(&plugin_id.to_string()).await?;
        }

        let install_path = self.plugin_install_path(plugin_id);
        if !install_path.exists() {
            return Err(EnhancedError::plugin(format!(
                "Plugin not installed: {}",
                plugin_id
            )));
        }

        if install_path.is_dir() {
            fs::remove_dir_all(&install_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to remove plugin directory: {}", e))
            })?;
        } else {
            fs::remove_file(&install_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to remove plugin file: {}", e))
            })?;
        }

        let mut state = self.load_installed_state()?;
        state.plugins.remove(plugin_id);
        self.save_installed_state(&state)?;

        Ok(())
    }

    /// Enable a plugin
    pub async fn enable_plugin_cli(&mut self, plugin_id: &str) -> Result<(), EnhancedError> {
        if !self.plugin_install_path(plugin_id).exists() {
            return Err(EnhancedError::plugin(format!(
                "Plugin not installed: {}",
                plugin_id
            )));
        }

        let mut state = self.load_installed_state()?;
        state.plugins.insert(
            plugin_id.to_string(),
            InstalledPluginState { enabled: true },
        );
        self.save_installed_state(&state)?;
        self.enable_plugin(&plugin_id.to_string()).await?;

        Ok(())
    }

    /// Disable a plugin
    pub async fn disable_plugin_cli(&mut self, plugin_id: &str) -> Result<(), EnhancedError> {
        if !self.plugin_install_path(plugin_id).exists() {
            return Err(EnhancedError::plugin(format!(
                "Plugin not installed: {}",
                plugin_id
            )));
        }

        let mut state = self.load_installed_state()?;
        state.plugins.insert(
            plugin_id.to_string(),
            InstalledPluginState { enabled: false },
        );
        self.save_installed_state(&state)?;
        self.disable_plugin(&plugin_id.to_string()).await?;

        Ok(())
    }

    /// Get plugin information
    pub async fn get_plugin_info(&self, plugin_id: &str) -> Result<PluginInfo, EnhancedError> {
        let plugins = self.plugins.read().await;
        if let Some(instance) = plugins.get(plugin_id) {
            return Ok(PluginInfo {
                metadata: instance.metadata.clone(),
                status: instance.status.clone(),
            });
        }

        let installed_plugin = self
            .scan_installed_plugins()?
            .into_iter()
            .find(|item| item.metadata.id == plugin_id)
            .ok_or_else(|| EnhancedError::plugin(format!("Plugin not installed: {}", plugin_id)))?;

        Ok(PluginInfo {
            metadata: installed_plugin.metadata,
            status: installed_plugin.status,
        })
    }

    /// Update a plugin
    pub async fn update_plugin(
        &mut self,
        plugin_id: &str,
    ) -> Result<PluginUpdateResult, EnhancedError> {
        let plugin = self.get_plugin_info(plugin_id).await?;
        Ok(PluginUpdateResult {
            plugin_id: plugin_id.to_string(),
            old_version: plugin.metadata.version.clone(),
            new_version: plugin.metadata.version,
        })
    }

    /// Update all plugins
    pub async fn update_all_plugins(&mut self) -> Result<Vec<PluginUpdateResult>, EnhancedError> {
        let plugins = self.scan_installed_plugins()?;
        let mut results = Vec::new();

        for plugin in plugins {
            results.push(self.update_plugin(&plugin.metadata.id).await?);
        }

        Ok(results)
    }

    /// Validate a plugin
    pub async fn validate_plugin(
        &self,
        target: &str,
    ) -> Result<PluginValidationResult, EnhancedError> {
        let target_path = PathBuf::from(target);
        let path = if target_path.exists() {
            target_path
        } else {
            self.plugin_install_path(target)
        };

        if !path.exists() {
            return Ok(PluginValidationResult {
                is_valid: false,
                errors: vec![format!("Plugin target does not exist: {}", target)],
                warnings: vec![],
            });
        }

        let mut warnings = Vec::new();
        let metadata = match self.load_metadata_from_source(&path) {
            Ok(metadata) => metadata,
            Err(e) => {
                return Ok(PluginValidationResult {
                    is_valid: false,
                    errors: vec![e.to_string()],
                    warnings,
                });
            }
        };

        if metadata.permissions.is_empty() {
            warnings.push("Plugin declares no permissions".to_string());
        }

        Ok(PluginValidationResult {
            is_valid: true,
            errors: vec![],
            warnings,
        })
    }

    fn plugin_install_path(&self, plugin_id: &str) -> PathBuf {
        self.plugin_dir.join(plugin_id)
    }

    fn state_file_path(&self) -> PathBuf {
        self.plugin_dir.join("installed_plugins.json")
    }

    fn load_installed_state(&self) -> Result<InstalledPluginStateFile, EnhancedError> {
        let state_path = self.state_file_path();
        if !state_path.exists() {
            return Ok(InstalledPluginStateFile::default());
        }

        let content = fs::read_to_string(&state_path)
            .map_err(|e| EnhancedError::storage(format!("Failed to read plugin state: {}", e)))?;
        serde_json::from_str(&content)
            .map_err(|e| EnhancedError::parsing(format!("Failed to parse plugin state: {}", e)))
    }

    fn save_installed_state(&self, state: &InstalledPluginStateFile) -> Result<(), EnhancedError> {
        fs::create_dir_all(&self.plugin_dir).map_err(|e| {
            EnhancedError::storage(format!("Failed to create plugin directory: {}", e))
        })?;
        let content = serde_json::to_string_pretty(state)?;
        fs::write(self.state_file_path(), content)
            .map_err(|e| EnhancedError::storage(format!("Failed to write plugin state: {}", e)))
    }

    fn scan_installed_plugins(&self) -> Result<Vec<PluginListItem>, EnhancedError> {
        if !self.plugin_dir.exists() {
            return Ok(Vec::new());
        }

        let state = self.load_installed_state()?;
        let mut items = Vec::new();

        for entry in fs::read_dir(&self.plugin_dir).map_err(|e| {
            EnhancedError::storage(format!("Failed to read plugin directory: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                EnhancedError::storage(format!("Failed to read plugin directory entry: {}", e))
            })?;
            let path = entry.path();

            if path.file_name().and_then(|name| name.to_str()) == Some("installed_plugins.json") {
                continue;
            }

            let metadata = self.load_metadata_from_source(&path)?;
            let enabled = state
                .plugins
                .get(&metadata.id)
                .map(|plugin_state| plugin_state.enabled)
                .unwrap_or(true);

            items.push(PluginListItem {
                metadata,
                status: if enabled {
                    PluginStatus::Unloaded
                } else {
                    PluginStatus::Disabled
                },
            });
        }

        items.sort_by(|a, b| a.metadata.id.cmp(&b.metadata.id));
        Ok(items)
    }

    fn load_metadata_from_source(&self, source: &Path) -> Result<PluginMetadata, EnhancedError> {
        let manifest_path = if source.is_dir() {
            ["plugin.json", "ruff-plugin.json", "manifest.json"]
                .iter()
                .map(|name| source.join(name))
                .find(|path| path.exists())
        } else if source.extension().and_then(|ext| ext.to_str()) == Some("json") {
            Some(source.to_path_buf())
        } else {
            None
        };

        if let Some(path) = manifest_path {
            let content = fs::read_to_string(&path).map_err(|e| {
                EnhancedError::storage(format!("Failed to read plugin manifest: {}", e))
            })?;
            return serde_json::from_str(&content).map_err(|e| {
                EnhancedError::parsing(format!(
                    "Failed to parse plugin manifest {}: {}",
                    path.display(),
                    e
                ))
            });
        }

        let id = source
            .file_stem()
            .or_else(|| source.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("plugin")
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '-'
                }
            })
            .collect::<String>();

        Ok(PluginMetadata {
            id: id.clone(),
            name: id,
            version: "0.0.0".to_string(),
            description: "Plugin without manifest metadata".to_string(),
            author: "Unknown".to_string(),
            homepage: None,
            repository: None,
            license: None,
            dependencies: Vec::new(),
            permissions: Vec::new(),
            min_ruff_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    fn copy_dir_recursive(&self, source: &Path, destination: &Path) -> Result<(), EnhancedError> {
        fs::create_dir_all(destination).map_err(|e| {
            EnhancedError::storage(format!("Failed to create plugin destination: {}", e))
        })?;

        for entry in fs::read_dir(source)
            .map_err(|e| EnhancedError::storage(format!("Failed to read plugin source: {}", e)))?
        {
            let entry = entry.map_err(|e| {
                EnhancedError::storage(format!("Failed to read plugin source entry: {}", e))
            })?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());

            if source_path.is_dir() {
                self.copy_dir_recursive(&source_path, &destination_path)?;
            } else {
                fs::copy(&source_path, &destination_path).map_err(|e| {
                    EnhancedError::storage(format!("Failed to copy plugin file: {}", e))
                })?;
            }
        }

        Ok(())
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
