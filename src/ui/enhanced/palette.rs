//! Command palette functionality

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;

/// Command palette for searchable actions
pub struct CommandPalette {
    commands: Vec<Command>,
    command_registry: HashMap<String, CommandHandler>,
    matcher: SkimMatcherV2,
    is_visible: bool,
    selected_index: usize,
    search_query: String,
    filtered_commands: Vec<CommandSearchResult>,
}

/// Command definition
#[derive(Debug, Clone)]
pub struct Command {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: CommandCategory,
    pub shortcut: Option<String>,
    pub enabled: bool,
}

/// Command categories for organization
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    Session,
    Message,
    Navigation,
    View,
    Export,
    Settings,
    Plugin,
    Help,
}

/// Command handler function type
pub type CommandHandler =
    Box<dyn for<'a> Fn(&'a [String]) -> Result<CommandResult, CommandError> + Send + Sync>;

/// Result of command execution
#[derive(Debug)]
pub enum CommandResult {
    Success(String),
    ShowMessage(String),
    NavigateTo(String),
    OpenDialog(String),
    ExecuteAction(String),
}

/// Command execution errors
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Command not found: {0}")]
    NotFound(String),
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Command disabled: {0}")]
    Disabled(String),
}

/// Search result with relevance score
#[derive(Debug, Clone)]
pub struct CommandSearchResult {
    pub command: Command,
    pub score: i64,
    pub matched_indices: Vec<usize>,
}

impl CommandPalette {
    /// Create a new command palette
    pub fn new() -> Self {
        let mut palette = Self {
            commands: Vec::new(),
            command_registry: HashMap::new(),
            matcher: SkimMatcherV2::default(),
            is_visible: false,
            selected_index: 0,
            search_query: String::new(),
            filtered_commands: Vec::new(),
        };

        palette.register_default_commands();
        palette
    }

    /// Register a new command
    pub fn register_command(&mut self, command: Command, handler: CommandHandler) {
        self.command_registry.insert(command.id.clone(), handler);
        self.commands.push(command);
        self.update_filtered_commands();
    }

    /// Register multiple commands at once
    pub fn register_commands(&mut self, commands: Vec<(Command, CommandHandler)>) {
        for (command, handler) in commands {
            self.command_registry.insert(command.id.clone(), handler);
            self.commands.push(command);
        }
        self.update_filtered_commands();
    }

    /// Show the command palette
    pub fn show(&mut self) {
        self.is_visible = true;
        self.selected_index = 0;
        self.search_query.clear();
        self.update_filtered_commands();
    }

    /// Hide the command palette
    pub fn hide(&mut self) {
        self.is_visible = false;
        self.search_query.clear();
        self.selected_index = 0;
    }

    /// Check if the palette is visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Handle key events for the command palette
    pub fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> crate::ui::UIAction {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (key.code, key.modifiers) {
            // Close palette
            (KeyCode::Esc, KeyModifiers::NONE) => {
                self.hide();
                crate::ui::UIAction::None
            }

            // Navigation
            (KeyCode::Up, KeyModifiers::NONE) => {
                self.select_previous();
                crate::ui::UIAction::None
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                self.select_next();
                crate::ui::UIAction::None
            }

            // Execute selected command
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if let Ok(result) = self.execute_selected(&[]) {
                    self.hide();
                    match result {
                        CommandResult::ExecuteAction(action) => match action.as_str() {
                            "new_session" => crate::ui::UIAction::CreateNewSession,
                            "goto_first_message" => crate::ui::UIAction::ScrollToTop,
                            "goto_last_message" => crate::ui::UIAction::ScrollToBottom,
                            "increase_font_size" => crate::ui::UIAction::IncreaseFontSize,
                            "decrease_font_size" => crate::ui::UIAction::DecreaseFontSize,
                            "toggle_theme" => crate::ui::UIAction::ToggleTheme,
                            "export_markdown" => crate::ui::UIAction::ExportSession(
                                crate::export::formats::ExportFormat::Markdown,
                            ),
                            "export_json" => crate::ui::UIAction::ExportSession(
                                crate::export::formats::ExportFormat::Json,
                            ),
                            _ => crate::ui::UIAction::None,
                        },
                        CommandResult::OpenDialog(dialog) => match dialog.as_str() {
                            "help_shortcuts" | "about" => crate::ui::UIAction::ShowHelp,
                            _ => crate::ui::UIAction::ShowCommandPalette,
                        },
                        CommandResult::Success(_)
                        | CommandResult::ShowMessage(_)
                        | CommandResult::NavigateTo(_) => crate::ui::UIAction::None,
                    }
                } else {
                    crate::ui::UIAction::None
                }
            }

            // Handle text input for search
            (KeyCode::Char(c), KeyModifiers::NONE) => {
                self.search_query.push(c);
                self.update_filtered_commands();
                crate::ui::UIAction::None
            }
            (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                self.search_query.push(c.to_uppercase().next().unwrap_or(c));
                self.update_filtered_commands();
                crate::ui::UIAction::None
            }

            // Backspace
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                self.search_query.pop();
                self.update_filtered_commands();
                crate::ui::UIAction::None
            }

            _ => crate::ui::UIAction::None,
        }
    }

    /// Update search query and filter commands
    pub fn update_search(&mut self, query: String) {
        self.search_query = query;
        self.selected_index = 0;
        self.update_filtered_commands();
    }

    /// Get current search query
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Get filtered commands
    pub fn filtered_commands(&self) -> &[CommandSearchResult] {
        &self.filtered_commands
    }

    /// Get selected command index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else if !self.filtered_commands.is_empty() {
            self.selected_index = self.filtered_commands.len() - 1;
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected_index < self.filtered_commands.len().saturating_sub(1) {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    /// Execute the selected command
    pub fn execute_selected(&self, args: &[String]) -> Result<CommandResult, CommandError> {
        if let Some(result) = self.filtered_commands.get(self.selected_index) {
            self.execute_command(&result.command.id, args)
        } else {
            Err(CommandError::NotFound("No command selected".to_string()))
        }
    }

    /// Execute a command by ID
    pub fn execute_command(
        &self,
        command_id: &str,
        args: &[String],
    ) -> Result<CommandResult, CommandError> {
        if let Some(handler) = self.command_registry.get(command_id) {
            // Check if command is enabled
            if let Some(command) = self.commands.iter().find(|c| c.id == command_id) {
                if !command.enabled {
                    return Err(CommandError::Disabled(command_id.to_string()));
                }
            }
            handler(args)
        } else {
            Err(CommandError::NotFound(command_id.to_string()))
        }
    }

    /// Get commands by category
    pub fn get_commands_by_category(&self, category: CommandCategory) -> Vec<&Command> {
        self.commands
            .iter()
            .filter(|c| c.category == category)
            .collect()
    }

    /// Get all categories with command counts
    pub fn get_categories(&self) -> HashMap<CommandCategory, usize> {
        let mut categories = HashMap::new();
        for command in &self.commands {
            *categories.entry(command.category.clone()).or_insert(0) += 1;
        }
        categories
    }

    /// Update filtered commands based on search query
    fn update_filtered_commands(&mut self) {
        if self.search_query.is_empty() {
            // Show all enabled commands when no search query
            self.filtered_commands = self
                .commands
                .iter()
                .filter(|c| c.enabled)
                .map(|c| CommandSearchResult {
                    command: c.clone(),
                    score: 100,
                    matched_indices: Vec::new(),
                })
                .collect();
        } else {
            // Fuzzy search through commands
            let mut results = Vec::new();

            for command in &self.commands {
                if !command.enabled {
                    continue;
                }

                // Search in name, description, and category
                let search_text = format!(
                    "{} {} {:?}",
                    command.name, command.description, command.category
                );

                if let Some((score, indices)) =
                    self.matcher.fuzzy_indices(&search_text, &self.search_query)
                {
                    results.push(CommandSearchResult {
                        command: command.clone(),
                        score,
                        matched_indices: indices,
                    });
                }
            }

            // Sort by score (higher is better)
            results.sort_by(|a, b| b.score.cmp(&a.score));
            self.filtered_commands = results;
        }

        // Ensure selected index is valid
        if self.selected_index >= self.filtered_commands.len() {
            self.selected_index = 0;
        }
    }

    /// Register default commands
    fn register_default_commands(&mut self) {
        // Helper functions for command handlers
        fn new_session_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::ExecuteAction("new_session".to_string()))
        }

        fn open_session_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("session_browser".to_string()))
        }

        fn rename_session_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("rename_session".to_string()))
        }

        fn message_search_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("message_search".to_string()))
        }

        fn global_search_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("global_search".to_string()))
        }

        fn goto_message_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("goto_message".to_string()))
        }

        fn goto_first_message_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::ExecuteAction(
                "goto_first_message".to_string(),
            ))
        }

        fn goto_last_message_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::ExecuteAction(
                "goto_last_message".to_string(),
            ))
        }

        fn increase_font_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::ExecuteAction(
                "increase_font_size".to_string(),
            ))
        }

        fn decrease_font_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::ExecuteAction(
                "decrease_font_size".to_string(),
            ))
        }

        fn toggle_theme_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::ExecuteAction("toggle_theme".to_string()))
        }

        fn export_markdown_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::ExecuteAction("export_markdown".to_string()))
        }

        fn export_json_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::ExecuteAction("export_json".to_string()))
        }

        fn settings_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("settings".to_string()))
        }

        fn model_settings_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("model_settings".to_string()))
        }

        fn help_shortcuts_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("help_shortcuts".to_string()))
        }

        fn about_handler(_args: &[String]) -> Result<CommandResult, CommandError> {
            Ok(CommandResult::OpenDialog("about".to_string()))
        }

        let default_commands = vec![
            // Session commands
            (
                Command {
                    id: "session.new".to_string(),
                    name: "New Session".to_string(),
                    description: "Create a new chat session".to_string(),
                    category: CommandCategory::Session,
                    shortcut: Some("Ctrl+N".to_string()),
                    enabled: true,
                },
                Box::new(new_session_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "session.open".to_string(),
                    name: "Open Session".to_string(),
                    description: "Open session browser".to_string(),
                    category: CommandCategory::Session,
                    shortcut: Some("Ctrl+O".to_string()),
                    enabled: true,
                },
                Box::new(open_session_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "session.rename".to_string(),
                    name: "Rename Session".to_string(),
                    description: "Rename the current session".to_string(),
                    category: CommandCategory::Session,
                    shortcut: Some("F2".to_string()),
                    enabled: true,
                },
                Box::new(rename_session_handler) as CommandHandler,
            ),
            // Message commands
            (
                Command {
                    id: "message.search".to_string(),
                    name: "Search Messages".to_string(),
                    description: "Search messages in current session".to_string(),
                    category: CommandCategory::Message,
                    shortcut: Some("Ctrl+F".to_string()),
                    enabled: true,
                },
                Box::new(message_search_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "message.search_global".to_string(),
                    name: "Global Search".to_string(),
                    description: "Search messages across all sessions".to_string(),
                    category: CommandCategory::Message,
                    shortcut: Some("Ctrl+Shift+F".to_string()),
                    enabled: true,
                },
                Box::new(global_search_handler) as CommandHandler,
            ),
            // Navigation commands
            (
                Command {
                    id: "navigation.goto_message".to_string(),
                    name: "Go to Message".to_string(),
                    description: "Jump to a specific message number".to_string(),
                    category: CommandCategory::Navigation,
                    shortcut: Some("Ctrl+G".to_string()),
                    enabled: true,
                },
                Box::new(goto_message_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "navigation.first_message".to_string(),
                    name: "First Message".to_string(),
                    description: "Jump to the first message".to_string(),
                    category: CommandCategory::Navigation,
                    shortcut: Some("Ctrl+Home".to_string()),
                    enabled: true,
                },
                Box::new(goto_first_message_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "navigation.last_message".to_string(),
                    name: "Last Message".to_string(),
                    description: "Jump to the last message".to_string(),
                    category: CommandCategory::Navigation,
                    shortcut: Some("Ctrl+End".to_string()),
                    enabled: true,
                },
                Box::new(goto_last_message_handler) as CommandHandler,
            ),
            // View commands
            (
                Command {
                    id: "view.increase_font".to_string(),
                    name: "Increase Font Size".to_string(),
                    description: "Make text larger".to_string(),
                    category: CommandCategory::View,
                    shortcut: Some("Ctrl+Plus".to_string()),
                    enabled: true,
                },
                Box::new(increase_font_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "view.decrease_font".to_string(),
                    name: "Decrease Font Size".to_string(),
                    description: "Make text smaller".to_string(),
                    category: CommandCategory::View,
                    shortcut: Some("Ctrl+Minus".to_string()),
                    enabled: true,
                },
                Box::new(decrease_font_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "view.toggle_theme".to_string(),
                    name: "Toggle Theme".to_string(),
                    description: "Switch between light and dark themes".to_string(),
                    category: CommandCategory::View,
                    shortcut: None,
                    enabled: true,
                },
                Box::new(toggle_theme_handler) as CommandHandler,
            ),
            // Export commands
            (
                Command {
                    id: "export.markdown".to_string(),
                    name: "Export as Markdown".to_string(),
                    description: "Export current conversation to Markdown".to_string(),
                    category: CommandCategory::Export,
                    shortcut: None,
                    enabled: true,
                },
                Box::new(export_markdown_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "export.json".to_string(),
                    name: "Export as JSON".to_string(),
                    description: "Export current conversation to JSON".to_string(),
                    category: CommandCategory::Export,
                    shortcut: None,
                    enabled: true,
                },
                Box::new(export_json_handler) as CommandHandler,
            ),
            // Settings commands
            (
                Command {
                    id: "settings.open".to_string(),
                    name: "Open Settings".to_string(),
                    description: "Open application settings".to_string(),
                    category: CommandCategory::Settings,
                    shortcut: Some("Ctrl+Comma".to_string()),
                    enabled: true,
                },
                Box::new(settings_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "settings.model".to_string(),
                    name: "Model Settings".to_string(),
                    description: "Configure AI model parameters".to_string(),
                    category: CommandCategory::Settings,
                    shortcut: Some("Ctrl+P".to_string()),
                    enabled: true,
                },
                Box::new(model_settings_handler) as CommandHandler,
            ),
            // Help commands
            (
                Command {
                    id: "help.shortcuts".to_string(),
                    name: "Keyboard Shortcuts".to_string(),
                    description: "Show all keyboard shortcuts".to_string(),
                    category: CommandCategory::Help,
                    shortcut: Some("F1".to_string()),
                    enabled: true,
                },
                Box::new(help_shortcuts_handler) as CommandHandler,
            ),
            (
                Command {
                    id: "help.about".to_string(),
                    name: "About Ruff".to_string(),
                    description: "Show application information".to_string(),
                    category: CommandCategory::Help,
                    shortcut: None,
                    enabled: true,
                },
                Box::new(about_handler) as CommandHandler,
            ),
        ];

        for (command, handler) in default_commands {
            self.command_registry.insert(command.id.clone(), handler);
            self.commands.push(command);
        }

        self.update_filtered_commands();
    }
}

impl Clone for CommandPalette {
    fn clone(&self) -> Self {
        // Create a new command palette and re-register all commands
        let mut new_palette = Self {
            commands: self.commands.clone(),
            command_registry: HashMap::new(),
            matcher: SkimMatcherV2::default(),
            is_visible: self.is_visible,
            selected_index: self.selected_index,
            search_query: self.search_query.clone(),
            filtered_commands: self.filtered_commands.clone(),
        };

        // Re-register default commands (we can't clone the function pointers)
        new_palette.register_default_commands();
        new_palette
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandCategory {
    /// Get display name for category
    pub fn display_name(&self) -> &'static str {
        match self {
            CommandCategory::Session => "Session",
            CommandCategory::Message => "Message",
            CommandCategory::Navigation => "Navigation",
            CommandCategory::View => "View",
            CommandCategory::Export => "Export",
            CommandCategory::Settings => "Settings",
            CommandCategory::Plugin => "Plugin",
            CommandCategory::Help => "Help",
        }
    }

    /// Get all categories
    pub fn all() -> Vec<CommandCategory> {
        vec![
            CommandCategory::Session,
            CommandCategory::Message,
            CommandCategory::Navigation,
            CommandCategory::View,
            CommandCategory::Export,
            CommandCategory::Settings,
            CommandCategory::Plugin,
            CommandCategory::Help,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_palette_creation() {
        let palette = CommandPalette::new();
        assert!(!palette.is_visible());
        assert_eq!(palette.selected_index(), 0);
        assert_eq!(palette.search_query(), "");

        // Should have default commands registered
        assert!(!palette.filtered_commands().is_empty());
    }

    #[test]
    fn test_command_registration() {
        let mut palette = CommandPalette::new();
        let initial_count = palette.filtered_commands().len();

        let test_command = Command {
            id: "test.command".to_string(),
            name: "Test Command".to_string(),
            description: "A test command".to_string(),
            category: CommandCategory::Plugin,
            shortcut: Some("Ctrl+T".to_string()),
            enabled: true,
        };

        let handler =
            Box::new(|_args: &[String]| Ok(CommandResult::Success("Test executed".to_string())));

        palette.register_command(test_command.clone(), handler);

        // Should have one more command
        assert_eq!(palette.filtered_commands().len(), initial_count + 1);

        // Should be able to find the command
        let found = palette
            .filtered_commands()
            .iter()
            .find(|r| r.command.id == "test.command");
        assert!(found.is_some());
    }

    #[test]
    fn test_fuzzy_search() {
        let mut palette = CommandPalette::new();

        // Test exact match
        palette.update_search("New Session".to_string());
        assert!(!palette.filtered_commands().is_empty());

        let first_result = &palette.filtered_commands()[0];
        assert_eq!(first_result.command.name, "New Session");

        // Test fuzzy match
        palette.update_search("new ses".to_string());
        assert!(!palette.filtered_commands().is_empty());

        // Should still find "New Session"
        let found = palette
            .filtered_commands()
            .iter()
            .find(|r| r.command.name == "New Session");
        assert!(found.is_some());

        // Test partial match
        palette.update_search("search".to_string());
        let search_commands: Vec<_> = palette
            .filtered_commands()
            .iter()
            .filter(|r| r.command.name.to_lowercase().contains("search"))
            .collect();
        assert!(!search_commands.is_empty());
    }

    #[test]
    fn test_search_clearing() {
        let mut palette = CommandPalette::new();
        let initial_count = palette.filtered_commands().len();

        // Apply search filter
        palette.update_search("nonexistent".to_string());
        assert!(palette.filtered_commands().is_empty());

        // Clear search
        palette.update_search("".to_string());
        assert_eq!(palette.filtered_commands().len(), initial_count);
    }

    #[test]
    fn test_selection_navigation() {
        let mut palette = CommandPalette::new();
        palette.show();

        assert_eq!(palette.selected_index(), 0);

        // Test moving down
        palette.select_next();
        assert_eq!(palette.selected_index(), 1);

        // Test moving up
        palette.select_previous();
        assert_eq!(palette.selected_index(), 0);

        // Test wrapping at beginning
        palette.select_previous();
        assert_eq!(
            palette.selected_index(),
            palette.filtered_commands().len() - 1
        );

        // Test wrapping at end
        palette.select_next();
        assert_eq!(palette.selected_index(), 0);
    }

    #[test]
    fn test_command_execution() {
        let palette = CommandPalette::new();

        // Test executing a default command
        let result = palette.execute_command("session.new", &[]);
        assert!(result.is_ok());

        match result.unwrap() {
            CommandResult::ExecuteAction(action) => {
                assert_eq!(action, "new_session");
            }
            _ => panic!("Expected ExecuteAction result"),
        }

        // Test executing non-existent command
        let result = palette.execute_command("nonexistent.command", &[]);
        assert!(result.is_err());

        match result.unwrap_err() {
            CommandError::NotFound(_) => {} // Expected
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_disabled_command() {
        let mut palette = CommandPalette::new();

        let disabled_command = Command {
            id: "test.disabled".to_string(),
            name: "Disabled Command".to_string(),
            description: "A disabled test command".to_string(),
            category: CommandCategory::Plugin,
            shortcut: None,
            enabled: false,
        };

        let handler = Box::new(|_args: &[String]| {
            Ok(CommandResult::Success("Should not execute".to_string()))
        });

        palette.register_command(disabled_command, handler);

        // Disabled command should not appear in filtered results
        let found = palette
            .filtered_commands()
            .iter()
            .find(|r| r.command.id == "test.disabled");
        assert!(found.is_none());

        // Executing disabled command should return error
        let result = palette.execute_command("test.disabled", &[]);
        assert!(result.is_err());

        match result.unwrap_err() {
            CommandError::Disabled(_) => {} // Expected
            _ => panic!("Expected Disabled error"),
        }
    }

    #[test]
    fn test_command_categories() {
        let palette = CommandPalette::new();

        // Test getting commands by category
        let session_commands = palette.get_commands_by_category(CommandCategory::Session);
        assert!(!session_commands.is_empty());

        for command in session_commands {
            assert_eq!(command.category, CommandCategory::Session);
        }

        // Test getting all categories
        let categories = palette.get_categories();
        assert!(categories.contains_key(&CommandCategory::Session));
        assert!(categories.contains_key(&CommandCategory::Message));
        assert!(categories.contains_key(&CommandCategory::Navigation));

        // Each category should have at least one command
        for (_, count) in categories {
            assert!(count > 0);
        }
    }

    #[test]
    fn test_visibility_toggle() {
        let mut palette = CommandPalette::new();

        assert!(!palette.is_visible());

        palette.show();
        assert!(palette.is_visible());
        assert_eq!(palette.selected_index(), 0);
        assert_eq!(palette.search_query(), "");

        palette.hide();
        assert!(!palette.is_visible());
    }

    #[test]
    fn test_search_with_categories() {
        let mut palette = CommandPalette::new();

        // Search for session-related commands
        palette.update_search("session".to_string());

        let session_results: Vec<_> = palette
            .filtered_commands()
            .iter()
            .filter(|r| r.command.category == CommandCategory::Session)
            .collect();

        assert!(!session_results.is_empty());

        // All results should have good relevance scores
        for result in session_results {
            assert!(result.score > 0);
        }
    }

    #[test]
    fn test_multiple_command_registration() {
        let mut palette = CommandPalette::new();
        let initial_count = palette.filtered_commands().len();

        let commands = vec![
            (
                Command {
                    id: "test.first".to_string(),
                    name: "First Test".to_string(),
                    description: "First test command".to_string(),
                    category: CommandCategory::Plugin,
                    shortcut: None,
                    enabled: true,
                },
                Box::new(|_: &[String]| Ok(CommandResult::Success("First".to_string())))
                    as CommandHandler,
            ),
            (
                Command {
                    id: "test.second".to_string(),
                    name: "Second Test".to_string(),
                    description: "Second test command".to_string(),
                    category: CommandCategory::Plugin,
                    shortcut: None,
                    enabled: true,
                },
                Box::new(|_: &[String]| Ok(CommandResult::Success("Second".to_string())))
                    as CommandHandler,
            ),
        ];

        palette.register_commands(commands);

        // Should have two more commands
        assert_eq!(palette.filtered_commands().len(), initial_count + 2);

        // Both commands should be executable
        assert!(palette.execute_command("test.first", &[]).is_ok());
        assert!(palette.execute_command("test.second", &[]).is_ok());
    }

    #[test]
    fn test_command_category_display_names() {
        assert_eq!(CommandCategory::Session.display_name(), "Session");
        assert_eq!(CommandCategory::Message.display_name(), "Message");
        assert_eq!(CommandCategory::Navigation.display_name(), "Navigation");
        assert_eq!(CommandCategory::View.display_name(), "View");
        assert_eq!(CommandCategory::Export.display_name(), "Export");
        assert_eq!(CommandCategory::Settings.display_name(), "Settings");
        assert_eq!(CommandCategory::Plugin.display_name(), "Plugin");
        assert_eq!(CommandCategory::Help.display_name(), "Help");
    }

    #[test]
    fn test_all_categories() {
        let all_categories = CommandCategory::all();
        assert_eq!(all_categories.len(), 8);
        assert!(all_categories.contains(&CommandCategory::Session));
        assert!(all_categories.contains(&CommandCategory::Message));
        assert!(all_categories.contains(&CommandCategory::Navigation));
        assert!(all_categories.contains(&CommandCategory::View));
        assert!(all_categories.contains(&CommandCategory::Export));
        assert!(all_categories.contains(&CommandCategory::Settings));
        assert!(all_categories.contains(&CommandCategory::Plugin));
        assert!(all_categories.contains(&CommandCategory::Help));
    }

    #[test]
    fn test_selected_command_execution() {
        let mut palette = CommandPalette::new();
        palette.show();

        // Should be able to execute the first (selected) command
        let result = palette.execute_selected(&[]);
        assert!(result.is_ok());

        // Move selection and execute different command
        palette.select_next();
        let result2 = palette.execute_selected(&[]);
        assert!(result2.is_ok());

        // Results should be different (different commands)
        // This is a basic check - in practice, commands might return similar results
        // but they should be from different command handlers
    }

    #[test]
    fn test_search_result_scoring() {
        let mut palette = CommandPalette::new();

        // Search for something that should match multiple commands
        palette.update_search("message".to_string());

        let results = palette.filtered_commands();
        assert!(!results.is_empty());

        // Results should be sorted by score (descending)
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }

        // All results should have positive scores
        for result in results {
            assert!(result.score > 0);
        }
    }
}
