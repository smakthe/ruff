//! Export service implementation

use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};
use uuid::Uuid;
use tokio::time::{sleep, Duration};

use crate::events::SessionId;
use crate::session::manager::{ChatSession, Message, MessageRole, MessageMetadata, SessionModelConfig};
use crate::models::TokenUsage;
use crate::EnhancedError;
use super::formats::{
    ExportFormat, ExportOptions, FormatHandler,
    MarkdownHandler, PlainTextHandler, JsonHandler, HtmlHandler
};

/// Export service for handling data exports
pub struct ExportService {
    export_directory: PathBuf,
}

/// Export result containing information about the exported data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub export_id: Uuid,
    pub session_id: Option<SessionId>,
    pub format: ExportFormat,
    pub file_path: String,
    pub size_bytes: u64,
    pub exported_at: DateTime<Local>,
    pub message_count: usize,
    pub options: ExportOptions,
}

/// Export request for single session
#[derive(Debug, Clone)]
pub struct SessionExportRequest {
    pub session_id: SessionId,
    pub format: ExportFormat,
    pub options: ExportOptions,
    pub output_path: Option<PathBuf>,
}

/// Export request for multiple sessions
#[derive(Debug, Clone)]
pub struct BulkExportRequest {
    pub session_ids: Vec<SessionId>,
    pub format: ExportFormat,
    pub options: ExportOptions,
    pub output_directory: Option<PathBuf>,
}

/// Export request for messages only
#[derive(Debug, Clone)]
pub struct MessagesExportRequest {
    pub messages: Vec<Message>,
    pub format: ExportFormat,
    pub options: ExportOptions,
    pub output_path: Option<PathBuf>,
    pub title: Option<String>,
}

/// Progress callback for long-running operations
pub type ProgressCallback = Arc<dyn Fn(usize, usize, String) + Send + Sync>;

/// Bulk operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkOperationResult {
    pub operation_id: Uuid,
    pub total_items: usize,
    pub successful_items: usize,
    pub failed_items: usize,
    pub results: Vec<ExportResult>,
    pub errors: Vec<BulkOperationError>,
    pub duration_ms: u64,
    pub completed_at: DateTime<Local>,
}

/// Bulk operation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkOperationError {
    pub session_id: SessionId,
    pub error_message: String,
    pub error_type: BulkErrorType,
}

/// Types of bulk operation errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BulkErrorType {
    SessionNotFound,
    ExportFailed,
    FileWriteError,
    ValidationError,
}

impl ExportService {
    /// Create a new export service with the specified export directory
    pub fn new(export_directory: PathBuf) -> Self {
        Self {
            export_directory,
        }
    }

    /// Initialize the export service (create directories if needed)
    pub fn initialize(&self) -> Result<(), EnhancedError> {
        if !self.export_directory.exists() {
            fs::create_dir_all(&self.export_directory)
                .map_err(|e| EnhancedError::storage(format!("Failed to create export directory: {}", e)))?;
        }
        Ok(())
    }

    /// Export a single session to the specified format
    /// Requires MessageManager to access current messages
    pub fn export_session(&self, session: &ChatSession, request: SessionExportRequest, message_manager: &crate::message::manager::MessageManager) -> Result<ExportResult, EnhancedError> {
        let handler = self.get_format_handler(&request.format);

        // Get messages from MessageManager (single source of truth)
        let messages = message_manager.get_session_messages_owned(session.id);
        let message_count = messages.len();

        let content = handler.export_session_with_messages(session, &messages, &request.options)
            .map_err(|e| EnhancedError::unknown(format!("Failed to export session: {}", e)))?;

        let file_path = self.determine_output_path(
            request.output_path,
            &session.title,
            &request.format,
            Some(session.id),
        )?;

        self.write_export_file(&file_path, &content)?;

        let file_size = fs::metadata(&file_path)
            .map_err(|e| EnhancedError::unknown(format!("Failed to get file size: {}", e)))?
            .len();

        Ok(ExportResult {
            export_id: Uuid::new_v4(),
            session_id: Some(session.id),
            format: request.format,
            file_path: file_path.to_string_lossy().to_string(),
            size_bytes: file_size,
            exported_at: Local::now(),
            message_count,
            options: request.options,
        })
    }

    /// Export multiple sessions
    /// Requires MessageManager to access current messages
    pub fn export_sessions(&self, sessions: &[ChatSession], request: BulkExportRequest, message_manager: &crate::message::manager::MessageManager) -> Result<Vec<ExportResult>, EnhancedError> {
        let mut results = Vec::new();
        let handler = self.get_format_handler(&request.format);

        let output_dir = request.output_directory
            .unwrap_or_else(|| self.export_directory.join("bulk_export"));

        if !output_dir.exists() {
            fs::create_dir_all(&output_dir)
                .map_err(|e| EnhancedError::storage(format!("Failed to create bulk export directory: {}", e)))?;
        }

        for session in sessions {
            if !request.session_ids.contains(&session.id) {
                continue;
            }

            // Get messages from MessageManager (single source of truth)
            let messages = message_manager.get_session_messages_owned(session.id);
            let message_count = messages.len();

            let content = handler.export_session_with_messages(session, &messages, &request.options)
                .map_err(|e| EnhancedError::unknown(format!("Failed to export session {}: {}", session.id, e)))?;

            let file_name = self.sanitize_filename(&format!("{}_{}",
                session.created_at.format("%Y%m%d_%H%M%S"),
                session.title
            ));
            let file_path = output_dir.join(format!("{}.{}", file_name, self.get_file_extension(&request.format)));

            self.write_export_file(&file_path, &content)?;

            let file_size = fs::metadata(&file_path)
                .map_err(|e| EnhancedError::unknown(format!("Failed to get file size: {}", e)))?
                .len();

            results.push(ExportResult {
                export_id: Uuid::new_v4(),
                session_id: Some(session.id),
                format: request.format.clone(),
                file_path: file_path.to_string_lossy().to_string(),
                size_bytes: file_size,
                exported_at: Local::now(),
                message_count,
                options: request.options.clone(),
            });
        }

        Ok(results)
    }

    /// Export multiple sessions with progress tracking
    pub async fn export_sessions_with_progress(
        &self,
        sessions: &[ChatSession],
        request: BulkExportRequest,
        message_manager: &crate::message::manager::MessageManager,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<BulkOperationResult, EnhancedError> {
        let start_time = std::time::SystemTime::now();
        let operation_id = Uuid::new_v4();
        
        let handler = self.get_format_handler(&request.format);
        let output_dir = request.output_directory.clone()
            .unwrap_or_else(|| self.export_directory.join("bulk_export"));

        if !output_dir.exists() {
            fs::create_dir_all(&output_dir)
                .map_err(|e| EnhancedError::storage(format!("Failed to create bulk export directory: {}", e)))?;
        }

        // Filter sessions to export
        let sessions_to_export: Vec<&ChatSession> = sessions
            .iter()
            .filter(|session| request.session_ids.contains(&session.id))
            .collect();

        let total_sessions = sessions_to_export.len();
        let mut results = Vec::new();
        let mut errors = Vec::new();
        let mut processed = 0;

        if let Some(callback) = &progress_callback {
            callback(0, total_sessions, "Starting bulk export...".to_string());
        }

        for session in sessions_to_export {
            if let Some(callback) = &progress_callback {
                callback(processed, total_sessions, format!("Exporting: {}", session.title));
            }

            match self.export_single_session_internal(session, &request, &output_dir, &handler, message_manager) {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    errors.push(BulkOperationError {
                        session_id: session.id,
                        error_message: e.to_string(),
                        error_type: BulkErrorType::ExportFailed,
                    });
                }
            }

            processed += 1;

            // Small delay to prevent overwhelming the system
            if processed % 10 == 0 {
                sleep(Duration::from_millis(10)).await;
            }
        }

        let duration = start_time.elapsed()
            .map_err(|e| EnhancedError::unknown(format!("Failed to calculate duration: {}", e)))?
            .as_millis() as u64;

        if let Some(callback) = &progress_callback {
            let message = if errors.is_empty() {
                format!("Bulk export completed successfully: {} sessions exported", results.len())
            } else {
                format!("Bulk export completed with errors: {} successful, {} failed", results.len(), errors.len())
            };
            callback(total_sessions, total_sessions, message);
        }

        Ok(BulkOperationResult {
            operation_id,
            total_items: total_sessions,
            successful_items: results.len(),
            failed_items: errors.len(),
            results,
            errors,
            duration_ms: duration,
            completed_at: Local::now(),
        })
    }

    /// Internal method to export a single session (used by bulk operations)
    fn export_single_session_internal(
        &self,
        session: &ChatSession,
        request: &BulkExportRequest,
        output_dir: &Path,
        handler: &Box<dyn FormatHandler>,
        message_manager: &crate::message::manager::MessageManager,
    ) -> Result<ExportResult, EnhancedError> {
        let messages = message_manager.get_session_messages_owned(session.id);
        let content = handler.export_session_with_messages(session, &messages, &request.options)
            .map_err(|e| EnhancedError::unknown(format!("Failed to export session {}: {}", session.id, e)))?;

        let file_name = self.sanitize_filename(&format!("{}_{}", 
            session.created_at.format("%Y%m%d_%H%M%S"),
            session.title
        ));
        let file_path = output_dir.join(format!("{}.{}", file_name, self.get_file_extension(&request.format)));

        self.write_export_file(&file_path, &content)?;

        let file_size = fs::metadata(&file_path)
            .map_err(|e| EnhancedError::unknown(format!("Failed to get file size: {}", e)))?
            .len();

        Ok(ExportResult {
            export_id: Uuid::new_v4(),
            session_id: Some(session.id),
            format: request.format.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            size_bytes: file_size,
            exported_at: Local::now(),
            message_count: messages.len(),
            options: request.options.clone(),
        })
    }

    /// Export messages only (without session metadata)
    pub fn export_messages(&self, request: MessagesExportRequest) -> Result<ExportResult, EnhancedError> {
        let handler = self.get_format_handler(&request.format);
        let content = handler.export_messages(&request.messages, &request.options)
            .map_err(|e| EnhancedError::unknown(format!("Failed to export messages: {}", e)))?;

        let title = request.title.unwrap_or_else(|| "messages".to_string());
        let file_path = self.determine_output_path(
            request.output_path,
            &title,
            &request.format,
            None,
        )?;

        self.write_export_file(&file_path, &content)?;

        let file_size = fs::metadata(&file_path)
            .map_err(|e| EnhancedError::unknown(format!("Failed to get file size: {}", e)))?
            .len();

        Ok(ExportResult {
            export_id: Uuid::new_v4(),
            session_id: None,
            format: request.format,
            file_path: file_path.to_string_lossy().to_string(),
            size_bytes: file_size,
            exported_at: Local::now(),
            message_count: request.messages.len(),
            options: request.options,
        })
    }

    /// Get the appropriate format handler for the given format
    fn get_format_handler(&self, format: &ExportFormat) -> Box<dyn FormatHandler> {
        match format {
            ExportFormat::Markdown => Box::new(MarkdownHandler),
            ExportFormat::PlainText => Box::new(PlainTextHandler),
            ExportFormat::Json => Box::new(JsonHandler),
            ExportFormat::Html => Box::new(HtmlHandler),
        }
    }

    /// Determine the output file path
    fn determine_output_path(
        &self,
        requested_path: Option<PathBuf>,
        title: &str,
        format: &ExportFormat,
        session_id: Option<SessionId>,
    ) -> Result<PathBuf, EnhancedError> {
        if let Some(path) = requested_path {
            // Use the requested path directly
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| EnhancedError::storage(format!("Failed to create output directory: {}", e)))?;
                }
            }
            Ok(path)
        } else {
            // Generate a path in the export directory
            let sanitized_title = self.sanitize_filename(title);
            let timestamp = Local::now().format("%Y%m%d_%H%M%S");
            
            let filename = if let Some(id) = session_id {
                format!("{}_{}_{}",
                    timestamp,
                    sanitized_title,
                    id.to_string().split('-').next().unwrap_or("unknown")
                )
            } else {
                format!("{}_{}", timestamp, sanitized_title)
            };

            let extension = self.get_file_extension(format);
            let file_path = self.export_directory.join(format!("{}.{}", filename, extension));

            Ok(file_path)
        }
    }

    /// Get the file extension for a format
    fn get_file_extension(&self, format: &ExportFormat) -> &'static str {
        match format {
            ExportFormat::Markdown => "md",
            ExportFormat::PlainText => "txt",
            ExportFormat::Json => "json",
            ExportFormat::Html => "html",
        }
    }

    /// Sanitize a filename by removing invalid characters
    pub fn sanitize_filename(&self, filename: &str) -> String {
        filename
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// Write content to a file
    fn write_export_file(&self, file_path: &Path, content: &str) -> Result<(), EnhancedError> {
        fs::write(file_path, content)
            .map_err(|e| EnhancedError::storage(format!("Failed to write export file: {}", e)))?;
        Ok(())
    }

    /// Get the export directory
    pub fn get_export_directory(&self) -> &Path {
        &self.export_directory
    }

    /// Set a new export directory
    pub fn set_export_directory(&mut self, directory: PathBuf) -> Result<(), EnhancedError> {
        if !directory.exists() {
            fs::create_dir_all(&directory)
                .map_err(|e| EnhancedError::storage(format!("Failed to create export directory: {}", e)))?;
        }
        self.export_directory = directory;
        Ok(())
    }

    /// List all export files in the export directory
    pub fn list_exports(&self) -> Result<Vec<PathBuf>, EnhancedError> {
        if !self.export_directory.exists() {
            return Ok(Vec::new());
        }

        let mut exports = Vec::new();
        let entries = fs::read_dir(&self.export_directory)
            .map_err(|e| EnhancedError::storage(format!("Failed to read export directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| EnhancedError::storage(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                    if matches!(extension, "md" | "txt" | "json" | "html") {
                        exports.push(path);
                    }
                }
            }
        }

        exports.sort();
        Ok(exports)
    }

    /// Delete an export file
    pub fn delete_export(&self, file_path: &Path) -> Result<(), EnhancedError> {
        if !file_path.exists() {
            return Err(EnhancedError::unknown("Export file does not exist".to_string()));
        }

        // Ensure the file is within the export directory for security
        if !file_path.starts_with(&self.export_directory) {
            return Err(EnhancedError::unknown("Cannot delete files outside export directory".to_string()));
        }

        fs::remove_file(file_path)
            .map_err(|e| EnhancedError::storage(format!("Failed to delete export file: {}", e)))?;

        Ok(())
    }

    /// Import multiple sessions from a directory
    pub async fn import_sessions_from_directory(
        &self,
        import_directory: &Path,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<BulkOperationResult, EnhancedError> {
        let start_time = std::time::SystemTime::now();
        let operation_id = Uuid::new_v4();

        if !import_directory.exists() {
            return Err(EnhancedError::unknown("Import directory does not exist".to_string()));
        }

        // Find all JSON files in the directory (assuming JSON format for imports)
        let mut session_files = Vec::new();
        let entries = fs::read_dir(import_directory)
            .map_err(|e| EnhancedError::storage(format!("Failed to read import directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| EnhancedError::storage(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                session_files.push(path);
            }
        }

        let total_files = session_files.len();
        let mut results = Vec::new();
        let mut errors = Vec::new();
        let mut processed = 0;

        if let Some(callback) = &progress_callback {
            callback(0, total_files, "Starting bulk import...".to_string());
        }

        for session_file in session_files {
            if let Some(callback) = &progress_callback {
                callback(processed, total_files, format!("Importing: {}", session_file.display()));
            }

            match self.import_single_session_file(&session_file).await {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    let session_id = self.extract_session_id_from_path(&session_file)
                        .unwrap_or_else(|| Uuid::new_v4());
                    errors.push(BulkOperationError {
                        session_id,
                        error_message: e.to_string(),
                        error_type: BulkErrorType::ValidationError,
                    });
                }
            }

            processed += 1;

            // Small delay to prevent overwhelming the system
            if processed % 10 == 0 {
                sleep(Duration::from_millis(10)).await;
            }
        }

        let duration = start_time.elapsed()
            .map_err(|e| EnhancedError::unknown(format!("Failed to calculate duration: {}", e)))?
            .as_millis() as u64;

        if let Some(callback) = &progress_callback {
            let message = if errors.is_empty() {
                format!("Bulk import completed successfully: {} sessions imported", results.len())
            } else {
                format!("Bulk import completed with errors: {} successful, {} failed", results.len(), errors.len())
            };
            callback(total_files, total_files, message);
        }

        Ok(BulkOperationResult {
            operation_id,
            total_items: total_files,
            successful_items: results.len(),
            failed_items: errors.len(),
            results,
            errors,
            duration_ms: duration,
            completed_at: Local::now(),
        })
    }

    /// Import a single session from a JSON file
    async fn import_single_session_file(&self, session_file: &Path) -> Result<ExportResult, EnhancedError> {
        let content = fs::read_to_string(session_file)
            .map_err(|e| EnhancedError::storage(format!("Failed to read session file: {}", e)))?;

        // Try to parse as ChatSession (new format without embedded messages)
        let (session, message_count) = match serde_json::from_str::<ChatSession>(&content) {
            Ok(session) => {
                let msg_count = session.message_count;
                (session, msg_count as usize)
            },
            Err(_) => {
                // Try to parse as old ChatSessionWithMessages format (backward compatibility)
                match serde_json::from_str::<crate::session::manager::ChatSessionWithMessages>(&content) {
                    Ok(session_with_msgs) => {
                        let msg_count = session_with_msgs.messages.len();
                        let (session, _messages) = session_with_msgs.split();
                        // TODO: Store _messages in MessageManager when this is integrated
                        (session, msg_count)
                    },
                    Err(_) => {
                        // Try to parse as export format (with "session" and "messages" fields)
                        let export_data: serde_json::Value = serde_json::from_str(&content)
                            .map_err(|e| EnhancedError::unknown(format!("Failed to parse JSON: {}", e)))?;

                        let session_with_msgs = self.parse_export_format(&export_data)?;
                        let msg_count = session_with_msgs.messages.len();
                        let (session, _messages) = session_with_msgs.split();
                        (session, msg_count)
                    }
                }
            }
        };

        // Validate the session data (basic validation without messages)
        self.validate_session_data(&session)?;

        // Create an export result for consistency
        Ok(ExportResult {
            export_id: Uuid::new_v4(),
            session_id: Some(session.id),
            format: ExportFormat::Json,
            file_path: session_file.to_string_lossy().to_string(),
            size_bytes: content.len() as u64,
            exported_at: Local::now(),
            message_count,
            options: ExportOptions::default(),
        })
    }

    /// Parse export format JSON into ChatSessionWithMessages
    fn parse_export_format(&self, export_data: &serde_json::Value) -> Result<crate::session::manager::ChatSessionWithMessages, EnhancedError> {
        // Check if it has the export format structure (flat format with title, model, messages)
        if let Some(messages_data) = export_data.get("messages") {
            // This is the flat export format
            let title = export_data.get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Imported Session")
                .to_string();
            
            let model = export_data.get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("gpt-3.5-turbo")
                .to_string();
            
            // Parse messages
            let messages: Result<Vec<Message>, EnhancedError> = messages_data.as_array()
                .ok_or_else(|| EnhancedError::unknown("Messages must be an array".to_string()))?
                .iter()
                .map(|msg_data| self.parse_message_from_json(msg_data))
                .collect();
            
            let messages = messages?;
            let now = Local::now();

            Ok(crate::session::manager::ChatSessionWithMessages {
                id: Uuid::new_v4(), // Generate new ID for imported session
                title,
                created_at: now,
                updated_at: now,
                messages,
                model,
                system_prompt: None,
                model_config: SessionModelConfig::default(),
                total_tokens_used: TokenUsage::default(),
                tags: Vec::new(),
                is_archived: false,
                export_count: 0,
                message_count: 0,
                last_activity: now,
            })
        }
        // Check if it has the nested export format structure (with "session" and "messages" fields)
        else if let (Some(session_data), Some(messages_data)) = (export_data.get("session"), export_data.get("messages")) {
            // Parse session metadata
            let session_id = session_data.get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(|| Uuid::new_v4());
            
            let title = session_data.get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Imported Session")
                .to_string();
            
            let model = session_data.get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("gpt-3.5-turbo")
                .to_string();
            
            let created_at = session_data.get("created_at")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(|| Local::now());
            
            let updated_at = session_data.get("updated_at")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(|| Local::now());
            
            let system_prompt = session_data.get("system_prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            let tags = session_data.get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_else(Vec::new);
            
            // Parse messages
            let messages: Result<Vec<Message>, EnhancedError> = messages_data.as_array()
                .ok_or_else(|| EnhancedError::unknown("Messages must be an array".to_string()))?
                .iter()
                .map(|msg_data| self.parse_message_from_json(msg_data))
                .collect();
            
            let messages = messages?;

            Ok(crate::session::manager::ChatSessionWithMessages {
                id: session_id,
                title,
                created_at,
                updated_at,
                messages,
                model,
                system_prompt,
                model_config: SessionModelConfig::default(),
                total_tokens_used: TokenUsage::default(),
                tags,
                is_archived: false,
                export_count: 0,
                message_count: 0,
                last_activity: Local::now(),
            })
        } else {
            Err(EnhancedError::unknown("Invalid export format: missing session or messages".to_string()))
        }
    }

    /// Parse a message from JSON data
    fn parse_message_from_json(&self, msg_data: &serde_json::Value) -> Result<Message, EnhancedError> {
        let id = msg_data.get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(|| Uuid::new_v4());
        
        let role = msg_data.get("role")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "User" => Some(MessageRole::User),
                "Assistant" => Some(MessageRole::Assistant),
                "System" => Some(MessageRole::System),
                _ => None,
            })
            .unwrap_or(MessageRole::User);
        
        let content = msg_data.get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let timestamp = msg_data.get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(|| Local::now());
        
        let edited_at = msg_data.get("edited_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Local));
        
        let token_usage = msg_data.get("token_usage")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        
        let parent_id = msg_data.get("parent_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        
        let children = msg_data.get("children")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok())).collect())
            .unwrap_or_else(Vec::new);
        
        let metadata = msg_data.get("metadata")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| MessageMetadata {
                model_used: "unknown".to_string(),
                temperature: 0.7,
                response_time_ms: 0,
                is_regenerated: false,
                regeneration_count: 0,
            });
        
        Ok(Message {
            id,
            role,
            content,
            timestamp,
            edited_at,
            token_usage,
            parent_id,
            children,
            metadata,
        })
    }

    /// Validate session data before import (basic validation without message access)
    pub fn validate_session_data(&self, session: &ChatSession) -> Result<(), EnhancedError> {
        // Basic validation
        if session.title.trim().is_empty() {
            return Err(EnhancedError::unknown("Session title cannot be empty".to_string()));
        }

        if session.title.len() > 200 {
            return Err(EnhancedError::unknown("Session title too long".to_string()));
        }

        // Note: Message validation should be done by MessageManager when messages are added
        // For imports, use validate_session_data_with_messages instead

        Ok(())
    }

    /// Extract session ID from file path
    pub fn extract_session_id_from_path(&self, file_path: &Path) -> Option<SessionId> {
        if let Some(filename) = file_path.file_stem().and_then(|name| name.to_str()) {
            // Try to parse the filename as a UUID
            if let Ok(uuid) = Uuid::parse_str(filename) {
                return Some(uuid);
            }
            
            // Look for UUID pattern in filename
            let parts: Vec<&str> = filename.split('_').collect();
            for part in parts {
                if let Ok(uuid) = Uuid::parse_str(part) {
                    return Some(uuid);
                }
            }
        }
        None
    }

    /// Get export statistics
    pub fn get_export_statistics(&self) -> Result<ExportStatistics, EnhancedError> {
        let exports = self.list_exports()?;
        let mut total_size = 0u64;
        let mut format_counts = std::collections::HashMap::new();

        for export_path in &exports {
            if let Ok(metadata) = fs::metadata(export_path) {
                total_size += metadata.len();
            }

            if let Some(extension) = export_path.extension().and_then(|ext| ext.to_str()) {
                let format = match extension {
                    "md" => ExportFormat::Markdown,
                    "txt" => ExportFormat::PlainText,
                    "json" => ExportFormat::Json,
                    "html" => ExportFormat::Html,
                    _ => continue,
                };
                *format_counts.entry(format).or_insert(0) += 1;
            }
        }

        Ok(ExportStatistics {
            total_exports: exports.len(),
            total_size_bytes: total_size,
            format_counts,
            export_directory: self.export_directory.clone(),
        })
    }
}

/// Export statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportStatistics {
    pub total_exports: usize,
    pub total_size_bytes: u64,
    pub format_counts: std::collections::HashMap<ExportFormat, usize>,
    pub export_directory: PathBuf,
}

impl Default for ExportService {
    fn default() -> Self {
        let export_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ruff")
            .join("exports");
        Self::new(export_dir)
    }
}

impl ExportService {
    /// Create a new export service with default configuration
    pub async fn new_default() -> Result<Self, EnhancedError> {
        let export_directory = dirs::data_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("ruff")
            .join("exports");
        
        let service = Self::new(export_directory);
        service.initialize()?;
        Ok(service)
    }

    /// Export a session by ID with CLI parameters
    pub async fn export_session_cli(&self, session_id: SessionId, output_path: &Path, format: ExportFormat, include_metadata: bool) -> Result<ExportResult, EnhancedError> {
        // This would need to get the session from the session manager
        // For now, return a placeholder result
        let file_size = 0; // Would be calculated after writing
        
        Ok(ExportResult {
            export_id: Uuid::new_v4(),
            session_id: Some(session_id),
            format,
            file_path: output_path.to_string_lossy().to_string(),
            size_bytes: file_size,
            exported_at: Local::now(),
            message_count: 0, // Would be from actual session
            options: ExportOptions {
                include_metadata,
                include_timestamps: true,
                include_model_info: true,
                include_token_usage: include_metadata,
                pretty_format: true,
            },
        })
    }

    /// Export all sessions with CLI parameters
    pub async fn export_all_sessions(&self, output_dir: &Path, _format: ExportFormat, _include_metadata: bool, compress: bool) -> Result<BulkExportResult, EnhancedError> {
        // This would need to get all sessions from the session manager
        // For now, return a placeholder result
        Ok(BulkExportResult {
            session_count: 0,
            total_size: 0,
            output_path: output_dir.to_path_buf(),
            compressed: compress,
        })
    }
}

/// Result of bulk export operation for CLI
#[derive(Debug, Clone)]
pub struct BulkExportResult {
    pub session_count: usize,
    pub total_size: u64,
    pub output_path: PathBuf,
    pub compressed: bool,
}

