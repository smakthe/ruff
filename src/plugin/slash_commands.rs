use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::plugin::{
    traits::{CommandResult, PluginContext, PluginResult, SlashCommand, SlashCommandHandler},
    PluginId,
};

/// Registry specifically for slash commands with advanced parsing and execution
pub struct SlashCommandRegistry {
    /// Registered slash commands by name
    commands: HashMap<String, RegisteredSlashCommand>,
    /// Command aliases mapping to command names
    aliases: HashMap<String, String>,
    /// Plugin ownership mapping
    plugin_commands: HashMap<PluginId, Vec<String>>,
}

/// Internal representation of a registered slash command
#[derive(Clone)]
struct RegisteredSlashCommand {
    command: SlashCommand,
    plugin_id: PluginId,
    enabled: bool,
}

impl SlashCommandRegistry {
    /// Create a new slash command registry
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
            plugin_commands: HashMap::new(),
        }
    }

    /// Register a slash command from a plugin
    pub fn register_command(
        &mut self,
        plugin_id: PluginId,
        command: SlashCommand,
    ) -> Result<(), String> {
        let name = command.name.clone();

        // Check if command already exists
        if self.commands.contains_key(&name) {
            return Err(format!("Slash command '{}' already registered", name));
        }

        // Check if any aliases conflict with existing commands or aliases
        for alias in &command.aliases {
            if self.commands.contains_key(alias) || self.aliases.contains_key(alias) {
                return Err(format!(
                    "Slash command alias '{}' conflicts with existing command",
                    alias
                ));
            }
        }

        // Register the main command
        let registered_command = RegisteredSlashCommand {
            command: command.clone(),
            plugin_id: plugin_id.clone(),
            enabled: true,
        };
        self.commands.insert(name.clone(), registered_command);

        // Register aliases
        for alias in &command.aliases {
            self.aliases.insert(alias.clone(), name.clone());
        }

        // Track plugin ownership
        self.plugin_commands
            .entry(plugin_id)
            .or_insert_with(Vec::new)
            .push(name);

        Ok(())
    }

    /// Unregister a specific slash command
    pub fn unregister_command(&mut self, name: &str) -> bool {
        if let Some(registered_command) = self.commands.remove(name) {
            // Remove aliases
            self.aliases.retain(|_, target| target != name);

            // Remove from plugin tracking
            if let Some(plugin_commands) =
                self.plugin_commands.get_mut(&registered_command.plugin_id)
            {
                plugin_commands.retain(|cmd| cmd != name);
                if plugin_commands.is_empty() {
                    self.plugin_commands.remove(&registered_command.plugin_id);
                }
            }

            true
        } else {
            false
        }
    }

    /// Unregister all commands from a specific plugin
    pub fn unregister_plugin_commands(&mut self, plugin_id: &PluginId) -> usize {
        let commands_to_remove: Vec<String> = self
            .plugin_commands
            .get(plugin_id)
            .cloned()
            .unwrap_or_default();

        let mut removed_count = 0;
        for command_name in commands_to_remove {
            if self.unregister_command(&command_name) {
                removed_count += 1;
            }
        }

        removed_count
    }

    /// Parse a slash command input and return the command name and arguments
    pub fn parse_command_input(&self, input: &str) -> Result<ParsedCommand, String> {
        let input = input.trim();

        // Check if it starts with a slash
        if !input.starts_with('/') {
            return Err("Command must start with '/'".to_string());
        }

        // Remove the leading slash
        let input = &input[1..];

        if input.is_empty() {
            return Err("Empty command".to_string());
        }

        // Split into command and arguments
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Err("Empty command".to_string());
        }

        let command_name = parts[0].to_string();
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

        // Resolve alias if necessary
        let resolved_name = self
            .aliases
            .get(&command_name)
            .unwrap_or(&command_name)
            .clone();

        Ok(ParsedCommand {
            name: resolved_name,
            original_name: command_name,
            args,
            raw_input: input.to_string(),
        })
    }

    /// Execute a slash command
    pub async fn execute_command(
        &self,
        input: &str,
        context: &PluginContext,
    ) -> PluginResult<CommandResult> {
        let parsed = match self.parse_command_input(input) {
            Ok(parsed) => parsed,
            Err(e) => return Ok(CommandResult::error(format!("Parse error: {}", e))),
        };

        if let Some(registered_command) = self.commands.get(&parsed.name) {
            if !registered_command.enabled {
                return Ok(CommandResult::error(format!(
                    "Command '{}' is disabled",
                    parsed.name
                )));
            }

            // Check if the plugin has permission to execute commands
            if !context.has_permission(&crate::plugin::Permission::SlashCommands) {
                return Ok(CommandResult::error(
                    "Plugin does not have permission to execute slash commands".to_string(),
                ));
            }

            registered_command
                .command
                .handler
                .execute(&parsed.args, context)
                .await
        } else {
            Ok(CommandResult::error(format!(
                "Slash command '{}' not found",
                parsed.name
            )))
        }
    }

    /// Get all registered slash commands
    pub fn get_commands(&self) -> Vec<&SlashCommand> {
        self.commands
            .values()
            .filter(|cmd| cmd.enabled)
            .map(|registered| &registered.command)
            .collect()
    }

    /// Get commands registered by a specific plugin
    pub fn get_plugin_commands(&self, plugin_id: &PluginId) -> Vec<&SlashCommand> {
        self.plugin_commands
            .get(plugin_id)
            .map(|command_names| {
                command_names
                    .iter()
                    .filter_map(|name| self.commands.get(name))
                    .filter(|cmd| cmd.enabled)
                    .map(|registered| &registered.command)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Search for commands matching a query
    pub fn search_commands(&self, query: &str) -> Vec<&SlashCommand> {
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();

        for registered_command in self.commands.values() {
            if !registered_command.enabled {
                continue;
            }

            let command = &registered_command.command;
            if command.name.to_lowercase().contains(&query_lower)
                || command.description.to_lowercase().contains(&query_lower)
                || command
                    .aliases
                    .iter()
                    .any(|alias| alias.to_lowercase().contains(&query_lower))
            {
                matches.push(command);
            }
        }

        // Sort by relevance (exact matches first, then by name)
        matches.sort_by(|a, b| {
            let a_exact = a.name.to_lowercase() == query_lower;
            let b_exact = b.name.to_lowercase() == query_lower;

            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        matches
    }

    /// Get help text for a specific command
    pub fn get_command_help(&self, name: &str) -> Option<String> {
        // Resolve alias if necessary
        let resolved_name = self.aliases.get(name).map_or(name, |v| v.as_str());

        self.commands.get(resolved_name).map(|registered| {
            let cmd = &registered.command;
            let mut help = format!("/{} - {}", cmd.name, cmd.description);

            if !cmd.usage.is_empty() && cmd.usage != format!("/{}", cmd.name) {
                help.push_str(&format!("\nUsage: {}", cmd.usage));
            }

            if !cmd.aliases.is_empty() {
                help.push_str(&format!("\nAliases: {}", cmd.aliases.join(", ")));
            }

            help.push_str(&format!("\nPlugin: {}", registered.plugin_id));

            help
        })
    }

    /// Get all available command names (including aliases)
    pub fn get_command_names(&self) -> Vec<String> {
        let mut names = Vec::new();

        // Add main command names
        for (name, registered) in &self.commands {
            if registered.enabled {
                names.push(name.clone());
            }
        }

        // Add aliases
        for (alias, target) in &self.aliases {
            if let Some(registered) = self.commands.get(target) {
                if registered.enabled {
                    names.push(alias.clone());
                }
            }
        }

        names.sort();
        names
    }

    /// Enable or disable a command
    pub fn set_command_enabled(&mut self, name: &str, enabled: bool) -> bool {
        // Resolve alias if necessary
        let resolved_name = self.aliases.get(name).map_or(name, |v| v.as_str());

        if let Some(registered_command) = self.commands.get_mut(resolved_name) {
            registered_command.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Check if a command exists
    pub fn has_command(&self, name: &str) -> bool {
        let resolved_name = self.aliases.get(name).map_or(name, |v| v.as_str());
        self.commands.contains_key(resolved_name)
    }

    /// Get statistics about the registry
    pub fn get_stats(&self) -> SlashCommandRegistryStats {
        let enabled_commands = self.commands.values().filter(|cmd| cmd.enabled).count();
        let disabled_commands = self.commands.len() - enabled_commands;

        SlashCommandRegistryStats {
            total_commands: self.commands.len(),
            enabled_commands,
            disabled_commands,
            total_aliases: self.aliases.len(),
            plugins_with_commands: self.plugin_commands.len(),
        }
    }

    /// Clear all commands
    pub fn clear(&mut self) {
        self.commands.clear();
        self.aliases.clear();
        self.plugin_commands.clear();
    }

    /// Get a list of all plugins that have registered commands
    pub fn get_plugins_with_commands(&self) -> Vec<PluginId> {
        self.plugin_commands.keys().cloned().collect()
    }
}

impl Default for SlashCommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed slash command with arguments
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    /// The resolved command name (after alias resolution)
    pub name: String,
    /// The original command name as typed by the user
    pub original_name: String,
    /// Command arguments
    pub args: Vec<String>,
    /// The raw input without the leading slash
    pub raw_input: String,
}

/// Statistics about the slash command registry
#[derive(Debug, Clone)]
pub struct SlashCommandRegistryStats {
    pub total_commands: usize,
    pub enabled_commands: usize,
    pub disabled_commands: usize,
    pub total_aliases: usize,
    pub plugins_with_commands: usize,
}

/// Helper trait for plugins to easily register slash commands
#[async_trait]
pub trait SlashCommandProvider {
    /// Get the slash commands provided by this plugin
    fn get_slash_commands(&self) -> Vec<SlashCommand>;
}

/// Builder for creating slash commands with fluent API
pub struct SlashCommandBuilder {
    name: String,
    description: String,
    usage: String,
    aliases: Vec<String>,
}

impl SlashCommandBuilder {
    /// Create a new slash command builder
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            usage: format!("/{}", name),
            name,
            description: String::new(),
            aliases: Vec::new(),
        }
    }

    /// Set the command description
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the command usage text
    pub fn usage(mut self, usage: impl Into<String>) -> Self {
        self.usage = usage.into();
        self
    }

    /// Add a single alias
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    /// Add multiple aliases
    pub fn aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases.extend(aliases);
        self
    }

    /// Build the slash command with the given handler
    pub fn build(self, handler: Arc<dyn SlashCommandHandler>) -> SlashCommand {
        SlashCommand {
            name: self.name,
            description: self.description,
            usage: self.usage,
            aliases: self.aliases,
            handler,
        }
    }
}

/// Macro for creating simple slash command handlers
#[macro_export]
macro_rules! slash_command_handler {
    ($name:expr, $description:expr, $usage:expr, |$args:ident, $context:ident| $body:block) => {{
        use async_trait::async_trait;
        use std::sync::Arc;
        use $crate::plugin::slash_commands::SlashCommandBuilder;
        use $crate::plugin::traits::{
            CommandResult, PluginContext, PluginResult, SlashCommandHandler,
        };

        struct Handler;

        #[async_trait]
        impl SlashCommandHandler for Handler {
            async fn execute(
                &self,
                $args: &[String],
                $context: &PluginContext,
            ) -> PluginResult<CommandResult> {
                $body
            }
        }

        SlashCommandBuilder::new($name)
            .description($description)
            .usage($usage)
            .build(Arc::new(Handler))
    }};

    ($name:expr, $description:expr, |$args:ident, $context:ident| $body:block) => {
        slash_command_handler!(
            $name,
            $description,
            format!("/{}", $name),
            |$args, $context| $body
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::plugin::Permission;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct TestSlashCommandHandler {
        response: String,
    }

    impl TestSlashCommandHandler {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait]
    impl SlashCommandHandler for TestSlashCommandHandler {
        async fn execute(
            &self,
            args: &[String],
            _context: &PluginContext,
        ) -> PluginResult<CommandResult> {
            Ok(CommandResult::success_with_message(format!(
                "{}: {:?}",
                self.response, args
            )))
        }
    }

    fn create_test_context() -> PluginContext {
        PluginContext::new(
            "test-plugin".to_string(),
            Arc::new(EventBus::new()),
            vec![Permission::SlashCommands],
            HashMap::new(),
        )
    }

    fn create_test_command(name: &str, aliases: Vec<String>) -> SlashCommand {
        SlashCommandBuilder::new(name)
            .description(format!("Test command {}", name))
            .usage(format!("/{} <args>", name))
            .aliases(aliases)
            .build(Arc::new(TestSlashCommandHandler::new(format!(
                "executed {}",
                name
            ))))
    }

    #[test]
    fn test_registry_creation() {
        let registry = SlashCommandRegistry::new();
        assert_eq!(registry.get_commands().len(), 0);
        assert_eq!(registry.get_command_names().len(), 0);
    }

    #[test]
    fn test_command_registration() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command = create_test_command("help", vec!["h".to_string()]);

        // Register command
        assert!(registry
            .register_command(plugin_id.clone(), command)
            .is_ok());

        // Check command exists
        assert!(registry.has_command("help"));
        assert!(registry.has_command("h")); // alias

        // Check stats
        let stats = registry.get_stats();
        assert_eq!(stats.total_commands, 1);
        assert_eq!(stats.enabled_commands, 1);
        assert_eq!(stats.total_aliases, 1);
        assert_eq!(stats.plugins_with_commands, 1);
    }

    #[test]
    fn test_duplicate_command_registration() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command1 = create_test_command("help", vec![]);
        let command2 = create_test_command("help", vec![]);

        // Register first command
        assert!(registry
            .register_command(plugin_id.clone(), command1)
            .is_ok());

        // Try to register duplicate
        assert!(registry.register_command(plugin_id, command2).is_err());
    }

    #[test]
    fn test_alias_conflict() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command1 = create_test_command("help", vec!["h".to_string()]);
        let command2 = create_test_command("hello", vec!["h".to_string()]);

        // Register first command
        assert!(registry
            .register_command(plugin_id.clone(), command1)
            .is_ok());

        // Try to register command with conflicting alias
        assert!(registry.register_command(plugin_id, command2).is_err());
    }

    #[test]
    fn test_command_parsing() {
        let registry = SlashCommandRegistry::new();

        // Valid commands
        let parsed = registry.parse_command_input("/help arg1 arg2").unwrap();
        assert_eq!(parsed.name, "help");
        assert_eq!(parsed.args, vec!["arg1", "arg2"]);

        let parsed = registry.parse_command_input("/test").unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.args.len(), 0);

        // Invalid commands
        assert!(registry.parse_command_input("help").is_err()); // no slash
        assert!(registry.parse_command_input("/").is_err()); // empty command
        assert!(registry.parse_command_input("").is_err()); // empty input
    }

    #[test]
    fn test_alias_resolution() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command = create_test_command("help", vec!["h".to_string(), "?".to_string()]);

        registry.register_command(plugin_id, command).unwrap();

        // Test alias resolution in parsing
        let parsed = registry.parse_command_input("/h").unwrap();
        assert_eq!(parsed.name, "help");
        assert_eq!(parsed.original_name, "h");

        let parsed = registry.parse_command_input("/?").unwrap();
        assert_eq!(parsed.name, "help");
        assert_eq!(parsed.original_name, "?");
    }

    #[tokio::test]
    async fn test_command_execution() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command = create_test_command("test", vec!["t".to_string()]);

        registry.register_command(plugin_id, command).unwrap();

        let context = create_test_context();

        // Execute by name
        let result = registry
            .execute_command("/test arg1 arg2", &context)
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.message.unwrap().contains("executed test"));

        // Execute by alias
        let result = registry.execute_command("/t arg1", &context).await.unwrap();
        assert!(result.success);

        // Execute non-existent command
        let result = registry
            .execute_command("/nonexistent", &context)
            .await
            .unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_command_execution_without_permission() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command = create_test_command("test", vec![]);

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

    #[test]
    fn test_command_search() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();

        let commands = vec![
            create_test_command("help", vec!["h".to_string()]),
            create_test_command("hello", vec![]),
            create_test_command("test", vec!["t".to_string()]),
        ];

        for command in commands {
            registry
                .register_command(plugin_id.clone(), command)
                .unwrap();
        }

        // Search by name
        let results = registry.search_commands("hel");
        assert_eq!(results.len(), 2); // help and hello

        // Search by alias (also matches "hello" because it contains "h")
        let results = registry.search_commands("h");
        assert_eq!(results.len(), 2); // help (has alias 'h') and hello (contains 'h')

        // Exact match should come first
        let results = registry.search_commands("help");
        assert_eq!(results[0].name, "help");
    }

    #[test]
    fn test_command_unregistration() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command = create_test_command("test", vec!["t".to_string()]);

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

    #[test]
    fn test_plugin_command_unregistration() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();

        let commands = vec![
            create_test_command("cmd1", vec![]),
            create_test_command("cmd2", vec![]),
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

    #[test]
    fn test_command_enable_disable() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command = create_test_command("test", vec![]);

        registry.register_command(plugin_id, command).unwrap();
        assert_eq!(registry.get_commands().len(), 1);

        // Disable command
        assert!(registry.set_command_enabled("test", false));
        assert_eq!(registry.get_commands().len(), 0); // Should not appear in enabled commands

        // Re-enable command
        assert!(registry.set_command_enabled("test", true));
        assert_eq!(registry.get_commands().len(), 1);
    }

    #[test]
    fn test_command_help() {
        let mut registry = SlashCommandRegistry::new();
        let plugin_id = "test-plugin".to_string();
        let command = create_test_command("help", vec!["h".to_string()]);

        registry.register_command(plugin_id, command).unwrap();

        let help = registry.get_command_help("help").unwrap();
        assert!(help.contains("/help"));
        assert!(help.contains("Test command help"));
        assert!(help.contains("Aliases: h"));
        assert!(help.contains("Plugin: test-plugin"));

        // Test help for alias
        let help = registry.get_command_help("h").unwrap();
        assert!(help.contains("/help")); // Should resolve to main command
    }

    #[test]
    fn test_slash_command_builder() {
        let handler = Arc::new(TestSlashCommandHandler::new("test"));

        let command = SlashCommandBuilder::new("test")
            .description("Test command")
            .usage("/test <arg>")
            .alias("t")
            .aliases(vec!["tst".to_string()])
            .build(handler);

        assert_eq!(command.name, "test");
        assert_eq!(command.description, "Test command");
        assert_eq!(command.usage, "/test <arg>");
        assert_eq!(command.aliases, vec!["t", "tst"]);
    }

    #[test]
    fn test_slash_command_handler_macro() {
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
}
