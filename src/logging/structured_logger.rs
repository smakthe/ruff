//! Structured logging system with configurable levels and outputs
//! 
//! This module provides comprehensive logging capabilities including:
//! - Structured logging with JSON output
//! - Configurable log levels and filters
//! - Multiple output destinations
//! - Performance metrics logging
//! - Error correlation and tracking

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::events::{SessionId, MessageId, PluginId};
use crate::error::enhanced_error::{EnhancedError, ErrorCategory, ErrorSeverity};

/// Structured logger with multiple outputs and filtering
pub struct StructuredLogger {
    /// Logger configuration
    config: LoggerConfig,
    /// Log outputs
    outputs: Vec<Box<dyn LogOutput + Send + Sync>>,
    /// Log filters
    filters: Vec<Box<dyn LogFilter + Send + Sync>>,
    /// Performance metrics collector
    metrics_collector: Arc<Mutex<MetricsCollector>>,
    /// Error correlation tracker
    error_tracker: Arc<Mutex<ErrorTracker>>,
}

/// Logger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerConfig {
    /// Minimum log level to output
    pub min_level: LogLevel,
    /// Whether to include timestamps
    pub include_timestamp: bool,
    /// Whether to include source location
    pub include_source: bool,
    /// Whether to include thread information
    pub include_thread: bool,
    /// Maximum log message length
    pub max_message_length: usize,
    /// Whether to enable performance logging
    pub enable_performance_logging: bool,
    /// Whether to enable error correlation
    pub enable_error_correlation: bool,
    /// Log rotation settings
    pub rotation: Option<LogRotationConfig>,
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    /// Maximum file size in bytes
    pub max_file_size: u64,
    /// Maximum number of backup files
    pub max_backup_files: u32,
    /// Whether to compress old files
    pub compress_old_files: bool,
}

/// Log levels in order of severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

/// Structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique log entry ID
    pub id: Uuid,
    /// Log level
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Timestamp
    pub timestamp: DateTime<Local>,
    /// Logger name/component
    pub logger: String,
    /// Thread ID
    pub thread_id: Option<String>,
    /// Source location
    pub source: Option<SourceLocation>,
    /// Structured fields
    pub fields: HashMap<String, Value>,
    /// Session context
    pub session_id: Option<SessionId>,
    /// Message context
    pub message_id: Option<MessageId>,
    /// Plugin context
    pub plugin_id: Option<PluginId>,
    /// Error correlation ID
    pub correlation_id: Option<Uuid>,
    /// Performance metrics
    pub metrics: Option<PerformanceMetrics>,
}

/// Source location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub function: Option<String>,
}

/// Performance metrics for log entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Operation duration in milliseconds
    pub duration_ms: u64,
    /// Memory usage in bytes
    pub memory_usage: Option<u64>,
    /// CPU usage percentage
    pub cpu_usage: Option<f64>,
    /// Custom performance counters
    pub counters: HashMap<String, u64>,
}

/// Log output trait
pub trait LogOutput {
    /// Write a log entry
    fn write(&mut self, entry: &LogEntry) -> Result<(), io::Error>;
    /// Flush any buffered output
    fn flush(&mut self) -> Result<(), io::Error>;
    /// Close the output
    fn close(&mut self) -> Result<(), io::Error>;
}

/// Log filter trait
pub trait LogFilter {
    /// Check if a log entry should be output
    fn should_log(&self, entry: &LogEntry) -> bool;
}

/// Console log output
pub struct ConsoleOutput {
    /// Whether to use colored output
    colored: bool,
    /// Output format
    format: ConsoleFormat,
}

/// Console output format
#[derive(Debug, Clone)]
pub enum ConsoleFormat {
    /// Human-readable format
    Human,
    /// JSON format
    Json,
    /// Compact format
    Compact,
}

/// File log output
pub struct FileOutput {
    /// File writer
    writer: Box<dyn Write + Send>,
    /// File path
    file_path: PathBuf,
    /// Current file size
    current_size: u64,
    /// Rotation config
    rotation_config: Option<LogRotationConfig>,
}

/// JSON log output
pub struct JsonOutput {
    /// Output writer
    writer: Box<dyn Write + Send>,
}

/// Level-based log filter
pub struct LevelFilter {
    /// Minimum level to allow
    min_level: LogLevel,
}

/// Component-based log filter
pub struct ComponentFilter {
    /// Allowed components
    allowed_components: Vec<String>,
    /// Blocked components
    blocked_components: Vec<String>,
}

/// Performance metrics collector
pub struct MetricsCollector {
    /// Operation timings
    operation_timings: HashMap<String, Vec<u64>>,
    /// Error counts by category
    error_counts: HashMap<ErrorCategory, u64>,
    /// Log counts by level
    log_counts: HashMap<LogLevel, u64>,
    /// Memory usage samples
    memory_samples: Vec<(DateTime<Local>, u64)>,
}

/// Error correlation tracker
pub struct ErrorTracker {
    /// Error chains
    error_chains: HashMap<Uuid, Vec<Uuid>>,
    /// Error contexts
    error_contexts: HashMap<Uuid, ErrorContext>,
}

/// Error context for correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub session_id: Option<SessionId>,
    pub operation: Option<String>,
    pub component: Option<String>,
    pub timestamp: DateTime<Local>,
}

impl StructuredLogger {
    /// Create a new structured logger
    pub fn new(config: LoggerConfig) -> Self {
        Self {
            config,
            outputs: Vec::new(),
            filters: Vec::new(),
            metrics_collector: Arc::new(Mutex::new(MetricsCollector::new())),
            error_tracker: Arc::new(Mutex::new(ErrorTracker::new())),
        }
    }

    /// Add a log output
    pub fn add_output(&mut self, output: Box<dyn LogOutput + Send + Sync>) {
        self.outputs.push(output);
    }

    /// Add a log filter
    pub fn add_filter(&mut self, filter: Box<dyn LogFilter + Send + Sync>) {
        self.filters.push(filter);
    }

    /// Log a message with the specified level
    pub fn log(&mut self, level: LogLevel, logger: &str, message: &str) {
        let entry = LogEntry {
            id: Uuid::new_v4(),
            level,
            message: self.truncate_message(message),
            timestamp: Local::now(),
            logger: logger.to_string(),
            thread_id: if self.config.include_thread {
                Some(format!("{:?}", std::thread::current().id()))
            } else {
                None
            },
            source: None,
            fields: HashMap::new(),
            session_id: None,
            message_id: None,
            plugin_id: None,
            correlation_id: None,
            metrics: None,
        };

        self.write_entry(entry);
    }

    /// Log with structured fields
    pub fn log_with_fields(
        &mut self,
        level: LogLevel,
        logger: &str,
        message: &str,
        fields: HashMap<String, Value>,
    ) {
        let entry = LogEntry {
            id: Uuid::new_v4(),
            level,
            message: self.truncate_message(message),
            timestamp: Local::now(),
            logger: logger.to_string(),
            thread_id: if self.config.include_thread {
                Some(format!("{:?}", std::thread::current().id()))
            } else {
                None
            },
            source: None,
            fields,
            session_id: None,
            message_id: None,
            plugin_id: None,
            correlation_id: None,
            metrics: None,
        };

        self.write_entry(entry);
    }

    /// Log with context
    pub fn log_with_context(
        &mut self,
        level: LogLevel,
        logger: &str,
        message: &str,
        session_id: Option<SessionId>,
        message_id: Option<MessageId>,
        plugin_id: Option<PluginId>,
    ) {
        let entry = LogEntry {
            id: Uuid::new_v4(),
            level,
            message: self.truncate_message(message),
            timestamp: Local::now(),
            logger: logger.to_string(),
            thread_id: if self.config.include_thread {
                Some(format!("{:?}", std::thread::current().id()))
            } else {
                None
            },
            source: None,
            fields: HashMap::new(),
            session_id,
            message_id,
            plugin_id,
            correlation_id: None,
            metrics: None,
        };

        self.write_entry(entry);
    }

    /// Log an enhanced error
    pub fn log_error(&mut self, error: &EnhancedError) {
        let level = match error.severity {
            ErrorSeverity::Info => LogLevel::Info,
            ErrorSeverity::Warning => LogLevel::Warn,
            ErrorSeverity::Error => LogLevel::Error,
            ErrorSeverity::Critical => LogLevel::Error,
            ErrorSeverity::Fatal => LogLevel::Fatal,
        };

        let mut fields = HashMap::new();
        fields.insert("error_id".to_string(), Value::String(error.id.to_string()));
        fields.insert("error_category".to_string(), Value::String(error.category.to_string()));
        fields.insert("error_severity".to_string(), Value::String(error.severity.to_string()));
        fields.insert("retry_count".to_string(), Value::Number(error.retry_count.into()));
        fields.insert("is_retryable".to_string(), Value::Bool(error.is_retryable));

        if let Some(details) = &error.details {
            fields.insert("error_details".to_string(), Value::String(details.clone()));
        }

        if let Some(operation) = &error.context.operation {
            fields.insert("operation".to_string(), Value::String(operation.clone()));
        }

        if let Some(component) = &error.context.component {
            fields.insert("component".to_string(), Value::String(component.clone()));
        }

        // Add metadata
        for (key, value) in &error.context.metadata {
            fields.insert(format!("meta_{}", key), Value::String(value.clone()));
        }

        let entry = LogEntry {
            id: Uuid::new_v4(),
            level,
            message: self.truncate_message(&error.message),
            timestamp: Local::now(),
            logger: "error".to_string(),
            thread_id: if self.config.include_thread {
                Some(format!("{:?}", std::thread::current().id()))
            } else {
                None
            },
            source: error.context.source_location.as_ref().map(|loc| SourceLocation {
                file: loc.file.clone(),
                line: loc.line,
                function: loc.function.clone(),
            }),
            fields,
            session_id: error.context.session_id,
            message_id: error.context.message_id,
            plugin_id: error.context.plugin_id.clone(),
            correlation_id: Some(error.id),
            metrics: None,
        };

        // Track error for correlation
        if self.config.enable_error_correlation {
            let mut tracker = self.error_tracker.lock().unwrap();
            tracker.track_error(error.id, ErrorContext {
                session_id: error.context.session_id,
                operation: error.context.operation.clone(),
                component: error.context.component.clone(),
                timestamp: error.timestamp,
            });
        }

        // Update metrics
        if self.config.enable_performance_logging {
            let mut collector = self.metrics_collector.lock().unwrap();
            collector.record_error(&error.category);
        }

        self.write_entry(entry);
    }

    /// Log performance metrics
    pub fn log_performance(
        &mut self,
        operation: &str,
        duration_ms: u64,
        additional_metrics: Option<PerformanceMetrics>,
    ) {
        if !self.config.enable_performance_logging {
            return;
        }

        let mut fields = HashMap::new();
        fields.insert("operation".to_string(), Value::String(operation.to_string()));
        fields.insert("duration_ms".to_string(), Value::Number(duration_ms.into()));

        let metrics = additional_metrics.unwrap_or_else(|| PerformanceMetrics {
            duration_ms,
            memory_usage: None,
            cpu_usage: None,
            counters: HashMap::new(),
        });

        let entry = LogEntry {
            id: Uuid::new_v4(),
            level: LogLevel::Info,
            message: format!("Performance: {} completed in {}ms", operation, duration_ms),
            timestamp: Local::now(),
            logger: "performance".to_string(),
            thread_id: if self.config.include_thread {
                Some(format!("{:?}", std::thread::current().id()))
            } else {
                None
            },
            source: None,
            fields,
            session_id: None,
            message_id: None,
            plugin_id: None,
            correlation_id: None,
            metrics: Some(metrics),
        };

        // Update metrics collector
        {
            let mut collector = self.metrics_collector.lock().unwrap();
            collector.record_operation_timing(operation, duration_ms);
        }

        self.write_entry(entry);
    }

    /// Write a log entry to all outputs
    fn write_entry(&mut self, entry: LogEntry) {
        // Check if entry passes all filters
        if !self.filters.iter().all(|filter| filter.should_log(&entry)) {
            return;
        }

        // Check minimum level
        if entry.level < self.config.min_level {
            return;
        }

        // Update log count metrics
        if self.config.enable_performance_logging {
            let mut collector = self.metrics_collector.lock().unwrap();
            collector.record_log_entry(&entry.level);
        }

        // Write to all outputs
        for output in &mut self.outputs {
            if let Err(e) = output.write(&entry) {
                eprintln!("Failed to write log entry: {}", e);
            }
        }
    }

    /// Truncate message if it exceeds maximum length
    fn truncate_message(&self, message: &str) -> String {
        if message.len() <= self.config.max_message_length {
            message.to_string()
        } else {
            format!("{}...", &message[..self.config.max_message_length - 3])
        }
    }

    /// Flush all outputs
    pub fn flush(&mut self) {
        for output in &mut self.outputs {
            if let Err(e) = output.flush() {
                eprintln!("Failed to flush log output: {}", e);
            }
        }
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> MetricsSnapshot {
        let collector = self.metrics_collector.lock().unwrap();
        collector.get_snapshot()
    }

    /// Get error correlation information
    pub fn get_error_correlations(&self, error_id: Uuid) -> Vec<Uuid> {
        let tracker = self.error_tracker.lock().unwrap();
        tracker.get_related_errors(error_id)
    }

    /// Convenience methods for different log levels
    pub fn trace(&mut self, logger: &str, message: &str) {
        self.log(LogLevel::Trace, logger, message);
    }

    pub fn debug(&mut self, logger: &str, message: &str) {
        self.log(LogLevel::Debug, logger, message);
    }

    pub fn info(&mut self, logger: &str, message: &str) {
        self.log(LogLevel::Info, logger, message);
    }

    pub fn warn(&mut self, logger: &str, message: &str) {
        self.log(LogLevel::Warn, logger, message);
    }

    pub fn error(&mut self, logger: &str, message: &str) {
        self.log(LogLevel::Error, logger, message);
    }

    pub fn fatal(&mut self, logger: &str, message: &str) {
        self.log(LogLevel::Fatal, logger, message);
    }
}

impl ConsoleOutput {
    /// Create a new console output
    pub fn new(colored: bool, format: ConsoleFormat) -> Self {
        Self { colored, format }
    }
}

impl LogOutput for ConsoleOutput {
    fn write(&mut self, entry: &LogEntry) -> Result<(), io::Error> {
        let output = match self.format {
            ConsoleFormat::Human => self.format_human(entry),
            ConsoleFormat::Json => serde_json::to_string(entry).unwrap_or_else(|_| "Invalid JSON".to_string()),
            ConsoleFormat::Compact => self.format_compact(entry),
        };

        if self.colored {
            self.write_colored(&output, entry.level)
        } else {
            println!("{}", output);
            Ok(())
        }
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        io::stdout().flush()
    }

    fn close(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

impl ConsoleOutput {
    fn format_human(&self, entry: &LogEntry) -> String {
        let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
        let level = format!("{:5}", entry.level.to_string().to_uppercase());
        
        let mut output = format!("{} {} [{}] {}", timestamp, level, entry.logger, entry.message);
        
        if !entry.fields.is_empty() {
            output.push_str(" | ");
            let fields: Vec<String> = entry.fields.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            output.push_str(&fields.join(" "));
        }
        
        if let Some(session_id) = entry.session_id {
            output.push_str(&format!(" | session={}", &session_id.to_string()[..8]));
        }
        
        output
    }

    fn format_compact(&self, entry: &LogEntry) -> String {
        let timestamp = entry.timestamp.format("%H:%M:%S");
        let level = entry.level.to_string().chars().next().unwrap().to_uppercase();
        format!("{} {} {}: {}", timestamp, level, entry.logger, entry.message)
    }

    fn write_colored(&self, output: &str, _level: LogLevel) -> Result<(), io::Error> {
        // This would use a crate like `colored` for actual color output
        // For now, just print normally
        println!("{}", output);
        Ok(())
    }
}

impl FileOutput {
    /// Create a new file output
    pub fn new(file_path: PathBuf, rotation_config: Option<LogRotationConfig>) -> Result<Self, io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
        
        let current_size = file.metadata()?.len();
        
        Ok(Self {
            writer: Box::new(file),
            file_path,
            current_size,
            rotation_config,
        })
    }
}

impl LogOutput for FileOutput {
    fn write(&mut self, entry: &LogEntry) -> Result<(), io::Error> {
        let json_line = serde_json::to_string(entry)?;
        let line_with_newline = format!("{}\n", json_line);
        
        self.writer.write_all(line_with_newline.as_bytes())?;
        self.current_size += line_with_newline.len() as u64;
        
        // Check if rotation is needed
        if let Some(config) = self.rotation_config.clone() {
            if self.current_size > config.max_file_size {
                self.rotate_file(&config)?;
            }
        }
        
        Ok(())
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.writer.flush()
    }

    fn close(&mut self) -> Result<(), io::Error> {
        self.writer.flush()
    }
}

impl FileOutput {
    fn rotate_file(&mut self, config: &LogRotationConfig) -> Result<(), io::Error> {
        // Close current file
        self.writer.flush()?;
        
        // Rotate existing backup files
        for i in (1..config.max_backup_files).rev() {
            let old_backup = self.file_path.with_extension(format!("log.{}", i));
            let new_backup = self.file_path.with_extension(format!("log.{}", i + 1));
            
            if old_backup.exists() {
                std::fs::rename(old_backup, new_backup)?;
            }
        }
        
        // Move current file to .1 backup
        let first_backup = self.file_path.with_extension("log.1");
        std::fs::rename(&self.file_path, first_backup)?;
        
        // Create new file
        let new_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.file_path)?;
        
        self.writer = Box::new(new_file);
        self.current_size = 0;
        
        Ok(())
    }
}

impl LevelFilter {
    /// Create a new level filter
    pub fn new(min_level: LogLevel) -> Self {
        Self { min_level }
    }
}

impl LogFilter for LevelFilter {
    fn should_log(&self, entry: &LogEntry) -> bool {
        entry.level >= self.min_level
    }
}

impl ComponentFilter {
    /// Create a new component filter
    pub fn new(allowed: Vec<String>, blocked: Vec<String>) -> Self {
        Self {
            allowed_components: allowed,
            blocked_components: blocked,
        }
    }
}

impl LogFilter for ComponentFilter {
    fn should_log(&self, entry: &LogEntry) -> bool {
        // If blocked list contains the component, don't log
        if self.blocked_components.contains(&entry.logger) {
            return false;
        }
        
        // If allowed list is empty, allow all (except blocked)
        if self.allowed_components.is_empty() {
            return true;
        }
        
        // Otherwise, only allow if in allowed list
        self.allowed_components.contains(&entry.logger)
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            operation_timings: HashMap::new(),
            error_counts: HashMap::new(),
            log_counts: HashMap::new(),
            memory_samples: Vec::new(),
        }
    }

    /// Record operation timing
    pub fn record_operation_timing(&mut self, operation: &str, duration_ms: u64) {
        self.operation_timings
            .entry(operation.to_string())
            .or_insert_with(Vec::new)
            .push(duration_ms);
    }

    /// Record error occurrence
    pub fn record_error(&mut self, category: &ErrorCategory) {
        *self.error_counts.entry(category.clone()).or_insert(0) += 1;
    }

    /// Record log entry
    pub fn record_log_entry(&mut self, level: &LogLevel) {
        *self.log_counts.entry(*level).or_insert(0) += 1;
    }

    /// Get metrics snapshot
    pub fn get_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            operation_timings: self.operation_timings.clone(),
            error_counts: self.error_counts.clone(),
            log_counts: self.log_counts.clone(),
            memory_samples: self.memory_samples.clone(),
        }
    }
}

impl ErrorTracker {
    /// Create a new error tracker
    pub fn new() -> Self {
        Self {
            error_chains: HashMap::new(),
            error_contexts: HashMap::new(),
        }
    }

    /// Track an error
    pub fn track_error(&mut self, error_id: Uuid, context: ErrorContext) {
        self.error_contexts.insert(error_id, context);
    }

    /// Get related errors
    pub fn get_related_errors(&self, error_id: Uuid) -> Vec<Uuid> {
        self.error_chains.get(&error_id).cloned().unwrap_or_default()
    }
}

/// Metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub operation_timings: HashMap<String, Vec<u64>>,
    pub error_counts: HashMap<ErrorCategory, u64>,
    pub log_counts: HashMap<LogLevel, u64>,
    pub memory_samples: Vec<(DateTime<Local>, u64)>,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Info,
            include_timestamp: true,
            include_source: false,
            include_thread: false,
            max_message_length: 1000,
            enable_performance_logging: true,
            enable_error_correlation: true,
            rotation: None,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Macro for structured logging
#[macro_export]
macro_rules! log_structured {
    ($logger:expr, $level:expr, $message:expr) => {
        $logger.log($level, module_path!(), $message)
    };
    ($logger:expr, $level:expr, $message:expr, $($key:expr => $value:expr),+) => {
        {
            let mut fields = std::collections::HashMap::new();
            $(
                fields.insert($key.to_string(), serde_json::Value::String($value.to_string()));
            )+
            $logger.log_with_fields($level, module_path!(), $message, fields)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_structured_logger_creation() {
        let config = LoggerConfig::default();
        let logger = StructuredLogger::new(config);
        
        // Logger should be created successfully
        assert_eq!(logger.outputs.len(), 0);
        assert_eq!(logger.filters.len(), 0);
    }

    #[test]
    fn test_console_output() {
        let mut output = ConsoleOutput::new(false, ConsoleFormat::Human);
        
        let entry = LogEntry {
            id: Uuid::new_v4(),
            level: LogLevel::Info,
            message: "Test message".to_string(),
            timestamp: Local::now(),
            logger: "test".to_string(),
            thread_id: None,
            source: None,
            fields: HashMap::new(),
            session_id: None,
            message_id: None,
            plugin_id: None,
            correlation_id: None,
            metrics: None,
        };
        
        // Should not panic
        let result = output.write(&entry);
        assert!(result.is_ok());
    }

    #[test]
    fn test_level_filter() {
        let filter = LevelFilter::new(LogLevel::Warn);
        
        let info_entry = LogEntry {
            id: Uuid::new_v4(),
            level: LogLevel::Info,
            message: "Info message".to_string(),
            timestamp: Local::now(),
            logger: "test".to_string(),
            thread_id: None,
            source: None,
            fields: HashMap::new(),
            session_id: None,
            message_id: None,
            plugin_id: None,
            correlation_id: None,
            metrics: None,
        };
        
        let error_entry = LogEntry {
            level: LogLevel::Error,
            ..info_entry.clone()
        };
        
        assert!(!filter.should_log(&info_entry));
        assert!(filter.should_log(&error_entry));
    }

    #[test]
    fn test_component_filter() {
        let filter = ComponentFilter::new(
            vec!["allowed".to_string()],
            vec!["blocked".to_string()],
        );
        
        let allowed_entry = LogEntry {
            id: Uuid::new_v4(),
            level: LogLevel::Info,
            message: "Test".to_string(),
            timestamp: Local::now(),
            logger: "allowed".to_string(),
            thread_id: None,
            source: None,
            fields: HashMap::new(),
            session_id: None,
            message_id: None,
            plugin_id: None,
            correlation_id: None,
            metrics: None,
        };
        
        let blocked_entry = LogEntry {
            logger: "blocked".to_string(),
            ..allowed_entry.clone()
        };
        
        let other_entry = LogEntry {
            logger: "other".to_string(),
            ..allowed_entry.clone()
        };
        
        assert!(filter.should_log(&allowed_entry));
        assert!(!filter.should_log(&blocked_entry));
        assert!(!filter.should_log(&other_entry));
    }

    #[test]
    fn test_metrics_collector() {
        let mut collector = MetricsCollector::new();
        
        collector.record_operation_timing("test_op", 100);
        collector.record_operation_timing("test_op", 200);
        collector.record_error(&ErrorCategory::Network);
        collector.record_log_entry(&LogLevel::Info);
        
        let snapshot = collector.get_snapshot();
        
        assert_eq!(snapshot.operation_timings.get("test_op"), Some(&vec![100, 200]));
        assert_eq!(snapshot.error_counts.get(&ErrorCategory::Network), Some(&1));
        assert_eq!(snapshot.log_counts.get(&LogLevel::Info), Some(&1));
    }

    #[test]
    fn test_file_output_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_path_buf();
        
        let output = FileOutput::new(file_path, None);
        assert!(output.is_ok());
    }
}