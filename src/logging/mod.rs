//! Logging module
//! 
//! This module provides comprehensive logging capabilities including:
//! - Structured logging with JSON output
//! - Configurable log levels and filters
//! - Multiple output destinations
//! - Performance metrics logging
//! - Error correlation and tracking

pub mod structured_logger;

pub use structured_logger::{
    StructuredLogger,
    LoggerConfig,
    LogLevel,
    LogEntry,
    LogOutput,
    LogFilter,
    ConsoleOutput,
    FileOutput,
    LevelFilter,
    ComponentFilter,
    PerformanceMetrics,
    MetricsSnapshot,
    ConsoleFormat,
    LogRotationConfig,
};