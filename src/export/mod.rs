//! Export and import functionality module
//! 
//! This module provides data export/import capabilities including:
//! - Multiple export formats (Markdown, JSON, HTML, Plain Text)
//! - Conversation import from other chat applications
//! - Bulk operations and backup management
//! - Data validation and integrity checking

pub mod formats;
pub mod service;
pub mod import;
pub mod backup;
pub mod validation;

pub use formats::{ExportFormat, ImportFormat, FormatHandler, ExportOptions};
pub use service::{
    ExportService, ExportResult, SessionExportRequest, BulkExportRequest, MessagesExportRequest, 
    ExportStatistics, ProgressCallback, BulkOperationResult, BulkOperationError, BulkErrorType
};
pub use import::{ImportService, ImportResult};
pub use backup::{
    BackupManager, BackupConfig, BackupMetadata, BackupResult, RestoreResult, BackupStatistics
};
pub use validation::{DataValidator, ValidationResult};

#[cfg(test)]
pub mod tests;