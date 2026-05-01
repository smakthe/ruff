use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::plugin::traits::{
    CommandResult, PluginContext, PluginResult, SlashCommand, SlashCommandHandler,
};

/// Registry for managing plugin commands and slash commands
pub struct CommandRegistry {
    /// Regular commands registered by plugins
    commands: HashMap<String, Arc<dyn CommandHandler>>,
    /// Slash commands registered by plugins
    slash_commands: HashMap<String, SlashCommand>,
    /// Command aliases
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    /// Create a new command registry
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            slash_commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    /// Register a regular command
    pub fn register_command(
        &mut self,
        name: String,
        handler: Arc<dyn CommandHandler>,
        aliases: Vec<String>,
    ) -> Result<(), String> {
        // Check if command already exists
        if self.commands.contains_key(&name) {
            return Err(format!("Command '{}' already registered", name));
        }

        // Check if any aliases conflict
        for alias in &aliases {
            if self.commands.contains_key(alias) || self.aliases.contains_key(alias) {
                return Err(format!("Alias '{}' conflicts with existing command", alias));
            }
        }

        // Register the command
        self.commands.insert(name.clone(), handler);

        // Register aliases
        for alias in aliases {
            self.aliases.insert(alias, name.clone());
        }

        Ok(())
    }

    /// Register a slash command
    pub fn register_slash_command(&mut self, command: SlashCommand) -> Result<(), String> {
        let name = command.name.clone();

        // Check if command already exists
        if self.slash_commands.contains_key(&name) {
            return Err(format!("Slash command '{}' already registered", name));
        }

        // Check if any aliases conflict
        for alias in &command.aliases {
            if self.slash_commands.contains_key(alias) {
                return Err(format!(
                    "Slash command alias '{}' conflicts with existing command",
                    alias
                ));
            }
        }

        // Register aliases
        for alias in &command.aliases {
            let alias_command = SlashCommand {
                name: alias.clone(),
                description: format!("Alias for {}", command.name),
                usage: command.usage.clone(),
                aliases: vec![],
                handler: Arc::clone(&command.handler),
            };
            self.slash_commands.insert(alias.clone(), alias_command);
        }

        // Register the main command
        self.slash_commands.insert(name, command);

        Ok(())
    }

    /// Unregister a command
    pub fn unregister_command(&mut self, name: &str) -> bool {
        // Remove the command
        let removed = self.commands.remove(name).is_some();

        // Remove any aliases pointing to this command
        self.aliases.retain(|_, target| target != name);

        removed
    }

    /// Unregister a slash command
    pub fn unregister_slash_command(&mut self, name: &str) -> bool {
        self.slash_commands.remove(name).is_some()
    }

    /// Execute a regular command
    pub async fn execute_command(
        &self,
        name: &str,
        args: &[String],
        context: &PluginContext,
    ) -> PluginResult<CommandResult> {
        // Resolve alias if necessary
        let command_name = if let Some(target) = self.aliases.get(name) {
            target.as_str()
        } else {
            name
        };

        // Find and execute the command
        if let Some(handler) = self.commands.get(command_name) {
            handler.execute(args, context).await
        } else {
            Ok(CommandResult::error(format!(
                "Command '{}' not found",
                name
            )))
        }
    }

    /// Execute a slash command
    pub async fn execute_slash_command(
        &self,
        name: &str,
        args: &[String],
        context: &PluginContext,
    ) -> PluginResult<CommandResult> {
        if let Some(command) = self.slash_commands.get(name) {
            command.handler.execute(args, context).await
        } else {
            Ok(CommandResult::error(format!(
                "Slash command '{}' not found",
                name
            )))
        }
    }

    /// Get all registered commands
    pub fn get_commands(&self) -> Vec<String> {
        self.commands.keys().cloned().collect()
    }

    /// Get all registered slash commands
    pub fn get_slash_commands(&self) -> Vec<&SlashCommand> {
        self.slash_commands.values().collect()
    }

    /// Search for commands matching a query
    pub fn search_commands(&self, query: &str) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();

        // Search regular commands
        for name in self.commands.keys() {
            if name.to_lowercase().contains(&query_lower) {
                matches.push(name.clone());
            }
        }

        // Search aliases
        for alias in self.aliases.keys() {
            if alias.to_lowercase().contains(&query_lower) {
                matches.push(alias.clone());
            }
        }

        matches.sort();
        matches.dedup();
        matches
    }

    /// Search for slash commands matching a query
    pub fn search_slash_commands(&self, query: &str) -> Vec<&SlashCommand> {
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();

        for command in self.slash_commands.values() {
            if command.name.to_lowercase().contains(&query_lower)
                || command.description.to_lowercase().contains(&query_lower)
            {
                matches.push(command);
            }
        }

        // Sort by name
        matches.sort_by(|a, b| a.name.cmp(&b.name));
        matches.dedup_by(|a, b| a.name == b.name);
        matches
    }

    /// Get command help text
    pub fn get_command_help(&self, name: &str) -> Option<String> {
        // For regular commands, we don't have built-in help
        // This could be extended to include help text in the future
        if self.commands.contains_key(name) || self.aliases.contains_key(name) {
            Some(format!("Command: {}", name))
        } else {
            None
        }
    }

    /// Get slash command help text
    pub fn get_slash_command_help(&self, name: &str) -> Option<String> {
        self.slash_commands
            .get(name)
            .map(|cmd| format!("/{} - {}\nUsage: {}", cmd.name, cmd.description, cmd.usage))
    }

    /// Clear all registered commands
    pub fn clear(&mut self) {
        self.commands.clear();
        self.slash_commands.clear();
        self.aliases.clear();
    }

    /// Get statistics about registered commands
    pub fn get_stats(&self) -> CommandRegistryStats {
        CommandRegistryStats {
            regular_commands: self.commands.len(),
            slash_commands: self.slash_commands.len(),
            aliases: self.aliases.len(),
        }
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Handler trait for regular commands
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(
        &self,
        args: &[String],
        context: &PluginContext,
    ) -> PluginResult<CommandResult>;
}

/// Statistics about the command registry
#[derive(Debug, Clone)]
pub struct CommandRegistryStats {
    pub regular_commands: usize,
    pub slash_commands: usize,
    pub aliases: usize,
}

/// Helper struct for creating slash commands
pub struct SlashCommandBuilder {
    name: String,
    description: String,
    usage: String,
    aliases: Vec<String>,
}

impl SlashCommandBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            usage: format!("/{}", name),
            name,
            description: String::new(),
            aliases: Vec::new(),
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn usage(mut self, usage: impl Into<String>) -> Self {
        self.usage = usage.into();
        self
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases.extend(aliases);
        self
    }

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
macro_rules! slash_command {
    ($name:expr, $description:expr, $usage:expr, |$args:ident, $context:ident| $body:block) => {{
        use async_trait::async_trait;
        use std::sync::Arc;
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

        $crate::plugin::registry::SlashCommandBuilder::new($name)
            .description($description)
            .usage($usage)
            .build(Arc::new(Handler))
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::plugin::Permission;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct TestCommandHandler;

    #[async_trait]
    impl CommandHandler for TestCommandHandler {
        async fn execute(
            &self,
            args: &[String],
            _context: &PluginContext,
        ) -> PluginResult<CommandResult> {
            Ok(CommandResult::success_with_message(format!(
                "Executed with args: {:?}",
                args
            )))
        }
    }

    struct TestSlashCommandHandler;

    #[async_trait]
    impl SlashCommandHandler for TestSlashCommandHandler {
        async fn execute(
            &self,
            args: &[String],
            _context: &PluginContext,
        ) -> PluginResult<CommandResult> {
            Ok(CommandResult::success_with_message(format!(
                "Slash command executed with args: {:?}",
                args
            )))
        }
    }

    fn create_test_context() -> PluginContext {
        PluginContext::new(
            "test-plugin".to_string(),
            Arc::new(EventBus::new()),
            vec![Permission::ReadSessions],
            HashMap::new(),
        )
    }

    #[tokio::test]
    async fn test_command_registration() {
        let mut registry = CommandRegistry::new();
        let handler = Arc::new(TestCommandHandler);

        // Register command
        assert!(registry
            .register_command(
                "test".to_string(),
                handler as Arc<dyn CommandHandler>,
                vec!["t".to_string()]
            )
            .is_ok());

        // Check command exists
        assert!(registry.get_commands().contains(&"test".to_string()));

        // Try to register duplicate
        let handler2 = Arc::new(TestCommandHandler);
        assert!(registry
            .register_command(
                "test".to_string(),
                handler2 as Arc<dyn CommandHandler>,
                vec![]
            )
            .is_err());
    }

    #[tokio::test]
    async fn test_slash_command_registration() {
        let mut registry = CommandRegistry::new();
        let handler = Arc::new(TestSlashCommandHandler);

        let command = SlashCommandBuilder::new("help")
            .description("Show help")
            .usage("/help [command]")
            .alias("h")
            .build(handler);

        // Register slash command
        assert!(registry.register_slash_command(command).is_ok());

        // Check command exists
        let commands = registry.get_slash_commands();
        assert!(commands.iter().any(|cmd| cmd.name == "help"));
        assert!(commands.iter().any(|cmd| cmd.name == "h")); // alias
    }

    #[tokio::test]
    async fn test_command_execution() {
        let mut registry = CommandRegistry::new();
        let handler = Arc::new(TestCommandHandler);

        registry
            .register_command(
                "test".to_string(),
                handler as Arc<dyn CommandHandler>,
                vec!["t".to_string()],
            )
            .unwrap();

        let context = create_test_context();
        let args = vec!["arg1".to_string(), "arg2".to_string()];

        // Execute by name
        let result = registry
            .execute_command("test", &args, &context)
            .await
            .unwrap();
        assert!(result.success);

        // Execute by alias
        let result = registry
            .execute_command("t", &args, &context)
            .await
            .unwrap();
        assert!(result.success);

        // Execute non-existent command
        let result = registry
            .execute_command("nonexistent", &args, &context)
            .await
            .unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_slash_command_execution() {
        let mut registry = CommandRegistry::new();
        let handler = Arc::new(TestSlashCommandHandler);

        let command = SlashCommandBuilder::new("test")
            .description("Test command")
            .build(handler);

        registry.register_slash_command(command).unwrap();

        let context = create_test_context();
        let args = vec!["arg1".to_string()];

        // Execute slash command
        let result = registry
            .execute_slash_command("test", &args, &context)
            .await
            .unwrap();
        assert!(result.success);

        // Execute non-existent slash command
        let result = registry
            .execute_slash_command("nonexistent", &args, &context)
            .await
            .unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_command_search() {
        let mut registry = CommandRegistry::new();
        let handler = Arc::new(TestCommandHandler);

        registry
            .register_command(
                "help".to_string(),
                Arc::clone(&handler) as Arc<dyn CommandHandler>,
                vec!["h".to_string()],
            )
            .unwrap();
        registry
            .register_command(
                "test".to_string(),
                Arc::clone(&handler) as Arc<dyn CommandHandler>,
                vec!["t".to_string()],
            )
            .unwrap();
        registry
            .register_command(
                "example".to_string(),
                handler as Arc<dyn CommandHandler>,
                vec![],
            )
            .unwrap();

        // Search for commands
        let results = registry.search_commands("he");
        assert!(results.contains(&"help".to_string()));

        let results = registry.search_commands("t");
        assert!(results.contains(&"test".to_string()));
        assert!(results.contains(&"t".to_string())); // alias
    }

    #[test]
    fn test_slash_command_builder() {
        let handler = Arc::new(TestSlashCommandHandler);

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

    #[test]
    fn test_registry_stats() {
        let mut registry = CommandRegistry::new();
        let handler = Arc::new(TestCommandHandler);
        let slash_handler = Arc::new(TestSlashCommandHandler);

        registry
            .register_command(
                "cmd1".to_string(),
                Arc::clone(&handler) as Arc<dyn CommandHandler>,
                vec!["c1".to_string()],
            )
            .unwrap();
        registry
            .register_command(
                "cmd2".to_string(),
                handler as Arc<dyn CommandHandler>,
                vec![],
            )
            .unwrap();

        let slash_cmd = SlashCommandBuilder::new("slash1")
            .alias("s1")
            .build(slash_handler);
        registry.register_slash_command(slash_cmd).unwrap();

        let stats = registry.get_stats();
        assert_eq!(stats.regular_commands, 2);
        assert_eq!(stats.slash_commands, 2); // includes alias
        assert_eq!(stats.aliases, 1);
    }
}
