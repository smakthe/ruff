//! Tests for command palette functionality

#[cfg(test)]
mod tests {
    use super::super::palette::*;

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
        
        let handler = Box::new(|_args: &[String]| {
            Ok(CommandResult::Success("Test executed".to_string()))
        });
        
        palette.register_command(test_command.clone(), handler);
        
        // Should have one more command
        assert_eq!(palette.filtered_commands().len(), initial_count + 1);
        
        // Should be able to find the command
        let found = palette.filtered_commands()
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
        let found = palette.filtered_commands()
            .iter()
            .find(|r| r.command.name == "New Session");
        assert!(found.is_some());
        
        // Test partial match
        palette.update_search("search".to_string());
        let search_commands: Vec<_> = palette.filtered_commands()
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
        assert_eq!(palette.selected_index(), palette.filtered_commands().len() - 1);
        
        // Test wrapping at end
        palette.select_next();
        assert_eq!(palette.selected_index(), 0);
    }

    #[test]
    fn test_command_execution() {
        let mut palette = CommandPalette::new();
        
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
            CommandError::NotFound(_) => {}, // Expected
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
        let found = palette.filtered_commands()
            .iter()
            .find(|r| r.command.id == "test.disabled");
        assert!(found.is_none());
        
        // Executing disabled command should return error
        let result = palette.execute_command("test.disabled", &[]);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            CommandError::Disabled(_) => {}, // Expected
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
        
        let session_results: Vec<_> = palette.filtered_commands()
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
                Box::new(|_: &[String]| Ok(CommandResult::Success("First".to_string()))) as CommandHandler,
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
                Box::new(|_: &[String]| Ok(CommandResult::Success("Second".to_string()))) as CommandHandler,
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
            assert!(results[i-1].score >= results[i].score);
        }
        
        // All results should have positive scores
        for result in results {
            assert!(result.score > 0);
        }
    }
}