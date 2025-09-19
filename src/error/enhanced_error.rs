//! Enhanced error handling with detailed context and recovery mechanisms
//! 
//! This module provides comprehensive error handling capabilities including:
//! - Detailed error context and stack traces
//! - Error recovery mechanisms
//! - Error categorization and severity levels
//! - Plugin error isolation

use std::collections::HashMap;
use std::fmt;
use std::error::Error as StdError;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::events::{SessionId, MessageId, PluginId};

/// Enhanced error type with detailed context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedError {
    /// Unique error ID for tracking
    pub id: Uuid,
    /// Error category
    pub category: ErrorCategory,
    /// Error severity level
    pub severity: ErrorSeverity,
    /// Main error message
    pub message: String,
    /// Detailed error description
    pub details: Option<String>,
    /// Error context information
    pub context: ErrorContext,
    /// Timestamp when error occurred
    pub timestamp: DateTime<Local>,
    /// Suggested recovery actions
    pub recovery_suggestions: Vec<RecoveryAction>,
    /// Whether this error can be retried
    pub is_retryable: bool,
    /// Number of retry attempts made
    pub retry_count: u32,
    /// Maximum retry attempts allowed
    pub max_retries: u32,
    /// Related error IDs (for error chains)
    pub related_errors: Vec<Uuid>,
    /// User-facing error message
    pub user_message: Option<String>,
}

/// Error category for classification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Network and API related errors
    Network,
    /// File system and storage errors
    Storage,
    /// Configuration and validation errors
    Configuration,
    /// Session management errors
    Session,
    /// Message handling errors
    Message,
    /// Search and indexing errors
    Search,
    /// Plugin system errors
    Plugin,
    /// UI and rendering errors
    UI,
    /// Authentication and authorization errors
    Auth,
    /// Data parsing and serialization errors
    Parsing,
    /// Performance and resource errors
    Performance,
    /// Unknown or uncategorized errors
    Unknown,
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Informational - not really an error
    Info,
    /// Warning - something unexpected but recoverable
    Warning,
    /// Error - operation failed but application can continue
    Error,
    /// Critical - serious error that affects core functionality
    Critical,
    /// Fatal - application cannot continue
    Fatal,
}

/// Error context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Session ID if error is session-related
    pub session_id: Option<SessionId>,
    /// Message ID if error is message-related
    pub message_id: Option<MessageId>,
    /// Plugin ID if error is plugin-related
    pub plugin_id: Option<PluginId>,
    /// Operation being performed when error occurred
    pub operation: Option<String>,
    /// Component where error occurred
    pub component: Option<String>,
    /// Additional context data
    pub metadata: HashMap<String, String>,
    /// Stack trace if available
    pub stack_trace: Option<String>,
    /// Source file and line number
    pub source_location: Option<SourceLocation>,
}

/// Source code location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
    pub function: Option<String>,
}

/// Recovery action suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAction {
    /// Action description
    pub description: String,
    /// Action type
    pub action_type: RecoveryActionType,
    /// Whether this action can be automated
    pub is_automated: bool,
    /// Priority of this recovery action
    pub priority: u32,
    /// Additional parameters for the action
    pub parameters: HashMap<String, String>,
}

/// Types of recovery actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryActionType {
    /// Retry the failed operation
    Retry,
    /// Reload configuration
    ReloadConfig,
    /// Restart component
    RestartComponent,
    /// Clear cache
    ClearCache,
    /// Reset to default state
    ResetToDefault,
    /// Switch to fallback option
    UseFallback,
    /// Manual intervention required
    ManualIntervention,
    /// Ignore and continue
    Ignore,
}

/// Error recovery manager
pub struct ErrorRecoveryManager {
    /// Recovery strategies by error category
    recovery_strategies: HashMap<ErrorCategory, Vec<RecoveryStrategy>>,
    /// Error history for pattern analysis
    error_history: Vec<EnhancedError>,
    /// Maximum errors to keep in history
    max_history_size: usize,
    /// Recovery attempt tracking
    recovery_attempts: HashMap<Uuid, Vec<RecoveryAttempt>>,
}

/// Recovery strategy definition
#[derive(Debug, Clone)]
pub struct RecoveryStrategy {
    /// Strategy name
    pub name: String,
    /// Conditions when this strategy applies
    pub conditions: Vec<RecoveryCondition>,
    /// Actions to take
    pub actions: Vec<RecoveryAction>,
    /// Maximum attempts for this strategy
    pub max_attempts: u32,
    /// Cooldown period between attempts
    pub cooldown_seconds: u64,
}

/// Conditions for applying recovery strategies
#[derive(Debug, Clone)]
pub enum RecoveryCondition {
    /// Error message contains specific text
    MessageContains(String),
    /// Error occurred in specific component
    ComponentEquals(String),
    /// Error severity level
    SeverityLevel(ErrorSeverity),
    /// Retry count is below threshold
    RetryCountBelow(u32),
    /// Time since last occurrence
    TimeSinceLastOccurrence(u64),
}

/// Recovery attempt record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    /// Attempt ID
    pub id: Uuid,
    /// Error ID being recovered from
    pub error_id: Uuid,
    /// Recovery action attempted
    pub action: RecoveryAction,
    /// Timestamp of attempt
    pub timestamp: DateTime<Local>,
    /// Whether the recovery was successful
    pub success: bool,
    /// Result message
    pub result_message: Option<String>,
}

impl EnhancedError {
    /// Create a new enhanced error
    pub fn new(category: ErrorCategory, severity: ErrorSeverity, message: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            category,
            severity,
            message,
            details: None,
            context: ErrorContext::default(),
            timestamp: Local::now(),
            recovery_suggestions: Vec::new(),
            is_retryable: false,
            retry_count: 0,
            max_retries: 3,
            related_errors: Vec::new(),
            user_message: None,
        }
    }

    /// Add detailed description
    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }

    /// Add context information
    pub fn with_context(mut self, context: ErrorContext) -> Self {
        self.context = context;
        self
    }

    /// Add session context
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.context.session_id = Some(session_id);
        self
    }

    /// Add message context
    pub fn with_message(mut self, message_id: MessageId) -> Self {
        self.context.message_id = Some(message_id);
        self
    }

    /// Add plugin context
    pub fn with_plugin(mut self, plugin_id: PluginId) -> Self {
        self.context.plugin_id = Some(plugin_id);
        self
    }

    /// Add operation context
    pub fn with_operation(mut self, operation: String) -> Self {
        self.context.operation = Some(operation);
        self
    }

    /// Add component context
    pub fn with_component(mut self, component: String) -> Self {
        self.context.component = Some(component);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.context.metadata.insert(key, value);
        self
    }

    /// Add stack trace
    pub fn with_stack_trace(mut self, stack_trace: String) -> Self {
        self.context.stack_trace = Some(stack_trace);
        self
    }

    /// Add source location
    pub fn with_source_location(mut self, location: SourceLocation) -> Self {
        self.context.source_location = Some(location);
        self
    }

    /// Mark as retryable
    pub fn retryable(mut self, max_retries: u32) -> Self {
        self.is_retryable = true;
        self.max_retries = max_retries;
        self
    }

    /// Add recovery suggestion
    pub fn with_recovery_suggestion(mut self, action: RecoveryAction) -> Self {
        self.recovery_suggestions.push(action);
        self
    }

    /// Add user-facing message
    pub fn with_user_message(mut self, message: String) -> Self {
        self.user_message = Some(message);
        self
    }

    /// Link to related error
    pub fn with_related_error(mut self, error_id: Uuid) -> Self {
        self.related_errors.push(error_id);
        self
    }

    /// Check if error can be retried
    pub fn can_retry(&self) -> bool {
        self.is_retryable && self.retry_count < self.max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// Get user-friendly error message
    pub fn user_friendly_message(&self) -> String {
        self.user_message.clone().unwrap_or_else(|| {
            match self.category {
                ErrorCategory::Network => "Network connection issue. Please check your internet connection and try again.".to_string(),
                ErrorCategory::Storage => "File system error. Please check disk space and permissions.".to_string(),
                ErrorCategory::Configuration => "Configuration error. Please check your settings.".to_string(),
                ErrorCategory::Session => "Session error. Please try creating a new session.".to_string(),
                ErrorCategory::Message => "Message processing error. Please try again.".to_string(),
                ErrorCategory::Search => "Search error. Please try a different search query.".to_string(),
                ErrorCategory::Plugin => "Plugin error. The plugin may need to be reloaded.".to_string(),
                ErrorCategory::UI => "Display error. Please try refreshing the interface.".to_string(),
                ErrorCategory::Auth => "Authentication error. Please check your credentials.".to_string(),
                ErrorCategory::Parsing => "Data format error. The data may be corrupted.".to_string(),
                ErrorCategory::Performance => "Performance issue. The system may be under heavy load.".to_string(),
                ErrorCategory::Unknown => "An unexpected error occurred. Please try again.".to_string(),
            }
        })
    }

    /// Convert to JSON for logging
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Create from standard error
    pub fn from_std_error<E: StdError>(error: E, category: ErrorCategory) -> Self {
        let mut enhanced_error = Self::new(
            category,
            ErrorSeverity::Error,
            error.to_string(),
        );

        // Add source chain information
        let mut source = error.source();
        let mut details = Vec::new();
        while let Some(err) = source {
            details.push(err.to_string());
            source = err.source();
        }

        if !details.is_empty() {
            enhanced_error = enhanced_error.with_details(details.join(" -> "));
        }

        enhanced_error
    }
}

impl ErrorRecoveryManager {
    /// Create a new error recovery manager
    pub fn new() -> Self {
        let mut manager = Self {
            recovery_strategies: HashMap::new(),
            error_history: Vec::new(),
            max_history_size: 1000,
            recovery_attempts: HashMap::new(),
        };

        manager.initialize_default_strategies();
        manager
    }

    /// Initialize default recovery strategies
    fn initialize_default_strategies(&mut self) {
        // Network error recovery
        self.add_recovery_strategy(
            ErrorCategory::Network,
            RecoveryStrategy {
                name: "Network Retry".to_string(),
                conditions: vec![
                    RecoveryCondition::SeverityLevel(ErrorSeverity::Error),
                    RecoveryCondition::RetryCountBelow(3),
                ],
                actions: vec![
                    RecoveryAction {
                        description: "Wait and retry network operation".to_string(),
                        action_type: RecoveryActionType::Retry,
                        is_automated: true,
                        priority: 1,
                        parameters: HashMap::from([
                            ("delay_seconds".to_string(), "5".to_string()),
                        ]),
                    },
                ],
                max_attempts: 3,
                cooldown_seconds: 5,
            },
        );

        // Plugin error recovery
        self.add_recovery_strategy(
            ErrorCategory::Plugin,
            RecoveryStrategy {
                name: "Plugin Isolation".to_string(),
                conditions: vec![
                    RecoveryCondition::SeverityLevel(ErrorSeverity::Critical),
                ],
                actions: vec![
                    RecoveryAction {
                        description: "Disable problematic plugin".to_string(),
                        action_type: RecoveryActionType::RestartComponent,
                        is_automated: true,
                        priority: 1,
                        parameters: HashMap::new(),
                    },
                ],
                max_attempts: 1,
                cooldown_seconds: 0,
            },
        );

        // Configuration error recovery
        self.add_recovery_strategy(
            ErrorCategory::Configuration,
            RecoveryStrategy {
                name: "Config Reset".to_string(),
                conditions: vec![
                    RecoveryCondition::MessageContains("invalid".to_string()),
                ],
                actions: vec![
                    RecoveryAction {
                        description: "Reset to default configuration".to_string(),
                        action_type: RecoveryActionType::ResetToDefault,
                        is_automated: false,
                        priority: 2,
                        parameters: HashMap::new(),
                    },
                ],
                max_attempts: 1,
                cooldown_seconds: 0,
            },
        );
    }

    /// Add a recovery strategy
    pub fn add_recovery_strategy(&mut self, category: ErrorCategory, strategy: RecoveryStrategy) {
        self.recovery_strategies
            .entry(category)
            .or_insert_with(Vec::new)
            .push(strategy);
    }

    /// Record an error in history
    pub fn record_error(&mut self, error: EnhancedError) {
        self.error_history.push(error);

        // Trim history if it exceeds maximum size
        if self.error_history.len() > self.max_history_size {
            self.error_history.remove(0);
        }
    }

    /// Attempt to recover from an error
    pub async fn attempt_recovery(&mut self, error: &EnhancedError) -> Vec<RecoveryAttempt> {
        let mut attempts = Vec::new();

        if let Some(strategies) = self.recovery_strategies.get(&error.category) {
            for strategy in strategies {
                if self.strategy_applies(strategy, error) {
                    let attempt = self.execute_recovery_strategy(strategy, error).await;
                    attempts.push(attempt);

                    // If recovery was successful, stop trying other strategies
                    if attempts.last().unwrap().success {
                        break;
                    }
                }
            }
        }

        // Record attempts
        self.recovery_attempts.insert(error.id, attempts.clone());

        attempts
    }

    /// Check if a recovery strategy applies to an error
    fn strategy_applies(&self, strategy: &RecoveryStrategy, error: &EnhancedError) -> bool {
        strategy.conditions.iter().all(|condition| {
            match condition {
                RecoveryCondition::MessageContains(text) => {
                    error.message.contains(text) || 
                    error.details.as_ref().map_or(false, |d| d.contains(text))
                }
                RecoveryCondition::ComponentEquals(component) => {
                    error.context.component.as_ref() == Some(component)
                }
                RecoveryCondition::SeverityLevel(level) => {
                    error.severity >= *level
                }
                RecoveryCondition::RetryCountBelow(max) => {
                    error.retry_count < *max
                }
                RecoveryCondition::TimeSinceLastOccurrence(seconds) => {
                    // Check if enough time has passed since last similar error
                    let cutoff = Local::now() - chrono::Duration::seconds(*seconds as i64);
                    !self.error_history.iter().any(|e| {
                        e.category == error.category && 
                        e.message == error.message && 
                        e.timestamp > cutoff
                    })
                }
            }
        })
    }

    /// Execute a recovery strategy
    async fn execute_recovery_strategy(
        &self,
        strategy: &RecoveryStrategy,
        error: &EnhancedError,
    ) -> RecoveryAttempt {
        let attempt_id = Uuid::new_v4();
        let mut success = false;
        let mut result_message = None;

        // Execute each action in the strategy
        for action in &strategy.actions {
            match self.execute_recovery_action(action, error).await {
                Ok(message) => {
                    success = true;
                    result_message = Some(message);
                    break; // Success, no need to try other actions
                }
                Err(e) => {
                    result_message = Some(format!("Recovery action failed: {}", e));
                }
            }
        }

        RecoveryAttempt {
            id: attempt_id,
            error_id: error.id,
            action: strategy.actions.first().cloned().unwrap_or_else(|| {
                RecoveryAction {
                    description: "No action".to_string(),
                    action_type: RecoveryActionType::Ignore,
                    is_automated: false,
                    priority: 0,
                    parameters: HashMap::new(),
                }
            }),
            timestamp: Local::now(),
            success,
            result_message,
        }
    }

    /// Execute a specific recovery action
    async fn execute_recovery_action(
        &self,
        action: &RecoveryAction,
        _error: &EnhancedError,
    ) -> Result<String, Box<dyn StdError + Send + Sync>> {
        match action.action_type {
            RecoveryActionType::Retry => {
                // Add delay if specified
                if let Some(delay_str) = action.parameters.get("delay_seconds") {
                    if let Ok(delay) = delay_str.parse::<u64>() {
                        tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                    }
                }
                Ok("Retry delay completed".to_string())
            }
            RecoveryActionType::ReloadConfig => {
                // This would trigger a config reload in the actual implementation
                Ok("Configuration reload triggered".to_string())
            }
            RecoveryActionType::RestartComponent => {
                // This would restart the specified component
                Ok("Component restart triggered".to_string())
            }
            RecoveryActionType::ClearCache => {
                // This would clear relevant caches
                Ok("Cache cleared".to_string())
            }
            RecoveryActionType::ResetToDefault => {
                // This would reset to default state
                Ok("Reset to default state".to_string())
            }
            RecoveryActionType::UseFallback => {
                // This would switch to fallback option
                Ok("Switched to fallback option".to_string())
            }
            RecoveryActionType::ManualIntervention => {
                Err("Manual intervention required".into())
            }
            RecoveryActionType::Ignore => {
                Ok("Error ignored".to_string())
            }
        }
    }

    /// Get error statistics
    pub fn get_error_statistics(&self) -> ErrorStatistics {
        let mut category_counts = HashMap::new();
        let mut severity_counts = HashMap::new();
        let mut recent_errors = 0;

        let one_hour_ago = Local::now() - chrono::Duration::hours(1);

        for error in &self.error_history {
            *category_counts.entry(error.category.clone()).or_insert(0) += 1;
            *severity_counts.entry(error.severity.clone()).or_insert(0) += 1;

            if error.timestamp > one_hour_ago {
                recent_errors += 1;
            }
        }

        ErrorStatistics {
            total_errors: self.error_history.len(),
            recent_errors,
            category_counts,
            severity_counts,
            recovery_success_rate: self.calculate_recovery_success_rate(),
        }
    }

    /// Calculate recovery success rate
    fn calculate_recovery_success_rate(&self) -> f64 {
        let total_attempts: usize = self.recovery_attempts.values().map(|v| v.len()).sum();
        if total_attempts == 0 {
            return 0.0;
        }

        let successful_attempts: usize = self.recovery_attempts
            .values()
            .flatten()
            .filter(|attempt| attempt.success)
            .count();

        successful_attempts as f64 / total_attempts as f64
    }
}

/// Error statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStatistics {
    pub total_errors: usize,
    pub recent_errors: usize,
    pub category_counts: HashMap<ErrorCategory, usize>,
    pub severity_counts: HashMap<ErrorSeverity, usize>,
    pub recovery_success_rate: f64,
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self {
            session_id: None,
            message_id: None,
            plugin_id: None,
            operation: None,
            component: None,
            metadata: HashMap::new(),
            stack_trace: None,
            source_location: None,
        }
    }
}

impl fmt::Display for EnhancedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.category, self.message)?;
        
        if let Some(details) = &self.details {
            write!(f, " - {}", details)?;
        }
        
        if let Some(operation) = &self.context.operation {
            write!(f, " (during {})", operation)?;
        }
        
        Ok(())
    }
}

impl StdError for EnhancedError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        None
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Network => write!(f, "Network"),
            ErrorCategory::Storage => write!(f, "Storage"),
            ErrorCategory::Configuration => write!(f, "Configuration"),
            ErrorCategory::Session => write!(f, "Session"),
            ErrorCategory::Message => write!(f, "Message"),
            ErrorCategory::Search => write!(f, "Search"),
            ErrorCategory::Plugin => write!(f, "Plugin"),
            ErrorCategory::UI => write!(f, "UI"),
            ErrorCategory::Auth => write!(f, "Auth"),
            ErrorCategory::Parsing => write!(f, "Parsing"),
            ErrorCategory::Performance => write!(f, "Performance"),
            ErrorCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Info => write!(f, "INFO"),
            ErrorSeverity::Warning => write!(f, "WARN"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
            ErrorSeverity::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Macro for creating enhanced errors with source location
#[macro_export]
macro_rules! enhanced_error {
    ($category:expr, $severity:expr, $message:expr) => {
        $crate::error::enhanced_error::EnhancedError::new($category, $severity, $message.to_string())
            .with_source_location($crate::error::enhanced_error::SourceLocation {
                file: file!().to_string(),
                line: line!(),
                column: Some(column!()),
                function: None,
            })
    };
    ($category:expr, $severity:expr, $message:expr, $($key:expr => $value:expr),+) => {
        {
            let mut error = $crate::error::enhanced_error::EnhancedError::new($category, $severity, $message.to_string())
                .with_source_location($crate::error::enhanced_error::SourceLocation {
                    file: file!().to_string(),
                    line: line!(),
                    column: Some(column!()),
                    function: None,
                });
            $(
                error = error.with_metadata($key.to_string(), $value.to_string());
            )+
            error
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_error_creation() {
        let error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Connection failed".to_string(),
        );

        assert_eq!(error.category, ErrorCategory::Network);
        assert_eq!(error.severity, ErrorSeverity::Error);
        assert_eq!(error.message, "Connection failed");
        assert!(!error.is_retryable);
        assert_eq!(error.retry_count, 0);
    }

    #[test]
    fn test_enhanced_error_builder() {
        let session_id = Uuid::new_v4();
        let error = EnhancedError::new(
            ErrorCategory::Session,
            ErrorSeverity::Warning,
            "Session warning".to_string(),
        )
        .with_session(session_id)
        .with_operation("create_session".to_string())
        .with_metadata("user_id".to_string(), "123".to_string())
        .retryable(5);

        assert_eq!(error.context.session_id, Some(session_id));
        assert_eq!(error.context.operation, Some("create_session".to_string()));
        assert_eq!(error.context.metadata.get("user_id"), Some(&"123".to_string()));
        assert!(error.is_retryable);
        assert_eq!(error.max_retries, 5);
    }

    #[test]
    fn test_error_retry_logic() {
        let mut error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Network error".to_string(),
        ).retryable(3);

        assert!(error.can_retry());
        
        error.increment_retry();
        assert_eq!(error.retry_count, 1);
        assert!(error.can_retry());
        
        error.increment_retry();
        error.increment_retry();
        assert_eq!(error.retry_count, 3);
        assert!(!error.can_retry());
    }

    #[tokio::test]
    async fn test_error_recovery_manager() {
        let mut manager = ErrorRecoveryManager::new();
        
        let error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Connection timeout".to_string(),
        );

        let attempts = manager.attempt_recovery(&error).await;
        assert!(!attempts.is_empty());
        
        // Should have attempted network retry strategy
        assert_eq!(attempts[0].action.action_type, RecoveryActionType::Retry);
    }

    #[test]
    fn test_error_statistics() {
        let mut manager = ErrorRecoveryManager::new();
        
        // Add some test errors
        manager.record_error(EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Error 1".to_string(),
        ));
        
        manager.record_error(EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Warning,
            "Error 2".to_string(),
        ));
        
        manager.record_error(EnhancedError::new(
            ErrorCategory::Plugin,
            ErrorSeverity::Critical,
            "Error 3".to_string(),
        ));

        let stats = manager.get_error_statistics();
        assert_eq!(stats.total_errors, 3);
        assert_eq!(stats.category_counts.get(&ErrorCategory::Network), Some(&2));
        assert_eq!(stats.category_counts.get(&ErrorCategory::Plugin), Some(&1));
        assert_eq!(stats.severity_counts.get(&ErrorSeverity::Error), Some(&1));
        assert_eq!(stats.severity_counts.get(&ErrorSeverity::Warning), Some(&1));
        assert_eq!(stats.severity_counts.get(&ErrorSeverity::Critical), Some(&1));
    }

    #[test]
    fn test_user_friendly_messages() {
        let error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Connection failed".to_string(),
        );

        let user_message = error.user_friendly_message();
        assert!(user_message.contains("Network connection issue"));

        let custom_error = EnhancedError::new(
            ErrorCategory::Unknown,
            ErrorSeverity::Error,
            "Unknown error".to_string(),
        ).with_user_message("Custom user message".to_string());

        assert_eq!(custom_error.user_friendly_message(), "Custom user message");
    }
}