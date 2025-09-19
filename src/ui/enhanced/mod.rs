//! Enhanced UI components module
//! 
//! This module provides modern UI enhancements including:
//! - Markdown rendering with syntax highlighting
//! - Resizable layout management
//! - Command palette and help system
//! - Theme management and accessibility features

pub mod accessibility;
pub mod markdown;
pub mod syntax;
pub mod layout;
pub mod palette;
pub mod parameters;
pub mod help;
pub mod system_prompt;
pub mod themes;
pub mod clipboard;
pub mod navigation;

pub use accessibility::{AccessibilityManager, AriaRole, AccessibilityValidation, WcagLevel, ElementAccessibilityInfo};
pub use markdown::MarkdownRenderer;
pub use syntax::SyntaxHighlighter;
pub use layout::{LayoutManager, ResizablePane};
pub use palette::{CommandPalette, Command};
pub use parameters::{ParameterAdjustment, ParameterMode};
pub use help::HelpSystem;
pub use system_prompt::{SystemPromptEditor, SystemPromptMode};
pub use themes::{ThemeService, Theme, ThemeColors, ThemeStyles};
pub use clipboard::ClipboardManager;
pub use navigation::{NavigationManager, NavigationResult, NavigationAction, NavigationDirection};