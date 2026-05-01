//! Session management module
//!
//! This module provides enhanced session management capabilities including:
//! - Multi-session support with switching
//! - Session search and filtering
//! - Session metadata management
//! - Session persistence and loading

pub mod lazy_loading;
pub mod manager;
pub mod metadata;
pub mod search;
pub mod system_prompt;

pub use lazy_loading::{
    LazyMessageLoader, LazySessionLoader, SessionMetadata as LazySessionMetadata,
};
pub use manager::SessionManager;
pub use metadata::{SessionMetadata, SessionTab};
pub use search::{SessionSearchIndex, SessionSearchResult};
pub use system_prompt::{SystemPromptCategory, SystemPromptManager, SystemPromptTemplate};
