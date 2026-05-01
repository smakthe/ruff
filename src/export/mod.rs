//! Export and import functionality module
//!
//! This module provides data export/import capabilities including:
//! - Multiple export formats (Markdown, JSON, HTML, Plain Text)
//! - Conversation import from other chat applications
//! - Bulk operations and backup management
//! - Data validation and integrity checking

pub mod backup;
pub mod formats;
pub mod import;
pub mod service;
pub mod validation;

pub use backup::{
    BackupConfig, BackupManager, BackupMetadata, BackupResult, BackupStatistics, RestoreResult,
};
pub use formats::{ExportFormat, ExportOptions, FormatHandler, ImportFormat};
pub use import::{ImportResult, ImportService};
pub use service::{
    BulkErrorType, BulkExportRequest, BulkOperationError, BulkOperationResult, ExportResult,
    ExportService, ExportStatistics, MessagesExportRequest, ProgressCallback, SessionExportRequest,
};
pub use validation::{DataValidator, ValidationResult};

#[cfg(any())]
pub mod tests;
