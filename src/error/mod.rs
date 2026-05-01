//! Error handling module
//!
//! This module provides comprehensive error handling including:
//! - Enhanced error types with context and metadata
//! - Automatic error recovery mechanisms
//! - Error categorization and severity levels
//! - Plugin error isolation
//! - Rich error context (stack traces, operation info, component tracking)
//! - User-friendly error messages
//! - Automatic retries and recovery suggestions

pub mod enhanced_error;
pub mod recovery;

#[cfg(any())]
mod tests;

pub use enhanced_error::{
    EnhancedError, ErrorCategory, ErrorContext, ErrorRecoveryManager, ErrorSeverity,
    ErrorStatistics, RecoveryAction, RecoveryActionType,
};

pub use recovery::{retry_simple, retry_with_backoff, RecoveryStrategy};

// Type alias for convenience
pub type Result<T> = std::result::Result<T, EnhancedError>;
