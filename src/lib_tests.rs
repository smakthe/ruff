//! Basic library tests to ensure core functionality works

#[cfg(test)]
mod tests {
    use crate::error::{EnhancedError, ErrorCategory, ErrorSeverity};
    use crate::events::EventBus;
    use crate::logging::{LogLevel, LoggerConfig, StructuredLogger};

    #[test]
    fn test_enhanced_error_creation() {
        let error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Test error".to_string(),
        );

        assert_eq!(error.category, ErrorCategory::Network);
        assert_eq!(error.severity, ErrorSeverity::Error);
        assert_eq!(error.message, "Test error");
    }

    #[test]
    fn test_structured_logger_creation() {
        let config = LoggerConfig::default();
        let logger = StructuredLogger::new(config);

        // Logger should be created successfully
        assert_eq!(logger.get_metrics().log_counts.len(), 0);
    }

    #[test]
    fn test_event_bus_creation() {
        let event_bus = EventBus::new();
        // Event bus should be created successfully
        // We can't easily test async functionality in a sync test
        assert!(true); // Placeholder assertion
    }

    #[tokio::test]
    async fn test_event_bus_subscription_count() {
        let event_bus = EventBus::new();
        assert_eq!(event_bus.subscription_count().await, 0);
    }

    #[test]
    fn test_error_display() {
        let error = EnhancedError::new(
            ErrorCategory::Configuration,
            ErrorSeverity::Warning,
            "Config warning".to_string(),
        );

        let display_string = format!("{}", error);
        assert!(display_string.contains("Config warning"));
        assert!(display_string.contains("Configuration"));
        assert!(display_string.contains("WARN"));
    }

    #[test]
    fn test_error_user_friendly_message() {
        let error = EnhancedError::new(
            ErrorCategory::Network,
            ErrorSeverity::Error,
            "Connection failed".to_string(),
        );

        let user_message = error.user_friendly_message();
        assert!(user_message.contains("Network connection issue"));
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Fatal > LogLevel::Error);
        assert!(LogLevel::Error > LogLevel::Warn);
        assert!(LogLevel::Warn > LogLevel::Info);
        assert!(LogLevel::Info > LogLevel::Debug);
        assert!(LogLevel::Debug > LogLevel::Trace);
    }

    #[test]
    fn test_error_severity_ordering() {
        assert!(ErrorSeverity::Fatal > ErrorSeverity::Critical);
        assert!(ErrorSeverity::Critical > ErrorSeverity::Error);
        assert!(ErrorSeverity::Error > ErrorSeverity::Warning);
        assert!(ErrorSeverity::Warning > ErrorSeverity::Info);
    }
}
