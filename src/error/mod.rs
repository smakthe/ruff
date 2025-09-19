//! Error handling module
//! 
//! This module provides comprehensive error handling including:
//! - Enhanced error types with context
//! - Error recovery mechanisms
//! - Error categorization and severity levels
//! - Plugin error isolation

pub mod enhanced_error;

#[cfg(test)]
mod tests;

// Re-export the original RuffError for backward compatibility
use thiserror::Error;

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

#[derive(Error, Debug)]
pub enum RuffError {
    #[error("Configuration error: {0}")]
    Config(#[from] confy::ConfyError),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("API error: {message}")]
    Api { message: String },
    
    #[error("Invalid API key for model: {model}")]
    InvalidApiKey { model: String },
    
    #[error("Model not supported: {model}")]
    UnsupportedModel { model: String },
    
    #[error("Rate limit exceeded for model: {model}")]
    RateLimit { model: String },
    
    #[error("Token limit exceeded. Input: {input_tokens}, Output: {output_tokens}, Max: {max_tokens}")]
    TokenLimit {
        input_tokens: u32,
        output_tokens: u32,
        max_tokens: u32,
    },
    
    #[error("Application error: {0}")]
    App(String),
    
    #[error("Session not found: {session_id}")]
    SessionNotFound { session_id: String },
    
    #[error("Message not found: {message_id} in session {session_id}")]
    MessageNotFound { session_id: String, message_id: String },
    
    #[error("Export failed: {format:?} - {reason}")]
    ExportFailed { format: String, reason: String },
    
    #[error("Import failed: {reason}")]
    ImportFailed { reason: String },
    
    #[error("Import validation failed: {errors:?}")]
    ImportValidationFailed { errors: Vec<String> },
    
    #[error("Unsupported import format: {format}")]
    UnsupportedImportFormat { format: String },
    
    #[error("Import file error: {path} - {reason}")]
    ImportFileError { path: String, reason: String },
    
    #[error("Invalid export format: {format}")]
    InvalidExportFormat { format: String },
    
    #[error("Export file not found: {path}")]
    ExportFileNotFound { path: String },
    
    #[error("Configuration validation failed: {field} - {message}")]
    ConfigValidation { field: String, message: String },
    
    #[error("Theme error: {theme_name} - {message}")]
    Theme { theme_name: String, message: String },
    
    #[error("Plugin error in {plugin_name}: {message}")]
    Plugin { plugin_name: String, message: String },
    
    // New enhanced error variant
    #[error("Enhanced error: {0}")]
    Enhanced(#[from] EnhancedError),
}

impl From<anyhow::Error> for RuffError {
    fn from(error: anyhow::Error) -> Self {
        RuffError::App(error.to_string())
    }
}

impl RuffError {
    /// Convert to enhanced error
    pub fn to_enhanced(&self) -> EnhancedError {
        let category = match self {
            RuffError::Config(_) => ErrorCategory::Configuration,
            RuffError::Network(_) => ErrorCategory::Network,
            RuffError::Serialization(_) => ErrorCategory::Parsing,
            RuffError::Io(_) => ErrorCategory::Storage,
            RuffError::Api { .. } => ErrorCategory::Network,
            RuffError::InvalidApiKey { .. } => ErrorCategory::Auth,
            RuffError::UnsupportedModel { .. } => ErrorCategory::Configuration,
            RuffError::RateLimit { .. } => ErrorCategory::Network,
            RuffError::TokenLimit { .. } => ErrorCategory::Performance,
            RuffError::App(_) => ErrorCategory::Unknown,
            RuffError::SessionNotFound { .. } => ErrorCategory::Session,
            RuffError::MessageNotFound { .. } => ErrorCategory::Message,
            RuffError::ExportFailed { .. } => ErrorCategory::Storage,
            RuffError::ImportFailed { .. } => ErrorCategory::Storage,
            RuffError::ImportValidationFailed { .. } => ErrorCategory::Parsing,
            RuffError::UnsupportedImportFormat { .. } => ErrorCategory::Configuration,
            RuffError::ImportFileError { .. } => ErrorCategory::Storage,
            RuffError::InvalidExportFormat { .. } => ErrorCategory::Configuration,
            RuffError::ExportFileNotFound { .. } => ErrorCategory::Storage,
            RuffError::ConfigValidation { .. } => ErrorCategory::Configuration,
            RuffError::Theme { .. } => ErrorCategory::UI,
            RuffError::Plugin { .. } => ErrorCategory::Plugin,
            RuffError::Enhanced(enhanced) => return enhanced.clone(),
        };

        let severity = match self {
            RuffError::Config(_) => ErrorSeverity::Error,
            RuffError::Network(_) => ErrorSeverity::Error,
            RuffError::Serialization(_) => ErrorSeverity::Error,
            RuffError::Io(_) => ErrorSeverity::Error,
            RuffError::Api { .. } => ErrorSeverity::Error,
            RuffError::InvalidApiKey { .. } => ErrorSeverity::Critical,
            RuffError::UnsupportedModel { .. } => ErrorSeverity::Error,
            RuffError::RateLimit { .. } => ErrorSeverity::Warning,
            RuffError::TokenLimit { .. } => ErrorSeverity::Warning,
            RuffError::App(_) => ErrorSeverity::Error,
            RuffError::SessionNotFound { .. } => ErrorSeverity::Error,
            RuffError::MessageNotFound { .. } => ErrorSeverity::Error,
            RuffError::ExportFailed { .. } => ErrorSeverity::Error,
            RuffError::ImportFailed { .. } => ErrorSeverity::Error,
            RuffError::ImportValidationFailed { .. } => ErrorSeverity::Warning,
            RuffError::UnsupportedImportFormat { .. } => ErrorSeverity::Error,
            RuffError::ImportFileError { .. } => ErrorSeverity::Error,
            RuffError::InvalidExportFormat { .. } => ErrorSeverity::Error,
            RuffError::ExportFileNotFound { .. } => ErrorSeverity::Error,
            RuffError::ConfigValidation { .. } => ErrorSeverity::Error,
            RuffError::Theme { .. } => ErrorSeverity::Warning,
            RuffError::Plugin { .. } => ErrorSeverity::Error,
            RuffError::Enhanced(enhanced) => enhanced.severity.clone(),
        };

        EnhancedError::new(category, severity, self.to_string())
    }
}