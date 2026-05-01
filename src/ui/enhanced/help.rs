//! Help system functionality

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;

/// Help system for keyboard shortcuts and documentation
pub struct HelpSystem {
    shortcuts: Vec<KeyboardShortcut>,
    help_sections: HashMap<HelpSection, Vec<HelpItem>>,
    matcher: SkimMatcherV2,
    is_visible: bool,
    current_section: HelpSection,
    search_query: String,
    filtered_shortcuts: Vec<ShortcutSearchResult>,
    selected_index: usize,
}

/// Keyboard shortcut definition
#[derive(Debug, Clone)]
pub struct KeyboardShortcut {
    pub key_combination: String,
    pub description: String,
    pub category: ShortcutCategory,
    pub context: ShortcutContext,
    pub command_id: Option<String>,
}

/// Categories for organizing shortcuts
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShortcutCategory {
    Session,
    Message,
    Navigation,
    View,
    Export,
    Settings,
    Help,
    General,
}

/// Context where shortcuts are available
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutContext {
    Global,
    Chat,
    CommandPalette,
    Dialog,
    MessageEdit,
}

/// Help sections for different topics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpSection {
    KeyboardShortcuts,
    GettingStarted,
    Features,
    Configuration,
    Troubleshooting,
    About,
}

/// Help item with content
#[derive(Debug, Clone)]
pub struct HelpItem {
    pub title: String,
    pub content: String,
    pub subsections: Vec<HelpSubsection>,
}

/// Help subsection
#[derive(Debug, Clone)]
pub struct HelpSubsection {
    pub title: String,
    pub content: String,
}

/// Search result for shortcuts
#[derive(Debug, Clone)]
pub struct ShortcutSearchResult {
    pub shortcut: KeyboardShortcut,
    pub score: i64,
    pub matched_indices: Vec<usize>,
}

impl HelpSystem {
    /// Create a new help system
    pub fn new() -> Self {
        let mut help_system = Self {
            shortcuts: Vec::new(),
            help_sections: HashMap::new(),
            matcher: SkimMatcherV2::default(),
            is_visible: false,
            current_section: HelpSection::KeyboardShortcuts,
            search_query: String::new(),
            filtered_shortcuts: Vec::new(),
            selected_index: 0,
        };

        help_system.initialize_shortcuts();
        help_system.initialize_help_content();
        help_system.update_filtered_shortcuts();
        help_system
    }

    /// Show the help system
    pub fn show(&mut self) {
        self.is_visible = true;
        self.selected_index = 0;
        self.search_query.clear();
        self.update_filtered_shortcuts();
    }

    /// Hide the help system
    pub fn hide(&mut self) {
        self.is_visible = false;
        self.search_query.clear();
        self.selected_index = 0;
    }

    /// Check if help system is visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Handle key events for the help system
    pub fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> crate::ui::UIAction {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (key.code, key.modifiers) {
            // Close help system
            (KeyCode::Esc, KeyModifiers::NONE) => {
                self.hide();
                crate::ui::UIAction::None
            }

            // Navigation within shortcuts
            (KeyCode::Up, KeyModifiers::NONE) => {
                if self.current_section == HelpSection::KeyboardShortcuts {
                    self.select_previous();
                }
                crate::ui::UIAction::None
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                if self.current_section == HelpSection::KeyboardShortcuts {
                    self.select_next();
                }
                crate::ui::UIAction::None
            }

            // Section navigation
            (KeyCode::Tab, KeyModifiers::NONE) => {
                let sections = HelpSection::all();
                let current_index = sections
                    .iter()
                    .position(|s| s == &self.current_section)
                    .unwrap_or(0);
                let next_index = (current_index + 1) % sections.len();
                self.switch_section(sections[next_index]);
                crate::ui::UIAction::None
            }
            (KeyCode::BackTab, KeyModifiers::SHIFT) => {
                let sections = HelpSection::all();
                let current_index = sections
                    .iter()
                    .position(|s| s == &self.current_section)
                    .unwrap_or(0);
                let prev_index = if current_index == 0 {
                    sections.len() - 1
                } else {
                    current_index - 1
                };
                self.switch_section(sections[prev_index]);
                crate::ui::UIAction::None
            }

            // Handle text input for search (only in shortcuts section)
            (KeyCode::Char(c), KeyModifiers::NONE)
                if self.current_section == HelpSection::KeyboardShortcuts =>
            {
                self.search_query.push(c);
                self.update_filtered_shortcuts();
                crate::ui::UIAction::None
            }
            (KeyCode::Char(c), KeyModifiers::SHIFT)
                if self.current_section == HelpSection::KeyboardShortcuts =>
            {
                self.search_query.push(c.to_uppercase().next().unwrap_or(c));
                self.update_filtered_shortcuts();
                crate::ui::UIAction::None
            }

            // Backspace for search
            (KeyCode::Backspace, KeyModifiers::NONE)
                if self.current_section == HelpSection::KeyboardShortcuts =>
            {
                self.search_query.pop();
                self.update_filtered_shortcuts();
                crate::ui::UIAction::None
            }

            _ => crate::ui::UIAction::None,
        }
    }

    /// Get current help section
    pub fn current_section(&self) -> &HelpSection {
        &self.current_section
    }

    /// Switch to a different help section
    pub fn switch_section(&mut self, section: HelpSection) {
        self.current_section = section;
        self.search_query.clear();
        self.selected_index = 0;
        if section == HelpSection::KeyboardShortcuts {
            self.update_filtered_shortcuts();
        }
    }

    /// Update search query for shortcuts
    pub fn update_search(&mut self, query: String) {
        self.search_query = query;
        self.selected_index = 0;
        if self.current_section == HelpSection::KeyboardShortcuts {
            self.update_filtered_shortcuts();
        }
    }

    /// Get current search query
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Get filtered shortcuts
    pub fn filtered_shortcuts(&self) -> &[ShortcutSearchResult] {
        &self.filtered_shortcuts
    }

    /// Get selected shortcut index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else if !self.filtered_shortcuts.is_empty() {
            self.selected_index = self.filtered_shortcuts.len() - 1;
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected_index < self.filtered_shortcuts.len().saturating_sub(1) {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    /// Get shortcuts by category
    pub fn get_shortcuts_by_category(&self, category: ShortcutCategory) -> Vec<&KeyboardShortcut> {
        self.shortcuts
            .iter()
            .filter(|s| s.category == category)
            .collect()
    }

    /// Get shortcuts by context
    pub fn get_shortcuts_by_context(&self, context: ShortcutContext) -> Vec<&KeyboardShortcut> {
        self.shortcuts
            .iter()
            .filter(|s| s.context == context)
            .collect()
    }

    /// Get help content for a section
    pub fn get_help_content(&self, section: &HelpSection) -> Option<&Vec<HelpItem>> {
        self.help_sections.get(section)
    }

    /// Get all available help sections
    pub fn get_help_sections(&self) -> Vec<&HelpSection> {
        self.help_sections.keys().collect()
    }

    /// Get contextual help based on current UI state
    pub fn get_contextual_help(&self, context: ShortcutContext) -> Vec<&KeyboardShortcut> {
        self.get_shortcuts_by_context(context)
    }

    /// Add a custom shortcut
    pub fn add_shortcut(&mut self, shortcut: KeyboardShortcut) {
        self.shortcuts.push(shortcut);
        self.update_filtered_shortcuts();
    }

    /// Remove a shortcut by key combination
    pub fn remove_shortcut(&mut self, key_combination: &str) {
        self.shortcuts
            .retain(|s| s.key_combination != key_combination);
        self.update_filtered_shortcuts();
    }

    /// Update filtered shortcuts based on search query
    fn update_filtered_shortcuts(&mut self) {
        if self.search_query.is_empty() {
            // Show all shortcuts when no search query
            self.filtered_shortcuts = self
                .shortcuts
                .iter()
                .map(|s| ShortcutSearchResult {
                    shortcut: s.clone(),
                    score: 100,
                    matched_indices: Vec::new(),
                })
                .collect();
        } else {
            // Fuzzy search through shortcuts
            let mut results = Vec::new();

            for shortcut in &self.shortcuts {
                // Search in key combination, description, and category
                let search_text = format!(
                    "{} {} {:?}",
                    shortcut.key_combination, shortcut.description, shortcut.category
                );

                if let Some((score, indices)) =
                    self.matcher.fuzzy_indices(&search_text, &self.search_query)
                {
                    results.push(ShortcutSearchResult {
                        shortcut: shortcut.clone(),
                        score,
                        matched_indices: indices,
                    });
                }
            }

            // Sort by score (higher is better)
            results.sort_by(|a, b| b.score.cmp(&a.score));
            self.filtered_shortcuts = results;
        }

        // Ensure selected index is valid
        if self.selected_index >= self.filtered_shortcuts.len() {
            self.selected_index = 0;
        }
    }

    /// Initialize default keyboard shortcuts
    fn initialize_shortcuts(&mut self) {
        let shortcuts = vec![
            // Session shortcuts
            KeyboardShortcut {
                key_combination: "Ctrl+N".to_string(),
                description: "Create a new chat session".to_string(),
                category: ShortcutCategory::Session,
                context: ShortcutContext::Global,
                command_id: Some("session.new".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+O".to_string(),
                description: "Open session browser".to_string(),
                category: ShortcutCategory::Session,
                context: ShortcutContext::Global,
                command_id: Some("session.open".to_string()),
            },
            KeyboardShortcut {
                key_combination: "F2".to_string(),
                description: "Rename current session".to_string(),
                category: ShortcutCategory::Session,
                context: ShortcutContext::Chat,
                command_id: Some("session.rename".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Alt+Left".to_string(),
                description: "Navigate to previous session".to_string(),
                category: ShortcutCategory::Session,
                context: ShortcutContext::Global,
                command_id: Some("session.previous".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Alt+Right".to_string(),
                description: "Navigate to next session".to_string(),
                category: ShortcutCategory::Session,
                context: ShortcutContext::Global,
                command_id: Some("session.next".to_string()),
            },
            // Message shortcuts
            KeyboardShortcut {
                key_combination: "Ctrl+F".to_string(),
                description: "Search messages in current session".to_string(),
                category: ShortcutCategory::Message,
                context: ShortcutContext::Chat,
                command_id: Some("message.search".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+Shift+F".to_string(),
                description: "Search messages across all sessions".to_string(),
                category: ShortcutCategory::Message,
                context: ShortcutContext::Global,
                command_id: Some("message.search_global".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+E".to_string(),
                description: "Edit selected message".to_string(),
                category: ShortcutCategory::Message,
                context: ShortcutContext::Chat,
                command_id: Some("message.edit".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Delete".to_string(),
                description: "Delete selected message".to_string(),
                category: ShortcutCategory::Message,
                context: ShortcutContext::Chat,
                command_id: Some("message.delete".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+C".to_string(),
                description: "Copy message content".to_string(),
                category: ShortcutCategory::Message,
                context: ShortcutContext::Chat,
                command_id: Some("message.copy".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+R".to_string(),
                description: "Regenerate AI response".to_string(),
                category: ShortcutCategory::Message,
                context: ShortcutContext::Chat,
                command_id: Some("message.regenerate".to_string()),
            },
            // Navigation shortcuts
            KeyboardShortcut {
                key_combination: "Ctrl+G".to_string(),
                description: "Go to specific message number".to_string(),
                category: ShortcutCategory::Navigation,
                context: ShortcutContext::Chat,
                command_id: Some("navigation.goto_message".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+Home".to_string(),
                description: "Jump to first message".to_string(),
                category: ShortcutCategory::Navigation,
                context: ShortcutContext::Chat,
                command_id: Some("navigation.first_message".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+End".to_string(),
                description: "Jump to last message".to_string(),
                category: ShortcutCategory::Navigation,
                context: ShortcutContext::Chat,
                command_id: Some("navigation.last_message".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Page Up".to_string(),
                description: "Scroll up through messages".to_string(),
                category: ShortcutCategory::Navigation,
                context: ShortcutContext::Chat,
                command_id: None,
            },
            KeyboardShortcut {
                key_combination: "Page Down".to_string(),
                description: "Scroll down through messages".to_string(),
                category: ShortcutCategory::Navigation,
                context: ShortcutContext::Chat,
                command_id: None,
            },
            // View shortcuts
            KeyboardShortcut {
                key_combination: "Ctrl+Plus".to_string(),
                description: "Increase font size".to_string(),
                category: ShortcutCategory::View,
                context: ShortcutContext::Global,
                command_id: Some("view.increase_font".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+Minus".to_string(),
                description: "Decrease font size".to_string(),
                category: ShortcutCategory::View,
                context: ShortcutContext::Global,
                command_id: Some("view.decrease_font".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+0".to_string(),
                description: "Reset font size to default".to_string(),
                category: ShortcutCategory::View,
                context: ShortcutContext::Global,
                command_id: Some("view.reset_font".to_string()),
            },
            KeyboardShortcut {
                key_combination: "F11".to_string(),
                description: "Toggle fullscreen mode".to_string(),
                category: ShortcutCategory::View,
                context: ShortcutContext::Global,
                command_id: Some("view.fullscreen".to_string()),
            },
            // Export shortcuts
            KeyboardShortcut {
                key_combination: "Ctrl+S".to_string(),
                description: "Export current conversation".to_string(),
                category: ShortcutCategory::Export,
                context: ShortcutContext::Chat,
                command_id: Some("export.conversation".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+Shift+S".to_string(),
                description: "Export all conversations".to_string(),
                category: ShortcutCategory::Export,
                context: ShortcutContext::Global,
                command_id: Some("export.all".to_string()),
            },
            // Settings shortcuts
            KeyboardShortcut {
                key_combination: "Ctrl+Comma".to_string(),
                description: "Open application settings".to_string(),
                category: ShortcutCategory::Settings,
                context: ShortcutContext::Global,
                command_id: Some("settings.open".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+P".to_string(),
                description: "Open model settings".to_string(),
                category: ShortcutCategory::Settings,
                context: ShortcutContext::Global,
                command_id: Some("settings.model".to_string()),
            },
            // Help shortcuts
            KeyboardShortcut {
                key_combination: "F1".to_string(),
                description: "Show keyboard shortcuts".to_string(),
                category: ShortcutCategory::Help,
                context: ShortcutContext::Global,
                command_id: Some("help.shortcuts".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Ctrl+?".to_string(),
                description: "Show help overlay".to_string(),
                category: ShortcutCategory::Help,
                context: ShortcutContext::Global,
                command_id: Some("help.overlay".to_string()),
            },
            // General shortcuts
            KeyboardShortcut {
                key_combination: "Ctrl+Shift+P".to_string(),
                description: "Open command palette".to_string(),
                category: ShortcutCategory::General,
                context: ShortcutContext::Global,
                command_id: Some("palette.open".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Escape".to_string(),
                description: "Close dialogs and overlays".to_string(),
                category: ShortcutCategory::General,
                context: ShortcutContext::Global,
                command_id: Some("general.escape".to_string()),
            },
            KeyboardShortcut {
                key_combination: "Enter".to_string(),
                description: "Send message or confirm action".to_string(),
                category: ShortcutCategory::General,
                context: ShortcutContext::Chat,
                command_id: None,
            },
            KeyboardShortcut {
                key_combination: "Shift+Enter".to_string(),
                description: "Add new line in message input".to_string(),
                category: ShortcutCategory::General,
                context: ShortcutContext::Chat,
                command_id: None,
            },
            KeyboardShortcut {
                key_combination: "Ctrl+Q".to_string(),
                description: "Quit application".to_string(),
                category: ShortcutCategory::General,
                context: ShortcutContext::Global,
                command_id: Some("general.quit".to_string()),
            },
        ];

        self.shortcuts = shortcuts;
    }

    /// Initialize help content for different sections
    fn initialize_help_content(&mut self) {
        // Getting Started section
        let getting_started = vec![
            HelpItem {
                title: "Welcome to Ruff".to_string(),
                content: "Ruff is a powerful terminal-based AI chat application that supports multiple AI models and providers.".to_string(),
                subsections: vec![
                    HelpSubsection {
                        title: "First Steps".to_string(),
                        content: "1. Configure your API keys in settings (Ctrl+Comma)\n2. Create a new session (Ctrl+N)\n3. Start chatting with AI models\n4. Use keyboard shortcuts for efficient navigation".to_string(),
                    },
                    HelpSubsection {
                        title: "Basic Usage".to_string(),
                        content: "Type your message and press Enter to send. Use Shift+Enter for multi-line messages. Navigate between sessions with Alt+Left/Right.".to_string(),
                    },
                ],
            },
        ];

        // Features section
        let features = vec![
            HelpItem {
                title: "Session Management".to_string(),
                content: "Organize your conversations with multiple sessions.".to_string(),
                subsections: vec![
                    HelpSubsection {
                        title: "Creating Sessions".to_string(),
                        content: "Press Ctrl+N to create a new session. Sessions are automatically titled based on your first message.".to_string(),
                    },
                    HelpSubsection {
                        title: "Session Navigation".to_string(),
                        content: "Use Alt+Left/Right to switch between sessions, or Ctrl+O to open the session browser.".to_string(),
                    },
                ],
            },
            HelpItem {
                title: "Message Management".to_string(),
                content: "Edit, delete, and search through your messages.".to_string(),
                subsections: vec![
                    HelpSubsection {
                        title: "Editing Messages".to_string(),
                        content: "Right-click on any message to edit, delete, or copy it. Editing a message will regenerate subsequent AI responses.".to_string(),
                    },
                    HelpSubsection {
                        title: "Search Functionality".to_string(),
                        content: "Use Ctrl+F to search within the current session, or Ctrl+Shift+F to search across all sessions.".to_string(),
                    },
                ],
            },
            HelpItem {
                title: "Export and Import".to_string(),
                content: "Save and share your conversations in multiple formats.".to_string(),
                subsections: vec![
                    HelpSubsection {
                        title: "Export Options".to_string(),
                        content: "Export conversations as Markdown, JSON, HTML, or plain text. Includes metadata like timestamps and model information.".to_string(),
                    },
                    HelpSubsection {
                        title: "Import Support".to_string(),
                        content: "Import conversations from ChatGPT, Claude, and other AI chat applications.".to_string(),
                    },
                ],
            },
        ];

        // Configuration section
        let configuration = vec![
            HelpItem {
                title: "API Configuration".to_string(),
                content: "Set up your AI model providers and API keys.".to_string(),
                subsections: vec![
                    HelpSubsection {
                        title: "Supported Providers".to_string(),
                        content: "OpenAI, Anthropic, Google, and custom endpoints are supported. Configure each provider separately.".to_string(),
                    },
                    HelpSubsection {
                        title: "Model Parameters".to_string(),
                        content: "Adjust temperature, max tokens, and other parameters per model. Use Ctrl+P to access model settings.".to_string(),
                    },
                ],
            },
            HelpItem {
                title: "UI Customization".to_string(),
                content: "Personalize your Ruff experience.".to_string(),
                subsections: vec![
                    HelpSubsection {
                        title: "Themes".to_string(),
                        content: "Choose from multiple color schemes including dark, light, and high contrast modes for accessibility.".to_string(),
                    },
                    HelpSubsection {
                        title: "Layout".to_string(),
                        content: "Resize panes by dragging dividers. Adjust font size with Ctrl+Plus/Minus.".to_string(),
                    },
                ],
            },
        ];

        // Troubleshooting section
        let troubleshooting = vec![
            HelpItem {
                title: "Common Issues".to_string(),
                content: "Solutions to frequently encountered problems.".to_string(),
                subsections: vec![
                    HelpSubsection {
                        title: "API Connection Issues".to_string(),
                        content: "Check your API keys, internet connection, and rate limits. Enable debug mode for detailed logs.".to_string(),
                    },
                    HelpSubsection {
                        title: "Performance Issues".to_string(),
                        content: "Large sessions may slow down the application. Consider archiving old sessions or using search to find specific messages.".to_string(),
                    },
                ],
            },
        ];

        // About section
        let about = vec![
            HelpItem {
                title: "About Ruff".to_string(),
                content: "Ruff is an open-source terminal-based AI chat application.".to_string(),
                subsections: vec![
                    HelpSubsection {
                        title: "Version Information".to_string(),
                        content: "Ruff v0.1.0 - Built with Rust and Ratatui for optimal performance and cross-platform compatibility.".to_string(),
                    },
                    HelpSubsection {
                        title: "License".to_string(),
                        content: "Licensed under MIT OR Apache-2.0. Source code available on GitHub.".to_string(),
                    },
                ],
            },
        ];

        // Insert all sections
        self.help_sections
            .insert(HelpSection::GettingStarted, getting_started);
        self.help_sections.insert(HelpSection::Features, features);
        self.help_sections
            .insert(HelpSection::Configuration, configuration);
        self.help_sections
            .insert(HelpSection::Troubleshooting, troubleshooting);
        self.help_sections.insert(HelpSection::About, about);
    }
}

impl Clone for HelpSystem {
    fn clone(&self) -> Self {
        Self {
            shortcuts: self.shortcuts.clone(),
            help_sections: self.help_sections.clone(),
            matcher: SkimMatcherV2::default(),
            is_visible: self.is_visible,
            current_section: self.current_section,
            search_query: self.search_query.clone(),
            filtered_shortcuts: self.filtered_shortcuts.clone(),
            selected_index: self.selected_index,
        }
    }
}

impl Default for HelpSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutCategory {
    /// Get display name for category
    pub fn display_name(&self) -> &'static str {
        match self {
            ShortcutCategory::Session => "Session",
            ShortcutCategory::Message => "Message",
            ShortcutCategory::Navigation => "Navigation",
            ShortcutCategory::View => "View",
            ShortcutCategory::Export => "Export",
            ShortcutCategory::Settings => "Settings",
            ShortcutCategory::Help => "Help",
            ShortcutCategory::General => "General",
        }
    }

    /// Get all categories
    pub fn all() -> Vec<ShortcutCategory> {
        vec![
            ShortcutCategory::Session,
            ShortcutCategory::Message,
            ShortcutCategory::Navigation,
            ShortcutCategory::View,
            ShortcutCategory::Export,
            ShortcutCategory::Settings,
            ShortcutCategory::Help,
            ShortcutCategory::General,
        ]
    }
}

impl ShortcutContext {
    /// Get display name for context
    pub fn display_name(&self) -> &'static str {
        match self {
            ShortcutContext::Global => "Global",
            ShortcutContext::Chat => "Chat",
            ShortcutContext::CommandPalette => "Command Palette",
            ShortcutContext::Dialog => "Dialog",
            ShortcutContext::MessageEdit => "Message Edit",
        }
    }

    /// Get all contexts
    pub fn all() -> Vec<ShortcutContext> {
        vec![
            ShortcutContext::Global,
            ShortcutContext::Chat,
            ShortcutContext::CommandPalette,
            ShortcutContext::Dialog,
            ShortcutContext::MessageEdit,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_system_creation() {
        let help_system = HelpSystem::new();
        assert!(!help_system.is_visible());
        assert_eq!(
            help_system.current_section(),
            &HelpSection::KeyboardShortcuts
        );
        assert_eq!(help_system.search_query(), "");
        assert_eq!(help_system.selected_index(), 0);

        // Should have shortcuts loaded
        assert!(!help_system.shortcuts.is_empty());
        assert!(!help_system.filtered_shortcuts.is_empty());

        // Should have help sections loaded
        assert!(!help_system.help_sections.is_empty());
    }

    #[test]
    fn test_visibility_toggle() {
        let mut help_system = HelpSystem::new();

        assert!(!help_system.is_visible());

        help_system.show();
        assert!(help_system.is_visible());
        assert_eq!(help_system.selected_index(), 0);
        assert_eq!(help_system.search_query(), "");

        help_system.hide();
        assert!(!help_system.is_visible());
    }

    #[test]
    fn test_section_switching() {
        let mut help_system = HelpSystem::new();

        assert_eq!(
            help_system.current_section(),
            &HelpSection::KeyboardShortcuts
        );

        help_system.switch_section(HelpSection::Features);
        assert_eq!(help_system.current_section(), &HelpSection::Features);
        assert_eq!(help_system.search_query(), "");
        assert_eq!(help_system.selected_index(), 0);

        help_system.switch_section(HelpSection::About);
        assert_eq!(help_system.current_section(), &HelpSection::About);
    }

    #[test]
    fn test_shortcut_search() {
        let mut help_system = HelpSystem::new();
        let initial_count = help_system.filtered_shortcuts().len();

        // Test exact match
        help_system.update_search("Ctrl+N".to_string());
        assert!(!help_system.filtered_shortcuts().is_empty());

        let found = help_system
            .filtered_shortcuts()
            .iter()
            .find(|r| r.shortcut.key_combination == "Ctrl+N");
        assert!(found.is_some());

        // Test fuzzy match
        help_system.update_search("new session".to_string());
        let new_session_results: Vec<_> = help_system
            .filtered_shortcuts()
            .iter()
            .filter(|r| {
                r.shortcut.description.to_lowercase().contains("new")
                    && r.shortcut.description.to_lowercase().contains("session")
            })
            .collect();
        assert!(!new_session_results.is_empty());

        // Test clearing search
        help_system.update_search("".to_string());
        assert_eq!(help_system.filtered_shortcuts().len(), initial_count);
    }

    #[test]
    fn test_selection_navigation() {
        let mut help_system = HelpSystem::new();
        help_system.show();

        assert_eq!(help_system.selected_index(), 0);

        // Test moving down
        help_system.select_next();
        assert_eq!(help_system.selected_index(), 1);

        // Test moving up
        help_system.select_previous();
        assert_eq!(help_system.selected_index(), 0);

        // Test wrapping at beginning
        help_system.select_previous();
        assert_eq!(
            help_system.selected_index(),
            help_system.filtered_shortcuts().len() - 1
        );

        // Test wrapping at end
        help_system.select_next();
        assert_eq!(help_system.selected_index(), 0);
    }

    #[test]
    fn test_shortcuts_by_category() {
        let help_system = HelpSystem::new();

        let session_shortcuts = help_system.get_shortcuts_by_category(ShortcutCategory::Session);
        assert!(!session_shortcuts.is_empty());

        for shortcut in session_shortcuts {
            assert_eq!(shortcut.category, ShortcutCategory::Session);
        }

        let message_shortcuts = help_system.get_shortcuts_by_category(ShortcutCategory::Message);
        assert!(!message_shortcuts.is_empty());

        for shortcut in message_shortcuts {
            assert_eq!(shortcut.category, ShortcutCategory::Message);
        }
    }

    #[test]
    fn test_shortcuts_by_context() {
        let help_system = HelpSystem::new();

        let global_shortcuts = help_system.get_shortcuts_by_context(ShortcutContext::Global);
        assert!(!global_shortcuts.is_empty());

        for shortcut in global_shortcuts {
            assert_eq!(shortcut.context, ShortcutContext::Global);
        }

        let chat_shortcuts = help_system.get_shortcuts_by_context(ShortcutContext::Chat);
        assert!(!chat_shortcuts.is_empty());

        for shortcut in chat_shortcuts {
            assert_eq!(shortcut.context, ShortcutContext::Chat);
        }
    }

    #[test]
    fn test_contextual_help() {
        let help_system = HelpSystem::new();

        let chat_help = help_system.get_contextual_help(ShortcutContext::Chat);
        assert!(!chat_help.is_empty());

        // Should include chat-specific shortcuts
        let found_message_search = chat_help.iter().any(|s| s.key_combination == "Ctrl+F");
        assert!(found_message_search);

        let global_help = help_system.get_contextual_help(ShortcutContext::Global);
        assert!(!global_help.is_empty());

        // Should include global shortcuts
        let found_new_session = global_help.iter().any(|s| s.key_combination == "Ctrl+N");
        assert!(found_new_session);
    }

    #[test]
    fn test_help_content() {
        let help_system = HelpSystem::new();

        // Test getting help content for different sections
        let getting_started = help_system.get_help_content(&HelpSection::GettingStarted);
        assert!(getting_started.is_some());
        assert!(!getting_started.unwrap().is_empty());

        let features = help_system.get_help_content(&HelpSection::Features);
        assert!(features.is_some());
        assert!(!features.unwrap().is_empty());

        let about = help_system.get_help_content(&HelpSection::About);
        assert!(about.is_some());
        assert!(!about.unwrap().is_empty());

        // Test getting all sections
        let sections = help_system.get_help_sections();
        assert!(!sections.is_empty());
        assert!(sections.contains(&&HelpSection::GettingStarted));
        assert!(sections.contains(&&HelpSection::Features));
        assert!(sections.contains(&&HelpSection::About));
    }

    #[test]
    fn test_custom_shortcut_management() {
        let mut help_system = HelpSystem::new();
        let initial_count = help_system.shortcuts.len();

        // Add custom shortcut
        let custom_shortcut = KeyboardShortcut {
            key_combination: "Ctrl+T".to_string(),
            description: "Test custom shortcut".to_string(),
            category: ShortcutCategory::General,
            context: ShortcutContext::Global,
            command_id: Some("test.custom".to_string()),
        };

        help_system.add_shortcut(custom_shortcut.clone());
        assert_eq!(help_system.shortcuts.len(), initial_count + 1);

        // Should be able to find the custom shortcut
        let found = help_system
            .shortcuts
            .iter()
            .find(|s| s.key_combination == "Ctrl+T");
        assert!(found.is_some());
        assert_eq!(found.unwrap().description, "Test custom shortcut");

        // Remove custom shortcut
        help_system.remove_shortcut("Ctrl+T");
        assert_eq!(help_system.shortcuts.len(), initial_count);

        // Should no longer find the custom shortcut
        let not_found = help_system
            .shortcuts
            .iter()
            .find(|s| s.key_combination == "Ctrl+T");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_search_result_scoring() {
        let mut help_system = HelpSystem::new();

        // Search for something that should match multiple shortcuts
        help_system.update_search("ctrl".to_string());

        let results = help_system.filtered_shortcuts();
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

    #[test]
    fn test_category_display_names() {
        assert_eq!(ShortcutCategory::Session.display_name(), "Session");
        assert_eq!(ShortcutCategory::Message.display_name(), "Message");
        assert_eq!(ShortcutCategory::Navigation.display_name(), "Navigation");
        assert_eq!(ShortcutCategory::View.display_name(), "View");
        assert_eq!(ShortcutCategory::Export.display_name(), "Export");
        assert_eq!(ShortcutCategory::Settings.display_name(), "Settings");
        assert_eq!(ShortcutCategory::Help.display_name(), "Help");
        assert_eq!(ShortcutCategory::General.display_name(), "General");
    }

    #[test]
    fn test_context_display_names() {
        assert_eq!(ShortcutContext::Global.display_name(), "Global");
        assert_eq!(ShortcutContext::Chat.display_name(), "Chat");
        assert_eq!(
            ShortcutContext::CommandPalette.display_name(),
            "Command Palette"
        );
        assert_eq!(ShortcutContext::Dialog.display_name(), "Dialog");
        assert_eq!(ShortcutContext::MessageEdit.display_name(), "Message Edit");
    }

    #[test]
    fn test_section_display_names() {
        assert_eq!(
            HelpSection::KeyboardShortcuts.display_name(),
            "Keyboard Shortcuts"
        );
        assert_eq!(
            HelpSection::GettingStarted.display_name(),
            "Getting Started"
        );
        assert_eq!(HelpSection::Features.display_name(), "Features");
        assert_eq!(HelpSection::Configuration.display_name(), "Configuration");
        assert_eq!(
            HelpSection::Troubleshooting.display_name(),
            "Troubleshooting"
        );
        assert_eq!(HelpSection::About.display_name(), "About");
    }

    #[test]
    fn test_all_categories() {
        let all_categories = ShortcutCategory::all();
        assert_eq!(all_categories.len(), 8);
        assert!(all_categories.contains(&ShortcutCategory::Session));
        assert!(all_categories.contains(&ShortcutCategory::Message));
        assert!(all_categories.contains(&ShortcutCategory::Navigation));
        assert!(all_categories.contains(&ShortcutCategory::View));
        assert!(all_categories.contains(&ShortcutCategory::Export));
        assert!(all_categories.contains(&ShortcutCategory::Settings));
        assert!(all_categories.contains(&ShortcutCategory::Help));
        assert!(all_categories.contains(&ShortcutCategory::General));
    }

    #[test]
    fn test_all_contexts() {
        let all_contexts = ShortcutContext::all();
        assert_eq!(all_contexts.len(), 5);
        assert!(all_contexts.contains(&ShortcutContext::Global));
        assert!(all_contexts.contains(&ShortcutContext::Chat));
        assert!(all_contexts.contains(&ShortcutContext::CommandPalette));
        assert!(all_contexts.contains(&ShortcutContext::Dialog));
        assert!(all_contexts.contains(&ShortcutContext::MessageEdit));
    }

    #[test]
    fn test_all_sections() {
        let all_sections = HelpSection::all();
        assert_eq!(all_sections.len(), 6);
        assert!(all_sections.contains(&HelpSection::KeyboardShortcuts));
        assert!(all_sections.contains(&HelpSection::GettingStarted));
        assert!(all_sections.contains(&HelpSection::Features));
        assert!(all_sections.contains(&HelpSection::Configuration));
        assert!(all_sections.contains(&HelpSection::Troubleshooting));
        assert!(all_sections.contains(&HelpSection::About));
    }

    #[test]
    fn test_help_content_structure() {
        let help_system = HelpSystem::new();

        // Test that help items have proper structure
        if let Some(getting_started) = help_system.get_help_content(&HelpSection::GettingStarted) {
            for item in getting_started {
                assert!(!item.title.is_empty());
                assert!(!item.content.is_empty());

                // Test subsections
                for subsection in &item.subsections {
                    assert!(!subsection.title.is_empty());
                    assert!(!subsection.content.is_empty());
                }
            }
        }
    }

    #[test]
    fn test_shortcut_command_ids() {
        let help_system = HelpSystem::new();

        // Test that important shortcuts have command IDs
        let new_session = help_system
            .shortcuts
            .iter()
            .find(|s| s.key_combination == "Ctrl+N");
        assert!(new_session.is_some());
        assert!(new_session.unwrap().command_id.is_some());
        assert_eq!(
            new_session.unwrap().command_id.as_ref().unwrap(),
            "session.new"
        );

        let search_messages = help_system
            .shortcuts
            .iter()
            .find(|s| s.key_combination == "Ctrl+F");
        assert!(search_messages.is_some());
        assert!(search_messages.unwrap().command_id.is_some());
        assert_eq!(
            search_messages.unwrap().command_id.as_ref().unwrap(),
            "message.search"
        );
    }
}

impl HelpSection {
    /// Get display name for section
    pub fn display_name(&self) -> &'static str {
        match self {
            HelpSection::KeyboardShortcuts => "Keyboard Shortcuts",
            HelpSection::GettingStarted => "Getting Started",
            HelpSection::Features => "Features",
            HelpSection::Configuration => "Configuration",
            HelpSection::Troubleshooting => "Troubleshooting",
            HelpSection::About => "About",
        }
    }

    /// Get all sections
    pub fn all() -> Vec<HelpSection> {
        vec![
            HelpSection::KeyboardShortcuts,
            HelpSection::GettingStarted,
            HelpSection::Features,
            HelpSection::Configuration,
            HelpSection::Troubleshooting,
            HelpSection::About,
        ]
    }
}
