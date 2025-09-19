//! Comprehensive error handling tests
//! 
//! This module contains tests for all error handling scenarios including:
//! - Enhanced error creation and manipulation
//! - Error recovery mechanisms
//! - Plugin error isolation
//! - Logging integration

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use uuid::Uuid;

use crate::error::{
    EnhancedError, ErrorCategory, ErrorSeverity, ErrorRecoveryManager,
    RecoveryAction, RecoveryActionType,
};
use crate::error::enhanced_error::SourceLocation;
use crate::logging::{StructuredLogger, LoggerConfig, LogLevel, ConsoleOutput, ConsoleFormat};
use crate::plugin::{PluginErrorIsolation, PluginErrorEvent, IsolationConfig};
use crate::events::EventBus;

/// Test enhanced error creation and manipulation
#[cfg(test)]
mod enhanced_error_tests {
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
        assert!(error.recovery_suggestions.is_empty());
    }

    #[test]
    fn test_enhanced_error_builder_pattern() {
        let session_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        
        let error = EnhancedError::new(
            ErrorCategory::Session,
            ErrorSeverity::Warning,
            "Session warning".to_string(),
        )
        .with_session(session_id)
        .with_message(message_id)
        .with_operation("create_session".to_string())
        .with_component("session_manager".to_string())
        .with_metadata("user_id".to_string(), "123".to_string())
        .with_details("Additional error details".to_string())
        .retryable(5)
        .with_user_message("Something went wrong with your session".to_string())
        .with_recovery_suggestion(RecoveryAction {
            description: "Try creating a new session".to_string(),
            action_type: RecoveryActionType::Retry,
            is_automated: false,
            priority: 1,
            parameters: HashMap::new(),
        });

        assert_eq!(error.context.session_id, Some(session_id));
        assert_eq!(error.context.message_id, Some(message_id));
        assert_eq!(error.context.operation, Some("create_session".to_string()));
        assert_eq!(error.context.component, Some("session_manager".to_string()));
        assert_eq!(error.context.metadata.get("user_id"), Some(&"123".to_string()));
        assert_eq!(error.details, Some("Additional error details".to_string()));
        assert!(error.is_retryable);
        assert_eq!(error.max_retries, 5);
        assert_eq!(error.user_message, Some("Something went wrong with your session".to_string()));
        assert_eq!(error.recovery_suggestions.len(), 1);
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

    #[test]
    fn test_error_with_source_location() {
        let error = EnhancedError::new(
            ErrorCategory::Parsing,
            ErrorSeverity::Error,
            "Parse error".to_string(),
        ).with_source_location(SourceLocation {
            file: "test.rs".to_string(),
            line: 42,
            column: Some(10),
            function: Some("parse_data".to_string()),
        });

        assert!(error.context.source_location.is_some());
        let location = error.context.source_location.unwrap();
        assert_eq!(location.file, "test.rs");
        assert_eq!(location.line, 42);
        assert_eq!(location.column, Some(10));
        assert_eq!(location.function, Some("parse_data".to_string()));
    }

    #[test]
    fn test_error_chain_relationships() {
        let parent_error_id = Uuid::new_v4();
        let child_error = EnhancedError::new(
            ErrorCategory::Storage,
            ErrorSeverity::Error,
            "Child error".to_string(),
        ).with_related_error(parent_error_id);

        assert_eq!(child_error.related_errors.len(), 1);
        assert_eq!(child_error.related_errors[0], parent_error_id);
    }

    #[test]
    fn test_user_friendly_messages() {
        let network_error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Connection timeout".to_string(),
        );

        let user_message = network_error.user_friendly_message();
        assert!(user_message.contains("Network connection issue"));

        let custom_error = EnhancedError::new(
            ErrorCategory::Unknown,
            ErrorSeverity::Error,
            "Unknown error".to_string(),
        ).with_user_message("Custom user message".to_string());

        assert_eq!(custom_error.user_friendly_message(), "Custom user message");
    }

    #[test]
    fn test_error_serialization() {
        let error = EnhancedError::new(
            ErrorCategory::Configuration,
            ErrorSeverity::Warning,
            "Config warning".to_string(),
        ).with_details("Invalid configuration value".to_string());

        let json = error.to_json().unwrap();
        assert!(json.contains("Configuration"));
        assert!(json.contains("Config warning"));
        assert!(json.contains("Invalid configuration value"));

        // Test deserialization
        let deserialized: EnhancedError = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.category, ErrorCategory::Configuration);
        assert_eq!(deserialized.message, "Config warning");
        assert_eq!(deserialized.details, Some("Invalid configuration value".to_string()));
    }
}

/// Test error recovery mechanisms
#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[tokio::test]
    async fn test_error_recovery_manager_creation() {
        let manager = ErrorRecoveryManager::new();
        let stats = manager.get_error_statistics();
        
        assert_eq!(stats.total_errors, 0);
        assert_eq!(stats.recent_errors, 0);
        assert_eq!(stats.recovery_success_rate, 0.0);
    }

    #[tokio::test]
    async fn test_error_recording() {
        let mut manager = ErrorRecoveryManager::new();
        
        let error1 = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Network error 1".to_string(),
        );
        
        let error2 = EnhancedError::new(
            ErrorCategory::Plugin,
            ErrorSeverity::Critical,
            "Plugin error 1".to_string(),
        );

        manager.record_error(error1);
        manager.record_error(error2);

        let stats = manager.get_error_statistics();
        assert_eq!(stats.total_errors, 2);
        assert_eq!(stats.category_counts.get(&ErrorCategory::Network), Some(&1));
        assert_eq!(stats.category_counts.get(&ErrorCategory::Plugin), Some(&1));
        assert_eq!(stats.severity_counts.get(&ErrorSeverity::Error), Some(&1));
        assert_eq!(stats.severity_counts.get(&ErrorSeverity::Critical), Some(&1));
    }

    #[tokio::test]
    async fn test_recovery_attempt() {
        let mut manager = ErrorRecoveryManager::new();
        
        let network_error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Connection timeout".to_string(),
        );

        let attempts = manager.attempt_recovery(&network_error).await;
        assert!(!attempts.is_empty());
        
        // Should have attempted network retry strategy
        assert_eq!(attempts[0].action.action_type, RecoveryActionType::Retry);
    }

    #[tokio::test]
    async fn test_plugin_error_recovery() {
        let mut manager = ErrorRecoveryManager::new();
        
        let plugin_error = EnhancedError::new(
            ErrorCategory::Plugin,
            ErrorSeverity::Critical,
            "Plugin crashed".to_string(),
        ).with_plugin("test_plugin".to_string());

        let attempts = manager.attempt_recovery(&plugin_error).await;
        assert!(!attempts.is_empty());
        
        // Should have attempted plugin isolation strategy
        assert_eq!(attempts[0].action.action_type, RecoveryActionType::RestartComponent);
    }

    #[tokio::test]
    async fn test_configuration_error_recovery() {
        let mut manager = ErrorRecoveryManager::new();
        
        let config_error = EnhancedError::new(
            ErrorCategory::Configuration,
            ErrorSeverity::Error,
            "Invalid configuration detected".to_string(),
        );

        let attempts = manager.attempt_recovery(&config_error).await;
        assert!(!attempts.is_empty());
        
        // Should suggest config reset
        assert_eq!(attempts[0].action.action_type, RecoveryActionType::ResetToDefault);
    }
}

/// Test plugin error isolation
#[cfg(test)]
mod plugin_isolation_tests {
    use super::*;

    fn create_test_isolation() -> PluginErrorIsolation {
        let event_bus = EventBus::new();
        let logger = Arc::new(RwLock::new(StructuredLogger::new(LoggerConfig::default())));
        let config = IsolationConfig::default();
        
        PluginErrorIsolation::new(event_bus, logger, config)
    }

    #[test]
    fn test_plugin_registration() {
        let isolation = create_test_isolation();
        let plugin_id = "test_plugin".to_string();
        
        isolation.register_plugin(plugin_id.clone());
        
        let health = isolation.get_plugin_health(&plugin_id);
        assert!(health.is_some());
        
        let health = health.unwrap();
        assert_eq!(health.plugin_id, plugin_id);
        assert_eq!(health.status, crate::plugin::HealthStatus::Healthy);
        assert_eq!(health.error_count, 0);
        assert_eq!(health.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_plugin_error_reporting() {
        let isolation = create_test_isolation();
        let plugin_id = "test_plugin".to_string();
        
        isolation.register_plugin(plugin_id.clone());
        
        let error_event = PluginErrorEvent {
            plugin_id: plugin_id.clone(),
            error: EnhancedError::new(
                ErrorCategory::Plugin,
                ErrorSeverity::Error,
                "Plugin operation failed".to_string(),
            ),
            operation: "test_operation".to_string(),
            response_time: Some(Duration::from_millis(500)),
            context: HashMap::new(),
        };
        
        let result = isolation.report_error(error_event).await;
        assert!(result.is_ok());
        
        let health = isolation.get_plugin_health(&plugin_id).unwrap();
        assert_eq!(health.error_count, 1);
        assert_eq!(health.total_errors, 1);
        assert_eq!(health.consecutive_failures, 1);
        assert_eq!(health.performance.failed_operations, 1);
        assert_eq!(health.performance.total_operations, 1);
    }

    #[tokio::test]
    async fn test_plugin_success_reporting() {
        let isolation = create_test_isolation();
        let plugin_id = "test_plugin".to_string();
        
        isolation.register_plugin(plugin_id.clone());
        
        let result = isolation.report_success(&plugin_id, Duration::from_millis(200)).await;
        assert!(result.is_ok());
        
        let health = isolation.get_plugin_health(&plugin_id).unwrap();
        assert_eq!(health.consecutive_failures, 0);
        assert_eq!(health.performance.successful_operations, 1);
        assert_eq!(health.performance.total_operations, 1);
        assert_eq!(health.performance.success_rate, 1.0);
        assert_eq!(health.performance.avg_response_time, 200.0);
        assert_eq!(health.performance.min_response_time, 200);
        assert_eq!(health.performance.max_response_time, 200);
    }

    #[tokio::test]
    async fn test_plugin_health_degradation() {
        let isolation = create_test_isolation();
        let plugin_id = "test_plugin".to_string();
        
        isolation.register_plugin(plugin_id.clone());
        
        // Report multiple errors to trigger health degradation
        for i in 0..5 {
            let error_event = PluginErrorEvent {
                plugin_id: plugin_id.clone(),
                error: EnhancedError::new(
                    ErrorCategory::Plugin,
                    ErrorSeverity::Error,
                    format!("Plugin error {}", i),
                ),
                operation: "test_operation".to_string(),
                response_time: Some(Duration::from_millis(1000)),
                context: HashMap::new(),
            };
            
            isolation.report_error(error_event).await.unwrap();
        }
        
        let health = isolation.get_plugin_health(&plugin_id).unwrap();
        assert_eq!(health.error_count, 5);
        assert_eq!(health.consecutive_failures, 5);
        
        // Should be quarantined due to consecutive failures
        assert_eq!(health.status, crate::plugin::HealthStatus::Quarantined);
    }

    #[test]
    fn test_plugin_unregistration() {
        let isolation = create_test_isolation();
        let plugin_id = "test_plugin".to_string();
        
        isolation.register_plugin(plugin_id.clone());
        assert!(isolation.get_plugin_health(&plugin_id).is_some());
        
        isolation.unregister_plugin(&plugin_id);
        assert!(isolation.get_plugin_health(&plugin_id).is_none());
    }

    #[test]
    fn test_multiple_plugin_health_tracking() {
        let isolation = create_test_isolation();
        let plugin1 = "plugin1".to_string();
        let plugin2 = "plugin2".to_string();
        
        isolation.register_plugin(plugin1.clone());
        isolation.register_plugin(plugin2.clone());
        
        let all_health = isolation.get_all_plugin_health();
        assert_eq!(all_health.len(), 2);
        assert!(all_health.contains_key(&plugin1));
        assert!(all_health.contains_key(&plugin2));
        
        assert!(isolation.is_plugin_healthy(&plugin1));
        assert!(isolation.is_plugin_healthy(&plugin2));
    }
}

/// Test logging integration
#[cfg(test)]
mod logging_integration_tests {
    use super::*;
    use crate::logging::{ConsoleOutput, ConsoleFormat, LevelFilter};

    #[test]
    fn test_error_logging_integration() {
        let mut logger = StructuredLogger::new(LoggerConfig::default());
        logger.add_output(Box::new(ConsoleOutput::new(false, ConsoleFormat::Json)));
        logger.add_filter(Box::new(LevelFilter::new(LogLevel::Debug)));
        
        let error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Network connection failed".to_string(),
        )
        .with_operation("api_request".to_string())
        .with_metadata("endpoint".to_string(), "https://api.example.com".to_string());
        
        // Should not panic
        logger.log_error(&error);
    }

    #[test]
    fn test_performance_logging() {
        let mut logger = StructuredLogger::new(LoggerConfig::default());
        logger.add_output(Box::new(ConsoleOutput::new(false, ConsoleFormat::Human)));
        
        // Should not panic
        logger.log_performance("test_operation", 150, None);
        
        let metrics = logger.get_metrics();
        assert_eq!(metrics.operation_timings.get("test_operation"), Some(&vec![150]));
    }

    #[test]
    fn test_structured_logging_with_context() {
        let mut logger = StructuredLogger::new(LoggerConfig::default());
        logger.add_output(Box::new(ConsoleOutput::new(false, ConsoleFormat::Compact)));
        
        let session_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        
        // Should not panic
        logger.log_with_context(
            LogLevel::Info,
            "test_component",
            "Test message with context",
            Some(session_id),
            Some(message_id),
            Some("test_plugin".to_string()),
        );
    }
}

/// Integration tests for complete error handling workflows
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_error_handling_workflow() {
        // Set up components
        let event_bus = EventBus::new();
        let logger = Arc::new(RwLock::new(StructuredLogger::new(LoggerConfig::default())));
        let isolation = PluginErrorIsolation::new(
            event_bus.clone(),
            Arc::clone(&logger),
            IsolationConfig::default(),
        );
        let mut recovery_manager = ErrorRecoveryManager::new();
        
        // Register plugin
        let plugin_id = "integration_test_plugin".to_string();
        isolation.register_plugin(plugin_id.clone());
        
        // Create an error
        let error = EnhancedError::new(
            ErrorCategory::Plugin,
            ErrorSeverity::Error,
            "Integration test error".to_string(),
        )
        .with_plugin(plugin_id.clone())
        .with_operation("integration_test".to_string())
        .retryable(3);
        
        // Log the error
        {
            let mut logger_guard = logger.write().unwrap();
            logger_guard.log_error(&error);
        }
        
        // Record error for recovery
        recovery_manager.record_error(error.clone());
        
        // Report error to isolation system
        let error_event = PluginErrorEvent {
            plugin_id: plugin_id.clone(),
            error: error.clone(),
            operation: "integration_test".to_string(),
            response_time: Some(Duration::from_millis(300)),
            context: HashMap::new(),
        };
        
        let result = isolation.report_error(error_event).await;
        assert!(result.is_ok());
        
        // Attempt recovery
        let recovery_attempts = recovery_manager.attempt_recovery(&error).await;
        assert!(!recovery_attempts.is_empty());
        
        // Verify plugin health was updated
        let health = isolation.get_plugin_health(&plugin_id).unwrap();
        assert_eq!(health.error_count, 1);
        assert_eq!(health.total_errors, 1);
        
        // Verify recovery manager statistics
        let stats = recovery_manager.get_error_statistics();
        assert_eq!(stats.total_errors, 1);
        assert_eq!(stats.category_counts.get(&ErrorCategory::Plugin), Some(&1));
        
        // Verify logging metrics
        let metrics = {
            let logger_guard = logger.read().unwrap();
            logger_guard.get_metrics()
        };
        assert_eq!(metrics.log_counts.get(&LogLevel::Error), Some(&1));
    }

    #[tokio::test]
    async fn test_error_escalation_workflow() {
        let event_bus = EventBus::new();
        let logger = Arc::new(RwLock::new(StructuredLogger::new(LoggerConfig::default())));
        let isolation = PluginErrorIsolation::new(
            event_bus,
            Arc::clone(&logger),
            IsolationConfig {
                max_consecutive_failures: 2, // Lower threshold for testing
                ..Default::default()
            },
        );
        
        let plugin_id = "escalation_test_plugin".to_string();
        isolation.register_plugin(plugin_id.clone());
        
        // Report multiple errors to trigger escalation
        for i in 0..3 {
            let error = EnhancedError::new(
                ErrorCategory::Plugin,
                ErrorSeverity::Error,
                format!("Escalation test error {}", i),
            );
            
            let error_event = PluginErrorEvent {
                plugin_id: plugin_id.clone(),
                error,
                operation: "escalation_test".to_string(),
                response_time: Some(Duration::from_millis(500)),
                context: HashMap::new(),
            };
            
            isolation.report_error(error_event).await.unwrap();
        }
        
        // Plugin should be quarantined
        let health = isolation.get_plugin_health(&plugin_id).unwrap();
        assert_eq!(health.status, crate::plugin::HealthStatus::Quarantined);
        assert_eq!(health.consecutive_failures, 3);
        
        // Should appear in quarantined plugins list
        let quarantined = isolation.get_quarantined_plugins();
        assert!(quarantined.contains_key(&plugin_id));
    }
}

/// Stress tests for error handling under load
#[cfg(test)]
mod stress_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn test_concurrent_error_reporting() {
        let event_bus = EventBus::new();
        let logger = Arc::new(RwLock::new(StructuredLogger::new(LoggerConfig::default())));
        let isolation = Arc::new(PluginErrorIsolation::new(
            event_bus,
            logger,
            IsolationConfig::default(),
        ));
        
        let plugin_id = "stress_test_plugin".to_string();
        isolation.register_plugin(plugin_id.clone());
        
        let error_count = Arc::new(AtomicUsize::new(0));
        let mut join_set = JoinSet::new();
        
        // Spawn multiple tasks reporting errors concurrently
        for i in 0..100 {
            let isolation_clone = Arc::clone(&isolation);
            let plugin_id_clone = plugin_id.clone();
            let error_count_clone = Arc::clone(&error_count);
            
            join_set.spawn(async move {
                let error = EnhancedError::new(
                    ErrorCategory::Plugin,
                    ErrorSeverity::Warning,
                    format!("Concurrent error {}", i),
                );
                
                let error_event = PluginErrorEvent {
                    plugin_id: plugin_id_clone,
                    error,
                    operation: "stress_test".to_string(),
                    response_time: Some(Duration::from_millis(100)),
                    context: HashMap::new(),
                };
                
                if isolation_clone.report_error(error_event).await.is_ok() {
                    error_count_clone.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
        
        // Wait for all tasks to complete
        while let Some(result) = join_set.join_next().await {
            result.unwrap();
        }
        
        // Verify all errors were processed
        assert_eq!(error_count.load(Ordering::SeqCst), 100);
        
        let health = isolation.get_plugin_health(&plugin_id).unwrap();
        assert_eq!(health.total_errors, 100);
    }

    #[tokio::test]
    async fn test_high_volume_logging() {
        let mut logger = StructuredLogger::new(LoggerConfig::default());
        logger.add_output(Box::new(ConsoleOutput::new(false, ConsoleFormat::Json)));
        
        let logger = Arc::new(RwLock::new(logger));
        let mut join_set = JoinSet::new();
        
        // Spawn multiple tasks logging concurrently
        for i in 0..1000 {
            let logger_clone = Arc::clone(&logger);
            
            join_set.spawn(async move {
                let error = EnhancedError::new(
                    ErrorCategory::Performance,
                    ErrorSeverity::Info,
                    format!("High volume log entry {}", i),
                );
                
                let mut logger_guard = logger_clone.write().unwrap();
                logger_guard.log_error(&error);
            });
        }
        
        // Wait for all tasks to complete
        while let Some(result) = join_set.join_next().await {
            result.unwrap();
        }
        
        // Verify metrics were updated
        let metrics = {
            let logger_guard = logger.read().unwrap();
            logger_guard.get_metrics()
        };
        
        assert_eq!(metrics.log_counts.get(&LogLevel::Info), Some(&1000));
    }
}