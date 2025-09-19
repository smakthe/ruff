//! Session management module
//! 
//! This module provides enhanced session management capabilities including:
//! - Multi-session support with switching
//! - Session search and filtering
//! - Session metadata management
//! - Session persistence and loading

pub mod manager;
pub mod search;
pub mod metadata;
pub mod system_prompt;
pub mod lazy_loading;

pub use manager::SessionManager;
pub use search::{SessionSearchIndex, SessionSearchResult};
pub use metadata::{SessionMetadata, SessionTab};
pub use system_prompt::{SystemPromptManager, SystemPromptTemplate, SystemPromptCategory};
pub use lazy_loading::{LazySessionLoader, LazyMessageLoader, SessionMetadata as LazySessionMetadata};