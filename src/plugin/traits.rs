use async_trait::async_trait;
use ratatui::{layout::Rect, Frame};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use crate::events::{AppEvent, EventBus};
use crate::chat::Message;
use crate::plugin::{PluginId, PluginMetadata, Permission};
use crate::RuffError;

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Context provided to plugins for interacting with the application
#[derive(Clone)]
pub struct PluginContext {
    pub plugin_id: PluginId,
    pub event_bus: Arc<EventBus>,
    pub permissions: Vec<Permission>,
    pub config: HashMap<String, Value>,
}

impl PluginContext {
    pub fn new(
        plugin_id: PluginId,
        event_bus: Arc<EventBus>,
        permissions: Vec<Permission>,
        config: HashMap<String, Value>,
    ) -> Self {
        Self {
            plugin_id,
            event_bus,
            permissions,
            config,
        }
    }

    /// Check if the plugin has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// Get a configuration value
    pub fn get_config<T>(&self, key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.config.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Publish an event through the event bus
    pub async fn publish_event(&self, event: AppEvent) -> PluginResult<()> {
        self.event_bus.publish(event).await.map_err(|e| e.into())
    }
}

/// Result of a command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<Value>,
}

impl CommandResult {
    pub fn success() -> Self {
        Self {
            success: true,
            message: None,
            data: None,
        }
    }

    pub fn success_with_message(message: String) -> Self {
        Self {
            success: true,
            message: Some(message),
            data: None,
        }
    }

    pub fn success_with_data(data: Value) -> Self {
        Self {
            success: true,
            message: None,
            data: Some(data),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            message: Some(message),
            data: None,
        }
    }
}

/// Main plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> &PluginMetadata;

    /// Initialize the plugin with the given context
    async fn initialize(&mut self, context: PluginContext) -> PluginResult<()>;

    /// Shutdown the plugin and clean up resources
    async fn shutdown(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// Handle a custom command
    async fn handle_command(&self, command: &str, args: &[String]) -> PluginResult<CommandResult> {
        let _ = (command, args);
        Ok(CommandResult::error("Command not supported".to_string()))
    }

    /// Process a message (pre or post processing)
    async fn process_message(&self, message: &Message) -> PluginResult<Option<Message>> {
        let _ = message;
        Ok(None)
    }

    /// Handle application events
    async fn handle_event(&self, event: &AppEvent) -> PluginResult<()> {
        let _ = event;
        Ok(())
    }

    /// Get UI extensions provided by this plugin
    fn get_ui_extensions(&self) -> Vec<Box<dyn UIExtension>> {
        vec![]
    }

    /// Check if the plugin is healthy
    async fn health_check(&self) -> PluginResult<bool> {
        Ok(true)
    }

    /// Get plugin configuration schema
    fn get_config_schema(&self) -> Option<Value> {
        None
    }

    /// Validate plugin configuration
    fn validate_config(&self, config: &HashMap<String, Value>) -> PluginResult<()> {
        let _ = config;
        Ok(())
    }

    /// Get slash commands provided by this plugin
    fn get_slash_commands(&self) -> Vec<SlashCommand> {
        vec![]
    }
}

/// Trait for UI extensions that plugins can provide
pub trait UIExtension: Send + Sync {
    /// Get the extension ID
    fn id(&self) -> &str;

    /// Get the extension name
    fn name(&self) -> &str;

    /// Get the preferred position for this extension
    fn position(&self) -> UIPosition;

    /// Render the UI extension
    fn render(&self, area: Rect, frame: &mut Frame<'_>) -> Result<(), RuffError>;

    /// Handle input events
    fn handle_input(&mut self, event: &crossterm::event::Event) -> Result<bool, RuffError> {
        let _ = event;
        Ok(false)
    }

    /// Check if the extension should be visible
    fn is_visible(&self) -> bool {
        true
    }

    /// Get the minimum size required for this extension
    fn min_size(&self) -> (u16, u16) {
        (10, 3)
    }
}

/// UI position preferences for extensions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UIPosition {
    /// Top of the screen
    Top,
    /// Bottom of the screen
    Bottom,
    /// Left side
    Left,
    /// Right side
    Right,
    /// Floating/overlay
    Floating,
    /// Custom position with coordinates
    Custom { x: u16, y: u16 },
}

/// Trait for plugins that provide slash commands
pub trait SlashCommandProvider {
    /// Get the commands provided by this plugin
    fn get_commands(&self) -> Vec<SlashCommand>;
}

/// Definition of a slash command
#[derive(Clone)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub aliases: Vec<String>,
    pub handler: Arc<dyn SlashCommandHandler>,
}

/// Handler for slash commands
#[async_trait]
pub trait SlashCommandHandler: Send + Sync {
    async fn execute(&self, args: &[String], context: &PluginContext) -> PluginResult<CommandResult>;
}

/// Macro to help implement the Plugin trait
#[macro_export]
macro_rules! plugin_metadata {
    (
        id: $id:expr,
        name: $name:expr,
        version: $version:expr,
        description: $description:expr,
        author: $author:expr,
        $(homepage: $homepage:expr,)?
        $(repository: $repository:expr,)?
        $(license: $license:expr,)?
        $(dependencies: [$($dep:expr),*],)?
        $(permissions: [$($perm:ident),*],)?
        $(min_ruff_version: $min_version:expr,)?
    ) => {
        $crate::plugin::PluginMetadata {
            id: $id.to_string(),
            name: $name.to_string(),
            version: $version.to_string(),
            description: $description.to_string(),
            author: $author.to_string(),
            homepage: plugin_metadata!(@optional $($homepage)?),
            repository: plugin_metadata!(@optional $($repository)?),
            license: plugin_metadata!(@optional $($license)?),
            dependencies: vec![$($(String::from($dep)),*)?],
            permissions: vec![$($(crate::plugin::Permission::$perm),*)?],
            min_ruff_version: plugin_metadata!(@default $($min_version)?, "0.1.0").to_string(),
        }
    };
    
    (@optional $value:expr) => { Some($value.to_string()) };
    (@optional) => { None };
    
    (@default $value:expr, $default:expr) => { $value };
    (@default , $default:expr) => { $default };
}