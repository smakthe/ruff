//! Integration tests for enhanced UI components

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::{
        config::Config,
        models::{AIModel, TokenUsage},
        session::manager::{ChatSession, Message, MessageRole, MessageMetadata},
        ui::enhanced::{
            help::HelpSection,
            palette::{CommandCategory, CommandResult},
        },
    };
    use std::collections::HashMap;
    use uuid::Uuid;
    use chrono::Local;

    fn create_test_config() -> Config {
        Config {
            default_model: "test-model".to_string(),
            temperature: 0.7,
            max_tokens: 1000,
            api_keys: HashMap::new(),
            system_prompt: None,
            auto_save: true,
            theme: "rust".to_string(),
        }
    }

    fn create_test_session() -> ChatSession {
        let session_id = Uuid::new_v4();
        let mut session = ChatSession {
            id: session_id,
            title: "Test Session".to_string(),
            model: "test-model".to_string(),
            system_prompt: None,
            messages: Vec::new(),
            created_at: Local::now(),
            updated_at: Local::now(),
            total_tokens_used: TokenUsage {
                input_tokens: 100,
                output_tokens: 150,
                total_tokens: 250,
            },
            metadata: HashMap::new(),
        };

        // Add some test messages
        session.messages.push(Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "Hello, this is a test message with some **markdown** and `code`.".to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test-model".to_string(),
                temperature: 0.7,
                response_time_ms: 0,
                is_regenerated: false,
                regeneration_count: 0,
            },
        });

        session.messages.push(Message {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content: r#"Here's a response with code:

```rust
fn main() {
    println!("Hello, world!");
    let x = 42;
    println!("The answer is {}", x);
}
```

And some more **formatted** text with *emphasis*."#.to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: Some(TokenUsage {
                input_tokens: 50,
                output_tokens: 75,
                total_tokens: 125,
            }),
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test-model".to_string(),
                temperature: 0.7,
                response_time_ms: 1500,
                is_regenerated: false,
                regeneration_count: 0,
            },
        });

        session
    }

    fn create_test_model() -> AIModel {
        AIModel {
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            max_tokens: 4000,
            supports_system: true,
            supports_streaming: false,
        }
    }

    #[test]
    fn test_ui_creation_with_enhanced_components() {
        let config = create_test_config();
        let ui_result = UI::new(config);
        
        assert!(ui_result.is_ok());
        let ui = ui_result.unwrap();
        
        // Test that enhanced components are initialized
        assert!(!ui.get_markdown_renderer().extract_text("# Test").is_empty());
        assert!(!ui.get_syntax_highlighter().supported_languages().is_empty());
        assert!(!ui.get_theme_service().get_available_themes().is_empty());
        assert!(!ui.get_command_palette().is_visible());
        assert!(!ui.get_help_system().is_visible());
    }

    #[test]
    fn test_markdown_rendering_integration() {
        let config = create_test_config();
        let ui = UI::new(config).unwrap();
        let renderer = ui.get_markdown_renderer();
        
        let markdown = "# Header\n\n**Bold text** and `inline code`\n\n```rust\nfn test() {}\n```";
        let rendered = renderer.render(markdown);
        
        assert!(!rendered.text.lines.is_empty());
        assert_eq!(rendered.code_blocks.len(), 1);
        assert_eq!(rendered.code_blocks[0].language, Some("rust".to_string()));
        assert!(rendered.code_blocks[0].content.contains("fn test()"));
    }

    #[test]
    fn test_syntax_highlighting_integration() {
        let config = create_test_config();
        let ui = UI::new(config).unwrap();
        let highlighter = ui.get_syntax_highlighter();
        
        let code = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let highlighted = highlighter.highlight(code, "rust");
        
        assert!(!highlighted.is_empty());
        assert_eq!(highlighted.len(), 3); // Three lines of code
        
        // Each line should have the pipe prefix for code block formatting
        for line in &highlighted {
            assert!(!line.spans.is_empty());
            assert!(line.spans[0].content.contains("│"));
        }
    }

    #[test]
    fn test_theme_management() {
        let config = create_test_config();
        let mut ui = UI::new(config).unwrap();
        
        // Test default theme
        let current_theme = ui.get_theme_service().get_current_theme();
        assert_eq!(current_theme.name, "Rust");
        
        // Test theme switching
        assert!(ui.set_theme("midnight").is_ok());
        let new_theme = ui.get_theme_service().get_current_theme();
        assert_eq!(new_theme.name, "Midnight");
        
        // Test invalid theme
        assert!(ui.set_theme("nonexistent").is_err());
        
        // Test theme toggle
        assert!(ui.toggle_theme().is_ok());
        let toggled_theme = ui.get_theme_service().get_current_theme();
        assert_eq!(toggled_theme.name, "Solar"); // Should switch to light theme
    }

    #[test]
    fn test_command_palette_integration() {
        let config = create_test_config();
        let mut ui = UI::new(config).unwrap();
        
        // Test initial state
        assert!(!ui.get_command_palette().is_visible());
        
        // Test showing command palette
        ui.get_command_palette_mut().show();
        assert!(ui.get_command_palette().is_visible());
        
        // Test search functionality
        ui.get_command_palette_mut().update_search("new".to_string());
        let filtered = ui.get_command_palette().filtered_commands();
        assert!(!filtered.is_empty());
        
        // Should find "New Session" command
        let found_new_session = filtered.iter()
            .any(|result| result.command.name.contains("New Session"));
        assert!(found_new_session);
        
        // Test hiding
        ui.get_command_palette_mut().hide();
        assert!(!ui.get_command_palette().is_visible());
    }

    #[test]
    fn test_help_system_integration() {
        let config = create_test_config();
        let mut ui = UI::new(config).unwrap();
        
        // Test initial state
        assert!(!ui.get_help_system().is_visible());
        
        // Test showing help
        ui.get_help_system_mut().show();
        assert!(ui.get_help_system().is_visible());
        
        // Test shortcuts search
        ui.get_help_system_mut().update_search("ctrl".to_string());
        let filtered = ui.get_help_system().filtered_shortcuts();
        assert!(!filtered.is_empty());
        
        // Should find shortcuts with Ctrl
        let found_ctrl_shortcut = filtered.iter()
            .any(|result| result.shortcut.key_combination.contains("Ctrl"));
        assert!(found_ctrl_shortcut);
        
        // Test section switching
        ui.get_help_system_mut().switch_section(HelpSection::About);
        assert_eq!(*ui.get_help_system().current_section(), HelpSection::About);
        
        // Test hiding
        ui.get_help_system_mut().hide();
        assert!(!ui.get_help_system().is_visible());
    }

    #[test]
    fn test_enhanced_key_handling() {
        let config = create_test_config();
        let mut ui = UI::new(config).unwrap();
        
        // Test command palette shortcut
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('p'),
            crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
        );
        let action = ui.handle_key_event(key);
        assert!(matches!(action, UIAction::ShowCommandPalette));
        assert!(ui.get_command_palette().is_visible());
        
        // Test help shortcut
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::F(1),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = ui.handle_key_event(key);
        assert!(matches!(action, UIAction::ShowHelp));
        assert!(ui.get_help_system().is_visible());
        
        // Test escape to close help
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        );
        let action = ui.handle_key_event(key);
        assert!(matches!(action, UIAction::None));
        assert!(!ui.get_help_system().is_visible());
    }

    #[test]
    fn test_font_size_management() {
        let config = create_test_config();
        let mut ui = UI::new(config).unwrap();
        
        // Test default font size
        assert_eq!(ui.get_font_size(), 14);
        
        // Test increase font size
        let result = ui.increase_font_size();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 15);
        assert_eq!(ui.get_font_size(), 15);
        
        // Test decrease font size
        let result = ui.decrease_font_size();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 14);
        assert_eq!(ui.get_font_size(), 14);
        
        // Test reset font size
        ui.increase_font_size().unwrap();
        ui.increase_font_size().unwrap();
        assert_eq!(ui.get_font_size(), 16);
        
        let result = ui.reset_font_size();
        assert!(result.is_ok());
        assert_eq!(ui.get_font_size(), 14);
    }

    #[test]
    fn test_layout_manager_integration() {
        let config = create_test_config();
        let ui = UI::new(config).unwrap();
        
        let layout_manager = ui.get_layout_manager();
        
        // Test that layout manager is properly initialized
        assert_eq!(layout_manager.get_font_size(), 14);
        
        // Test that panes are configured
        let panes = layout_manager.get_panes();
        assert!(panes.iter().any(|p| p.id == "header"));
        assert!(panes.iter().any(|p| p.id == "messages"));
        assert!(panes.iter().any(|p| p.id == "input"));
        assert!(panes.iter().any(|p| p.id == "status"));
    }

    #[test]
    fn test_message_formatting_with_markdown() {
        let config = create_test_config();
        let ui = UI::new(config).unwrap();
        let session = create_test_session();
        
        // Test that we can format messages (this tests the integration)
        let markdown_renderer = ui.get_markdown_renderer();
        
        // Find the assistant message with code
        let assistant_message = session.messages.iter()
            .find(|m| m.role == MessageRole::Assistant)
            .unwrap();
        
        // Test that markdown rendering works for the message content
        let rendered = markdown_renderer.render(&assistant_message.content);
        assert!(!rendered.text.lines.is_empty());
        assert_eq!(rendered.code_blocks.len(), 1);
        assert!(rendered.code_blocks[0].content.contains("println!"));
    }

    #[test]
    fn test_ui_extension_manager_integration() {
        let config = create_test_config();
        let mut ui = UI::new(config).unwrap();
        
        // Test that UI extension manager can be set
        assert!(ui.get_ui_extension_manager().is_none());
        
        // In a real test, we would create a mock UIExtensionManager
        // For now, just test that the getter works
        let extension_config = ui.get_ui_extension_config();
        // Extension config should be initialized with defaults
        assert!(extension_config.enabled_extensions.is_empty());
    }

    #[test]
    fn test_scroll_functionality() {
        let config = create_test_config();
        let mut ui = UI::new(config).unwrap();
        
        // Test scroll to top
        let result = ui.scroll_to_top();
        assert!(result.is_ok());
        
        // Test scroll to bottom
        let result = ui.scroll_to_bottom();
        assert!(result.is_ok());
        
        // Test scroll to specific message
        let result = ui.scroll_to_message(1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_state_rendering() {
        let config = create_test_config();
        let mut ui = UI::new(config).unwrap();
        let model = create_test_model();
        
        // Test that empty state can be rendered without panicking
        let result = ui.render_empty_state(&model);
        assert!(result.is_ok());
    }

    #[test]
    fn test_theme_colors_integration() {
        let config = create_test_config();
        let ui = UI::new(config).unwrap();
        
        let theme_service = ui.get_theme_service();
        let current_theme = theme_service.get_current_theme();
        
        // Test that theme colors are properly defined
        assert_ne!(current_theme.colors.primary, current_theme.colors.background);
        assert_ne!(current_theme.colors.on_primary, current_theme.colors.on_background);
        
        // Test that syntax colors are defined
        assert_ne!(current_theme.colors.syntax_keyword, current_theme.colors.syntax_string);
        assert_ne!(current_theme.colors.syntax_comment, current_theme.colors.syntax_function);
    }

    #[test]
    fn test_command_execution_integration() {
        let config = create_test_config();
        let ui = UI::new(config).unwrap();
        
        let command_palette = ui.get_command_palette();
        
        // Test that default commands are registered
        let session_commands = command_palette.get_commands_by_category(
            CommandCategory::Session
        );
        assert!(!session_commands.is_empty());
        
        let navigation_commands = command_palette.get_commands_by_category(
            CommandCategory::Navigation
        );
        assert!(!navigation_commands.is_empty());
        
        // Test command execution
        let result = command_palette.execute_command("session.new", &[]);
        assert!(result.is_ok());
        
        match result.unwrap() {
            CommandResult::ExecuteAction(action) => {
                assert_eq!(action, "new_session");
            }
            _ => panic!("Expected ExecuteAction result"),
        }
    }
}