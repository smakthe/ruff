//! Enhanced UI components module
//!
//! This module provides modern UI enhancements including:
//! - Markdown rendering with syntax highlighting
//! - Resizable layout management
//! - Command palette and help system
//! - Theme management and accessibility features

pub mod accessibility;
pub mod clipboard;
pub mod help;
pub mod layout;
pub mod markdown;
pub mod navigation;
pub mod palette;
pub mod parameters;
pub mod syntax;
pub mod system_prompt;
pub mod themes;

pub use accessibility::{
    AccessibilityManager, AccessibilityValidation, AriaRole, ElementAccessibilityInfo, WcagLevel,
};
pub use clipboard::ClipboardManager;
pub use help::HelpSystem;
pub use layout::{LayoutManager, ResizablePane};
pub use markdown::MarkdownRenderer;
pub use navigation::{NavigationAction, NavigationDirection, NavigationManager, NavigationResult};
pub use palette::{Command, CommandPalette};
pub use parameters::{ParameterAdjustment, ParameterMode};
pub use syntax::SyntaxHighlighter;
pub use system_prompt::{SystemPromptEditor, SystemPromptMode};
pub use themes::{Theme, ThemeColors, ThemeService, ThemeStyles};
