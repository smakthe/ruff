//! Message management module
//! 
//! This module provides advanced message handling capabilities including:
//! - Message CRUD operations (edit, delete, copy)
//! - Message search and indexing
//! - Message regeneration and versioning
//! - Message threading support

pub mod manager;
pub mod search;
pub mod operations;
pub mod threading;

pub use manager::{MessageManager, MessageStore};
pub use search::{MessageSearchIndex, MessageSearchResult};
pub use operations::{
    MessageOperations, MessageVersion, MessageStats, MessageOperation, MessageOperationResult
};
pub use threading::{
    MessageThread, ThreadManager, ThreadRelationship, ThreadStatistics
};