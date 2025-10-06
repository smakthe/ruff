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

#[cfg(test)]
mod tests;

pub use enhanced_error::{
    EnhancedError,
    ErrorCategory,
    ErrorSeverity,
    ErrorContext,
    ErrorRecoveryManager,
    RecoveryAction,
    RecoveryActionType,
    ErrorStatistics,
};

pub use recovery::{
    RecoveryStrategy,
    retry_with_backoff,
    retry_simple,
};

// Type alias for convenience
pub type Result<T> = std::result::Result<T, EnhancedError>;