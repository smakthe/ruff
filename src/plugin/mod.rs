pub mod manager;
pub mod traits;
pub mod registry;
pub mod security;
pub mod slash_commands;
pub mod ui_extensions;
pub mod error_isolation;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ui_extensions_tests;

pub use manager::PluginManager;
pub use traits::{Plugin, PluginContext, PluginResult, CommandResult, UIExtension, SlashCommandHandler, UIPosition};
pub use registry::CommandRegistry;
pub use security::{PluginSandbox, SecurityPolicy};
pub use slash_commands::{SlashCommandRegistry, SlashCommandBuilder, SlashCommandProvider, ParsedCommand};
pub use ui_extensions::{UIExtensionManager, UIExtensionConfig, UIExtensionLayout, UIExtensionStats, UIExtensionInfo};
pub use error_isolation::{PluginErrorIsolation, PluginHealth, HealthStatus, IsolationConfig, PluginErrorEvent};

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

pub type PluginId = String;

/// Plugin metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<String>,
    pub permissions: Vec<Permission>,
    pub min_ruff_version: String,
}

/// Plugin permissions for security
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Access to read session data
    ReadSessions,
    /// Access to modify session data
    WriteSessions,
    /// Access to read messages
    ReadMessages,
    /// Access to modify messages
    WriteMessages,
    /// Access to make network requests
    Network,
    /// Access to file system
    FileSystem,
    /// Access to execute system commands
    SystemCommands,
    /// Access to clipboard
    Clipboard,
    /// Access to register UI extensions
    UIExtensions,
    /// Access to register slash commands
    SlashCommands,
    /// Access to configuration
    Configuration,
}

/// Plugin status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginStatus {
    Loaded,
    Unloaded,
    Error(String),
    Disabled,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    pub settings: HashMap<String, serde_json::Value>,
    pub permissions: Vec<Permission>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            settings: HashMap::new(),
            permissions: vec![],
        }
    }
}