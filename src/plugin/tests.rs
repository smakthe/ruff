use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ratatui::{layout::Rect, Frame};
use serde_json::Value;
use tokio::time::sleep;

use crate::chat::Message;
use crate::events::{AppEvent, EventBus};
use crate::plugin::{
    manager::{PluginManager, PluginManagerStats},
    registry::{CommandRegistry, SlashCommandBuilder},
    security::{PluginSandbox, SecurityPolicy},
    slash_commands::{SlashCommandBuilder as SlashBuilder, SlashCommandRegistry},
    traits::{
        CommandResult, Plugin, PluginContext, PluginResult, SlashCommand, SlashCommandHandler,
        UIExtension, UIPosition,
    },
    Permission, PluginConfig, PluginId, PluginMetadata, PluginStatus,
};
use crate::plugin_metadata;
use crate::EnhancedError;

/// Test plugin implementation
struct TestPlugin {
    metadata: PluginMetadata,
    initialized: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    command_count: Arc<AtomicUsize>,
    event_count: Arc<AtomicUsize>,
    context: Option<PluginContext>,
}

impl TestPlugin {
    fn new(id: &str, permissions: Vec<Permission>) -> Self {
        let mut metadata = plugin_metadata! {
            id: id,
            name: "Test Plugin",
            version: "1.0.0",
            description: "A test plugin for unit testing",
            author: "Test Author",
            homepage: "https://example.com",
            repository: "https://github.com/example/test-plugin",
            license: "MIT",
            dependencies: ["dep1", "dep2"],
            min_ruff_version: "0.1.0",
        };
        metadata.permissions = permissions;

        Self {
            metadata,
            initialized: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(AtomicBool::new(false)),
            command_count: Arc::new(AtomicUsize::new(0)),
            event_count: Arc::new(AtomicUsize::new(0)),
            context: None,
        }
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    fn get_command_count(&self) -> usize {
        self.command_count.load(Ordering::SeqCst)
    }

    fn get_event_count(&self) -> usize {
        self.event_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Plugin for TestPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, context: PluginContext) -> PluginResult<()> {
        self.context = Some(context);
        self.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        self.shutdown.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn handle_command(&self, command: &str, args: &[String]) -> PluginResult<CommandResult> {
        self.command_count.fetch_add(1, Ordering::SeqCst);

        match command {
            "test" => Ok(CommandResult::success_with_message(format!(
                "Test command executed with args: {:?}",
                args
            ))),
            "error" => Ok(CommandResult::error("Test error".to_string())),
            "data" => Ok(CommandResult::success_with_data(
                serde_json::json!({"result": "test data"}),
            )),
            _ => Ok(CommandResult::error(format!(
                "Unknown command: {}",
                command
            ))),
        }
    }

    async fn handle_event(&self, _event: &AppEvent) -> PluginResult<()> {
        self.event_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn health_check(&self) -> PluginResult<bool> {
        Ok(self.is_initialized() && !self.is_shutdown())
    }

    fn get_config_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "setting1": {"type": "string"},
                "setting2": {"type": "number"}
            }
        }))
    }

    fn validate_config(&self, config: &HashMap<String, Value>) -> PluginResult<()> {
        if config.contains_key("invalid") {
            return Err("Invalid configuration".into());
        }
        Ok(())
    }

    fn get_ui_extensions(&self) -> Vec<Box<dyn UIExtension>> {
        vec![Box::new(TestUIExtension::new("test-extension"))]
    }

    fn get_slash_commands(&self) -> Vec<SlashCommand> {
        let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());
        vec![
            SlashBuilder::new("test")
                .description("Test slash command")
                .usage("/test <args>")
                .alias("t")
                .build(Arc::clone(&handler)),
            SlashBuilder::new("hello")
                .description("Hello slash command")
                .usage("/hello [name]")
                .build(handler),
        ]
    }
}

/// Test UI extension
struct TestUIExtension {
    id: String,
    visible: Arc<AtomicBool>,
}

impl TestUIExtension {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            visible: Arc::new(AtomicBool::new(true)),
        }
    }

    fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::SeqCst);
    }
}

impl UIExtension for TestUIExtension {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Test UI Extension"
    }

    fn position(&self) -> UIPosition {
        UIPosition::Bottom
    }

    fn render(&self, _area: Rect, _frame: &mut Frame<'_>) -> Result<(), EnhancedError> {
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }

    fn min_size(&self) -> (u16, u16) {
        (20, 5)
    }
}

/// Test slash command handler
struct TestSlashCommandHandler {
    execution_count: Arc<AtomicUsize>,
}

impl TestSlashCommandHandler {
    fn new() -> Self {
        Self {
            execution_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn get_execution_count(&self) -> usize {
        self.execution_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SlashCommandHandler for TestSlashCommandHandler {
    async fn execute(
        &self,
        args: &[String],
        _context: &PluginContext,
    ) -> PluginResult<CommandResult> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        Ok(CommandResult::success_with_message(format!(
            "Slash command executed with args: {:?}",
            args
        )))
    }
}

fn create_test_context() -> PluginContext {
    let mut config = HashMap::new();
    config.insert(
        "test_setting".to_string(),
        Value::String("test_value".to_string()),
    );

    PluginContext::new(
        "test-plugin".to_string(),
        Arc::new(EventBus::new()),
        vec![Permission::ReadSessions, Permission::Network],
        config,
    )
}

#[tokio::test]
async fn test_plugin_metadata_macro() {
    let metadata = plugin_metadata! {
        id: "test-plugin",
        name: "Test Plugin",
        version: "1.0.0",
        description: "Test description",
        author: "Test Author",
        homepage: "https://example.com",
        repository: "https://github.com/example/test",
        license: "MIT",
        dependencies: ["dep1", "dep2"],
        permissions: [ReadSessions, Network],
        min_ruff_version: "0.1.0",
    };

    assert_eq!(metadata.id, "test-plugin");
    assert_eq!(metadata.name, "Test Plugin");
    assert_eq!(metadata.version, "1.0.0");
    assert_eq!(metadata.description, "Test description");
    assert_eq!(metadata.author, "Test Author");
    assert_eq!(metadata.homepage, Some("https://example.com".to_string()));
    assert_eq!(
        metadata.repository,
        Some("https://github.com/example/test".to_string())
    );
    assert_eq!(metadata.license, Some("MIT".to_string()));
    assert_eq!(metadata.dependencies, vec!["dep1", "dep2"]);
    assert_eq!(
        metadata.permissions,
        vec![Permission::ReadSessions, Permission::Network]
    );
    assert_eq!(metadata.min_ruff_version, "0.1.0");
}

#[tokio::test]
async fn test_plugin_context() {
    let context = create_test_context();

    assert_eq!(context.plugin_id, "test-plugin");
    assert!(context.has_permission(&Permission::ReadSessions));
    assert!(context.has_permission(&Permission::Network));
    assert!(!context.has_permission(&Permission::WriteSessions));

    let setting: Option<String> = context.get_config("test_setting");
    assert_eq!(setting, Some("test_value".to_string()));

    let missing: Option<String> = context.get_config("missing_setting");
    assert_eq!(missing, None);
}

#[tokio::test]
async fn test_plugin_lifecycle() {
    let mut plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);
    let context = create_test_context();

    // Test initialization
    assert!(!plugin.is_initialized());
    plugin.initialize(context).await.unwrap();
    assert!(plugin.is_initialized());

    // Test health check
    let healthy = plugin.health_check().await.unwrap();
    assert!(healthy);

    // Test shutdown
    assert!(!plugin.is_shutdown());
    plugin.shutdown().await.unwrap();
    assert!(plugin.is_shutdown());

    // Health check should fail after shutdown
    let healthy = plugin.health_check().await.unwrap();
    assert!(!healthy);
}

#[tokio::test]
async fn test_plugin_commands() {
    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);

    // Test successful command
    let result = plugin
        .handle_command("test", &["arg1".to_string(), "arg2".to_string()])
        .await
        .unwrap();
    assert!(result.success);
    assert!(result.message.is_some());
    assert_eq!(plugin.get_command_count(), 1);

    // Test error command
    let result = plugin.handle_command("error", &[]).await.unwrap();
    assert!(!result.success);
    assert_eq!(result.message, Some("Test error".to_string()));
    assert_eq!(plugin.get_command_count(), 2);

    // Test data command
    let result = plugin.handle_command("data", &[]).await.unwrap();
    assert!(result.success);
    assert!(result.data.is_some());
    assert_eq!(plugin.get_command_count(), 3);

    // Test unknown command
    let result = plugin.handle_command("unknown", &[]).await.unwrap();
    assert!(!result.success);
    assert_eq!(plugin.get_command_count(), 4);
}

#[tokio::test]
async fn test_plugin_events() {
    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);

    // Test event handling
    let event = AppEvent::SessionCreated(uuid::Uuid::new_v4());
    plugin.handle_event(&event).await.unwrap();
    assert_eq!(plugin.get_event_count(), 1);

    let event = AppEvent::ThemeChanged("dark".to_string());
    plugin.handle_event(&event).await.unwrap();
    assert_eq!(plugin.get_event_count(), 2);
}

#[tokio::test]
async fn test_plugin_configuration() {
    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);

    // Test config schema
    let schema = plugin.get_config_schema();
    assert!(schema.is_some());

    // Test valid config
    let mut config = HashMap::new();
    config.insert("setting1".to_string(), Value::String("value1".to_string()));
    config.insert("setting2".to_string(), Value::Number(42.into()));

    let result = plugin.validate_config(&config);
    assert!(result.is_ok());

    // Test invalid config
    let mut invalid_config = HashMap::new();
    invalid_config.insert("invalid".to_string(), Value::Bool(true));

    let result = plugin.validate_config(&invalid_config);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_ui_extension() {
    let extension = TestUIExtension::new("test-extension");

    assert_eq!(extension.id(), "test-extension");
    assert_eq!(extension.name(), "Test UI Extension");
    assert_eq!(extension.position(), UIPosition::Bottom);
    assert!(extension.is_visible());
    assert_eq!(extension.min_size(), (20, 5));

    // Test visibility toggle
    extension.set_visible(false);
    assert!(!extension.is_visible());
    extension.set_visible(true);
    assert!(extension.is_visible());
}

#[tokio::test]
async fn test_slash_command_handler() {
    let handler = TestSlashCommandHandler::new();
    let context = create_test_context();

    let result = handler
        .execute(&["arg1".to_string()], &context)
        .await
        .unwrap();
    assert!(result.success);
    assert!(result.message.is_some());
    assert_eq!(handler.get_execution_count(), 1);
}

#[tokio::test]
async fn test_slash_command_builder() {
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command = SlashCommandBuilder::new("test")
        .description("Test command")
        .usage("/test <arg>")
        .alias("t")
        .alias("tst")
        .build(handler);

    assert_eq!(command.name, "test");
    assert_eq!(command.description, "Test command");
    assert_eq!(command.usage, "/test <arg>");
    assert_eq!(command.aliases, vec!["t", "tst"]);
}

#[tokio::test]
async fn test_plugin_manager_basic_operations() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);
    let plugin_id = plugin.metadata.id.clone();

    // Test plugin loading
    assert!(!manager.is_plugin_loaded(&plugin_id).await);
    let result = manager
        .load_plugin(plugin_id.clone(), Box::new(plugin))
        .await;
    assert!(result.is_ok());
    assert!(manager.is_plugin_loaded(&plugin_id).await);

    // Test plugin status
    let status = manager.get_plugin_status(&plugin_id).await;
    assert_eq!(status, Some(PluginStatus::Loaded));

    // Test plugin list
    let plugins = manager.get_loaded_plugins().await;
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].id, plugin_id);

    // Test plugin unloading
    let result = manager.unload_plugin(&plugin_id).await;
    assert!(result.is_ok());
    assert!(!manager.is_plugin_loaded(&plugin_id).await);
}

#[tokio::test]
async fn test_plugin_manager_command_execution() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);
    let plugin_id = plugin.metadata.id.clone();

    // Load plugin
    manager
        .load_plugin(plugin_id.clone(), Box::new(plugin))
        .await
        .unwrap();

    // Execute command
    let result = manager
        .execute_plugin_command(
            &plugin_id,
            "test",
            &["arg1".to_string(), "arg2".to_string()],
        )
        .await;

    assert!(result.is_ok());
    let command_result = result.unwrap();
    assert!(command_result.success);
    assert!(command_result.message.is_some());

    // Execute command on non-existent plugin
    let result = manager
        .execute_plugin_command(&"non-existent".to_string(), "test", &[])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_plugin_manager_configuration() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    let plugin_id = "test-plugin".to_string();
    let mut config = PluginConfig::default();
    config.enabled = false;
    config.permissions = vec![Permission::ReadSessions, Permission::Network];

    // Set configuration
    manager.set_plugin_config(plugin_id.clone(), config);

    // Get configuration
    let retrieved_config = manager.get_plugin_config(&plugin_id);
    assert!(retrieved_config.is_some());
    assert!(!retrieved_config.unwrap().enabled);
    assert_eq!(retrieved_config.unwrap().permissions.len(), 2);

    // Test enable/disable
    manager.enable_plugin(&plugin_id).await.unwrap();
    let config = manager.get_plugin_config(&plugin_id).unwrap();
    assert!(config.enabled);

    manager.disable_plugin(&plugin_id).await.unwrap();
    let config = manager.get_plugin_config(&plugin_id).unwrap();
    assert!(!config.enabled);
}

#[tokio::test]
async fn test_plugin_manager_health_check() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);
    let plugin_id = plugin.metadata.id.clone();

    // Load plugin
    manager
        .load_plugin(plugin_id.clone(), Box::new(plugin))
        .await
        .unwrap();

    // Run health check
    let health_results = manager.health_check().await;
    assert_eq!(health_results.len(), 1);
    assert_eq!(health_results.get(&plugin_id), Some(&true));
}

#[tokio::test]
async fn test_plugin_manager_stats() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    // Initial stats
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_plugins, 0);
    assert_eq!(stats.loaded_plugins, 0);

    // Load a plugin
    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);
    let plugin_id = plugin.metadata.id.clone();
    manager
        .load_plugin(plugin_id.clone(), Box::new(plugin))
        .await
        .unwrap();

    // Updated stats
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_plugins, 1);
    assert_eq!(stats.loaded_plugins, 1);
}

#[tokio::test]
async fn test_plugin_manager_ui_extensions() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);
    let plugin_id = plugin.metadata.id.clone();

    // Load plugin
    manager
        .load_plugin(plugin_id.clone(), Box::new(plugin))
        .await
        .unwrap();

    // Get UI extension manager
    let ui_extension_manager = manager.get_ui_extension_manager();
    let extensions = ui_extension_manager.list_extensions().await;
    assert_eq!(extensions.len(), 1);
    assert_eq!(extensions[0].plugin_id, plugin_id);
    assert_eq!(extensions[0].id, "test-extension");
}

#[tokio::test]
async fn test_plugin_manager_shutdown() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    // Load multiple plugins
    let plugin1 = TestPlugin::new("test-plugin-1", vec![Permission::ReadSessions]);
    let plugin1_id = plugin1.metadata.id.clone();
    manager
        .load_plugin(plugin1_id.clone(), Box::new(plugin1))
        .await
        .unwrap();

    let plugin2 = TestPlugin::new("test-plugin-2", vec![Permission::Network]);
    let plugin2_id = plugin2.metadata.id.clone();
    manager
        .load_plugin(plugin2_id.clone(), Box::new(plugin2))
        .await
        .unwrap();

    // Verify plugins are loaded
    assert!(manager.is_plugin_loaded(&plugin1_id).await);
    assert!(manager.is_plugin_loaded(&plugin2_id).await);

    // Shutdown all plugins
    let result = manager.shutdown().await;
    assert!(result.is_ok());

    // Verify plugins are unloaded
    assert!(!manager.is_plugin_loaded(&plugin1_id).await);
    assert!(!manager.is_plugin_loaded(&plugin2_id).await);
}

#[tokio::test]
async fn test_plugin_disabled_loading() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);
    let plugin_id = plugin.metadata.id.clone();

    // Disable the plugin first
    manager.disable_plugin(&plugin_id).await.unwrap();

    // Try to load the disabled plugin
    let result = manager
        .load_plugin(plugin_id.clone(), Box::new(plugin))
        .await;
    assert!(result.is_err());
    assert!(!manager.is_plugin_loaded(&plugin_id).await);
}

#[tokio::test]
async fn test_plugin_id_mismatch() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    let plugin = TestPlugin::new("test-plugin", vec![Permission::ReadSessions]);

    // Try to load with mismatched ID
    let result = manager
        .load_plugin("different-id".to_string(), Box::new(plugin))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_slash_command_registry_basic_operations() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command = SlashBuilder::new("test")
        .description("Test command")
        .usage("/test <args>")
        .alias("t")
        .build(handler);

    // Test registration
    assert!(registry
        .register_command(plugin_id.clone(), command)
        .is_ok());
    assert!(registry.has_command("test"));
    assert!(registry.has_command("t")); // alias

    // Test stats
    let stats = registry.get_stats();
    assert_eq!(stats.total_commands, 1);
    assert_eq!(stats.enabled_commands, 1);
    assert_eq!(stats.total_aliases, 1);
    assert_eq!(stats.plugins_with_commands, 1);

    // Test command names
    let names = registry.get_command_names();
    assert!(names.contains(&"test".to_string()));
    assert!(names.contains(&"t".to_string()));
}

#[tokio::test]
async fn test_slash_command_parsing() {
    let registry = SlashCommandRegistry::new();

    // Valid commands
    let parsed = registry.parse_command_input("/test arg1 arg2").unwrap();
    assert_eq!(parsed.name, "test");
    assert_eq!(parsed.original_name, "test");
    assert_eq!(parsed.args, vec!["arg1", "arg2"]);
    assert_eq!(parsed.raw_input, "test arg1 arg2");

    let parsed = registry.parse_command_input("/hello").unwrap();
    assert_eq!(parsed.name, "hello");
    assert_eq!(parsed.args.len(), 0);

    // Test with extra whitespace
    let parsed = registry
        .parse_command_input("  /test   arg1   arg2  ")
        .unwrap();
    assert_eq!(parsed.name, "test");
    assert_eq!(parsed.args, vec!["arg1", "arg2"]);

    // Invalid commands
    assert!(registry.parse_command_input("test").is_err()); // no slash
    assert!(registry.parse_command_input("/").is_err()); // empty command
    assert!(registry.parse_command_input("").is_err()); // empty input
    assert!(registry.parse_command_input("   ").is_err()); // whitespace only
}

#[tokio::test]
async fn test_slash_command_execution() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command = SlashBuilder::new("test")
        .description("Test command")
        .alias("t")
        .build(handler);

    registry.register_command(plugin_id, command).unwrap();

    let context = PluginContext::new(
        "test-plugin".to_string(),
        Arc::new(EventBus::new()),
        vec![Permission::SlashCommands],
        HashMap::new(),
    );

    // Execute by name
    let result = registry
        .execute_command("/test arg1 arg2", &context)
        .await
        .unwrap();
    assert!(result.success);
    assert!(result.message.is_some());

    // Execute by alias
    let result = registry.execute_command("/t arg1", &context).await.unwrap();
    assert!(result.success);

    // Execute non-existent command
    let result = registry
        .execute_command("/nonexistent", &context)
        .await
        .unwrap();
    assert!(!result.success);
    assert!(result.message.unwrap().contains("not found"));

    // Execute with parse error
    let result = registry.execute_command("invalid", &context).await.unwrap();
    assert!(!result.success);
    assert!(result.message.unwrap().contains("Parse error"));
}

#[tokio::test]
async fn test_slash_command_execution_without_permission() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command = SlashBuilder::new("test")
        .description("Test command")
        .build(handler);

    registry.register_command(plugin_id, command).unwrap();

    // Create context without slash command permission
    let context = PluginContext::new(
        "test-plugin".to_string(),
        Arc::new(EventBus::new()),
        vec![], // No permissions
        HashMap::new(),
    );

    let result = registry.execute_command("/test", &context).await.unwrap();
    assert!(!result.success);
    assert!(result.message.unwrap().contains("permission"));
}

#[tokio::test]
async fn test_slash_command_search() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let commands = vec![
        SlashBuilder::new("help")
            .description("Show help information")
            .alias("h")
            .build(Arc::clone(&handler)),
        SlashBuilder::new("hello")
            .description("Say hello")
            .build(Arc::clone(&handler)),
        SlashBuilder::new("test")
            .description("Run tests")
            .alias("t")
            .build(handler),
    ];

    for command in commands {
        registry
            .register_command(plugin_id.clone(), command)
            .unwrap();
    }

    // Search by name
    let results = registry.search_commands("hel");
    assert_eq!(results.len(), 2); // help and hello

    // Search by description
    let results = registry.search_commands("information");
    assert_eq!(results.len(), 1); // help

    // Search by alias (also matches "hello" because it contains "h")
    let results = registry.search_commands("h");
    assert_eq!(results.len(), 2); // help (has alias "h") and hello (contains "h")

    // Exact match should come first
    let results = registry.search_commands("help");
    assert_eq!(results[0].name, "help");
}

#[tokio::test]
async fn test_slash_command_help() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command = SlashBuilder::new("test")
        .description("Test command")
        .usage("/test <arg1> [arg2]")
        .alias("t")
        .build(handler);

    registry
        .register_command(plugin_id.clone(), command)
        .unwrap();

    let help = registry.get_command_help("test").unwrap();
    assert!(help.contains("/test"));
    assert!(help.contains("Test command"));
    assert!(help.contains("/test <arg1> [arg2]"));
    assert!(help.contains("Aliases: t"));
    assert!(help.contains(&format!("Plugin: {}", plugin_id)));

    // Test help for alias
    let help = registry.get_command_help("t").unwrap();
    assert!(help.contains("/test")); // Should resolve to main command

    // Test help for non-existent command
    let help = registry.get_command_help("nonexistent");
    assert!(help.is_none());
}

#[tokio::test]
async fn test_slash_command_enable_disable() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command = SlashBuilder::new("test")
        .description("Test command")
        .build(handler);

    registry.register_command(plugin_id, command).unwrap();

    // Initially enabled
    assert_eq!(registry.get_commands().len(), 1);

    // Disable command
    assert!(registry.set_command_enabled("test", false));
    assert_eq!(registry.get_commands().len(), 0); // Should not appear in enabled commands

    // Re-enable command
    assert!(registry.set_command_enabled("test", true));
    assert_eq!(registry.get_commands().len(), 1);

    // Try to enable/disable non-existent command
    assert!(!registry.set_command_enabled("nonexistent", false));
}

#[tokio::test]
async fn test_slash_command_unregistration() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command = SlashBuilder::new("test")
        .description("Test command")
        .alias("t")
        .build(handler);

    registry
        .register_command(plugin_id.clone(), command)
        .unwrap();
    assert!(registry.has_command("test"));
    assert!(registry.has_command("t"));

    // Unregister command
    assert!(registry.unregister_command("test"));
    assert!(!registry.has_command("test"));
    assert!(!registry.has_command("t")); // alias should be removed too

    // Try to unregister non-existent command
    assert!(!registry.unregister_command("nonexistent"));
}

#[tokio::test]
async fn test_slash_command_plugin_unregistration() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let commands = vec![
        SlashBuilder::new("cmd1")
            .description("Command 1")
            .build(Arc::clone(&handler)),
        SlashBuilder::new("cmd2")
            .description("Command 2")
            .build(handler),
    ];

    for command in commands {
        registry
            .register_command(plugin_id.clone(), command)
            .unwrap();
    }

    assert_eq!(registry.get_plugin_commands(&plugin_id).len(), 2);

    // Unregister all commands from plugin
    let removed_count = registry.unregister_plugin_commands(&plugin_id);
    assert_eq!(removed_count, 2);
    assert_eq!(registry.get_plugin_commands(&plugin_id).len(), 0);
}

#[tokio::test]
async fn test_slash_command_duplicate_registration() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command1 = SlashBuilder::new("test")
        .description("Test command 1")
        .build(Arc::clone(&handler));

    let command2 = SlashBuilder::new("test")
        .description("Test command 2")
        .build(handler);

    // Register first command
    assert!(registry
        .register_command(plugin_id.clone(), command1)
        .is_ok());

    // Try to register duplicate
    assert!(registry.register_command(plugin_id, command2).is_err());
}

#[tokio::test]
async fn test_slash_command_alias_conflict() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    let command1 = SlashBuilder::new("help")
        .description("Help command")
        .alias("h")
        .build(Arc::clone(&handler));

    let command2 = SlashBuilder::new("hello")
        .description("Hello command")
        .alias("h") // Conflicting alias
        .build(handler);

    // Register first command
    assert!(registry
        .register_command(plugin_id.clone(), command1)
        .is_ok());

    // Try to register command with conflicting alias
    assert!(registry.register_command(plugin_id, command2).is_err());
}

#[tokio::test]
async fn test_plugin_manager_slash_command_integration() {
    let event_bus = Arc::new(EventBus::new());
    let plugin_dir = std::path::PathBuf::from("/tmp/test-plugins");
    let mut manager = PluginManager::new(event_bus, plugin_dir);

    let plugin = TestPlugin::new("test-plugin", vec![Permission::SlashCommands]);
    let plugin_id = plugin.metadata.id.clone();

    // Load plugin (should register slash commands)
    manager
        .load_plugin(plugin_id.clone(), Box::new(plugin))
        .await
        .unwrap();

    // Check that slash commands were registered
    let slash_registry = manager.get_slash_command_registry();
    let registry = slash_registry.read().await;
    let commands = registry.get_plugin_commands(&plugin_id);
    assert_eq!(commands.len(), 2); // test and hello commands

    // Test command execution through manager
    let context = PluginContext::new(
        plugin_id.clone(),
        Arc::new(EventBus::new()),
        vec![Permission::SlashCommands],
        HashMap::new(),
    );

    drop(registry); // Release the read lock

    let result = manager.execute_slash_command("/test arg1", &context).await;
    assert!(result.is_ok());
    let command_result = result.unwrap();
    assert!(command_result.success);

    // Unload plugin (should unregister slash commands)
    manager.unload_plugin(&plugin_id).await.unwrap();

    let registry = slash_registry.read().await;
    let commands = registry.get_plugin_commands(&plugin_id);
    assert_eq!(commands.len(), 0); // Commands should be unregistered
}

#[tokio::test]
async fn test_slash_command_registry_stats() {
    let mut registry = SlashCommandRegistry::new();
    let plugin_id = "test-plugin".to_string();
    let handler: Arc<dyn SlashCommandHandler> = Arc::new(TestSlashCommandHandler::new());

    // Initial stats
    let stats = registry.get_stats();
    assert_eq!(stats.total_commands, 0);
    assert_eq!(stats.enabled_commands, 0);
    assert_eq!(stats.disabled_commands, 0);
    assert_eq!(stats.total_aliases, 0);
    assert_eq!(stats.plugins_with_commands, 0);

    // Register commands
    let commands = vec![
        SlashBuilder::new("cmd1")
            .alias("c1")
            .build(Arc::clone(&handler)),
        SlashBuilder::new("cmd2").alias("c2").build(handler),
    ];

    for command in commands {
        registry
            .register_command(plugin_id.clone(), command)
            .unwrap();
    }

    // Updated stats
    let stats = registry.get_stats();
    assert_eq!(stats.total_commands, 2);
    assert_eq!(stats.enabled_commands, 2);
    assert_eq!(stats.disabled_commands, 0);
    assert_eq!(stats.total_aliases, 2);
    assert_eq!(stats.plugins_with_commands, 1);

    // Disable one command
    registry.set_command_enabled("cmd1", false);
    let stats = registry.get_stats();
    assert_eq!(stats.enabled_commands, 1);
    assert_eq!(stats.disabled_commands, 1);
}

#[test]
fn test_slash_command_handler_macro() {
    use crate::slash_command_handler;

    let command = slash_command_handler!("test", "Test command", |args, _context| {
        Ok(CommandResult::success_with_message(format!(
            "Args: {:?}",
            args
        )))
    });

    assert_eq!(command.name, "test");
    assert_eq!(command.description, "Test command");
    assert_eq!(command.usage, "/test");
}
