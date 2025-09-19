//! Backup management functionality

use std::path::{Path, PathBuf};
use std::fs;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local, Duration as ChronoDuration};
use uuid::Uuid;
use tokio::time::{sleep, Duration};
use tokio::task::JoinHandle;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::session::manager::{ChatSession, SessionManager};
use crate::export::service::{ExportService, BulkExportRequest};
use crate::export::formats::{ExportFormat, ExportOptions};
use crate::events::SessionId;
use crate::RuffError;

/// Backup manager for automated backups
pub struct BackupManager {
    config: Arc<RwLock<BackupConfig>>,
    backup_scheduler: Option<JoinHandle<()>>,
    export_service: Arc<ExportService>,
}

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: bool,
    pub interval_hours: u32,
    pub backup_path: PathBuf,
    pub max_backups: u32,
    pub compress_backups: bool,
    pub include_archived: bool,
    pub backup_format: ExportFormat,
}

/// Backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub backup_id: Uuid,
    pub created_at: DateTime<Local>,
    pub session_count: usize,
    pub total_size_bytes: u64,
    pub backup_path: PathBuf,
    pub format: ExportFormat,
    pub compressed: bool,
    pub checksum: Option<String>,
}

/// Backup result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub metadata: BackupMetadata,
    pub session_ids: Vec<SessionId>,
    pub success: bool,
    pub error_message: Option<String>,
    pub duration_ms: u64,
}

/// Backup restoration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub restored_sessions: Vec<SessionId>,
    pub skipped_sessions: Vec<SessionId>,
    pub failed_sessions: Vec<(SessionId, String)>,
    pub success: bool,
    pub duration_ms: u64,
}

/// Progress callback for long-running operations
pub type ProgressCallback = Arc<dyn Fn(usize, usize, String) + Send + Sync>;

impl Default for BackupConfig {
    fn default() -> Self {
        let backup_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ruff")
            .join("backups");

        Self {
            enabled: false,
            interval_hours: 24, // Daily backups by default
            backup_path,
            max_backups: 30, // Keep 30 backups by default
            compress_backups: true,
            include_archived: false,
            backup_format: ExportFormat::Json,
        }
    }
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new(export_service: Arc<ExportService>) -> Self {
        Self {
            config: Arc::new(RwLock::new(BackupConfig::default())),
            backup_scheduler: None,
            export_service,
        }
    }

    /// Initialize the backup manager
    pub async fn initialize(&mut self) -> Result<(), RuffError> {
        let config = self.config.read().await;
        
        // Create backup directory if it doesn't exist
        if !config.backup_path.exists() {
            fs::create_dir_all(&config.backup_path)
                .map_err(|e| RuffError::App(format!("Failed to create backup directory: {}", e)))?;
        }

        // Start scheduler if enabled
        let enabled = config.enabled;
        drop(config); // Explicitly drop the read guard
        if enabled {
            self.start_scheduler().await?;
        }

        Ok(())
    }

    /// Update backup configuration
    pub async fn update_config(&mut self, new_config: BackupConfig) -> Result<(), RuffError> {
        // Validate configuration
        if new_config.interval_hours == 0 {
            return Err(RuffError::App("Backup interval must be greater than 0".to_string()));
        }

        if new_config.max_backups == 0 {
            return Err(RuffError::App("Max backups must be greater than 0".to_string()));
        }

        // Create backup directory if it doesn't exist
        if !new_config.backup_path.exists() {
            fs::create_dir_all(&new_config.backup_path)
                .map_err(|e| RuffError::App(format!("Failed to create backup directory: {}", e)))?;
        }

        let old_enabled = {
            let config = self.config.read().await;
            config.enabled
        };
        *self.config.write().await = new_config.clone();

        // Restart scheduler if needed
        if new_config.enabled && (!old_enabled || self.backup_scheduler.is_none()) {
            self.start_scheduler().await?;
        } else if !new_config.enabled && self.backup_scheduler.is_some() {
            self.stop_scheduler().await;
        }

        Ok(())
    }

    /// Get current backup configuration
    pub async fn get_config(&self) -> BackupConfig {
        self.config.read().await.clone()
    }

    /// Start the backup scheduler
    async fn start_scheduler(&mut self) -> Result<(), RuffError> {
        // Stop existing scheduler if running
        self.stop_scheduler().await;

        let config = self.config.clone();
        let _export_service = self.export_service.clone();

        let handle = tokio::spawn(async move {
            loop {
                let current_config = config.read().await.clone();
                
                if !current_config.enabled {
                    break;
                }

                // Wait for the specified interval
                let interval = Duration::from_secs(current_config.interval_hours as u64 * 3600);
                sleep(interval).await;

                // Perform backup (this would need access to SessionManager)
                // For now, we'll just log that a backup should occur
                println!("Scheduled backup triggered at {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
            }
        });

        self.backup_scheduler = Some(handle);
        Ok(())
    }

    /// Stop the backup scheduler
    async fn stop_scheduler(&mut self) {
        if let Some(handle) = self.backup_scheduler.take() {
            handle.abort();
        }
    }

    /// Create a full backup of all sessions
    pub async fn create_backup(
        &self,
        sessions: &[ChatSession],
        progress_callback: Option<ProgressCallback>,
    ) -> Result<BackupResult, RuffError> {
        let start_time = SystemTime::now();
        let config = self.config.read().await.clone();
        
        let backup_id = Uuid::new_v4();
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let backup_dir = config.backup_path.join(format!("backup_{}_{}", timestamp, backup_id));

        // Create backup directory
        fs::create_dir_all(&backup_dir)
            .map_err(|e| RuffError::App(format!("Failed to create backup directory: {}", e)))?;

        let mut session_ids = Vec::new();
        let mut total_size = 0u64;
        let mut processed = 0;
        let _total_sessions = sessions.len();

        // Filter sessions based on configuration
        let sessions_to_backup: Vec<&ChatSession> = sessions
            .iter()
            .filter(|session| config.include_archived || !session.is_archived)
            .collect();

        let actual_total = sessions_to_backup.len();

        // Export each session
        for session in sessions_to_backup {
            if let Some(callback) = &progress_callback {
                callback(processed, actual_total, format!("Backing up session: {}", session.title));
            }

            // Create bulk export request for single session
            let export_request = BulkExportRequest {
                session_ids: vec![session.id],
                format: config.backup_format.clone(),
                options: ExportOptions {
                    include_metadata: true,
                    include_timestamps: true,
                    include_token_usage: true,
                    include_model_info: true,
                    pretty_format: false, // Compact format for backups
                },
                output_directory: Some(backup_dir.clone()),
            };

            match self.export_service.export_sessions(&[session.clone()], export_request) {
                Ok(results) => {
                    for result in results {
                        session_ids.push(session.id);
                        total_size += result.size_bytes;
                    }
                }
                Err(e) => {
                    return Err(RuffError::App(format!("Failed to backup session {}: {}", session.id, e)));
                }
            }

            processed += 1;
        }

        // Create backup metadata
        let metadata = BackupMetadata {
            backup_id,
            created_at: Local::now(),
            session_count: session_ids.len(),
            total_size_bytes: total_size,
            backup_path: backup_dir.clone(),
            format: config.backup_format.clone(),
            compressed: config.compress_backups,
            checksum: None, // TODO: Implement checksum calculation
        };

        // Save backup metadata
        let metadata_file = backup_dir.join("backup_metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| RuffError::App(format!("Failed to serialize backup metadata: {}", e)))?;
        
        fs::write(&metadata_file, metadata_json)
            .map_err(|e| RuffError::App(format!("Failed to write backup metadata: {}", e)))?;

        // Compress backup if enabled
        if config.compress_backups {
            if let Some(callback) = &progress_callback {
                callback(actual_total, actual_total, "Compressing backup...".to_string());
            }
            // TODO: Implement compression
        }

        // Clean up old backups
        self.cleanup_old_backups().await?;

        let duration = start_time.elapsed()
            .map_err(|e| RuffError::App(format!("Failed to calculate duration: {}", e)))?
            .as_millis() as u64;

        if let Some(callback) = &progress_callback {
            callback(actual_total, actual_total, "Backup completed successfully".to_string());
        }

        Ok(BackupResult {
            metadata,
            session_ids,
            success: true,
            error_message: None,
            duration_ms: duration,
        })
    }

    /// Restore sessions from a backup
    pub async fn restore_backup(
        &self,
        backup_path: &Path,
        session_manager: &mut SessionManager,
        overwrite_existing: bool,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<RestoreResult, RuffError> {
        let start_time = SystemTime::now();

        if !backup_path.exists() {
            return Err(RuffError::App("Backup path does not exist".to_string()));
        }

        // Load backup metadata
        let metadata_file = backup_path.join("backup_metadata.json");
        if !metadata_file.exists() {
            return Err(RuffError::App("Backup metadata not found".to_string()));
        }

        let metadata_content = fs::read_to_string(&metadata_file)
            .map_err(|e| RuffError::App(format!("Failed to read backup metadata: {}", e)))?;
        
        let metadata: BackupMetadata = serde_json::from_str(&metadata_content)
            .map_err(|e| RuffError::App(format!("Failed to parse backup metadata: {}", e)))?;

        // Find all session files in the backup
        let session_files = self.find_session_files(backup_path, &metadata.format)?;
        
        let mut restored_sessions = Vec::new();
        let mut skipped_sessions = Vec::new();
        let mut failed_sessions = Vec::new();
        let mut processed = 0;
        let total_files = session_files.len();

        for session_file in session_files {
            if let Some(callback) = &progress_callback {
                callback(processed, total_files, format!("Restoring: {}", session_file.display()));
            }

            match self.restore_session_from_file(&session_file, session_manager, overwrite_existing).await {
                Ok(Some(session_id)) => {
                    restored_sessions.push(session_id);
                }
                Ok(None) => {
                    // Session was skipped (already exists and overwrite_existing is false)
                    if let Some(session_id) = self.extract_session_id_from_filename(&session_file) {
                        skipped_sessions.push(session_id);
                    }
                }
                Err(e) => {
                    let session_id = self.extract_session_id_from_filename(&session_file)
                        .unwrap_or_else(|| Uuid::new_v4()); // Fallback ID
                    failed_sessions.push((session_id, e.to_string()));
                }
            }

            processed += 1;
        }

        let duration = start_time.elapsed()
            .map_err(|e| RuffError::App(format!("Failed to calculate duration: {}", e)))?
            .as_millis() as u64;

        let success = failed_sessions.is_empty();

        if let Some(callback) = &progress_callback {
            let message = if success {
                format!("Restore completed: {} restored, {} skipped", restored_sessions.len(), skipped_sessions.len())
            } else {
                format!("Restore completed with errors: {} restored, {} skipped, {} failed", 
                    restored_sessions.len(), skipped_sessions.len(), failed_sessions.len())
            };
            callback(total_files, total_files, message);
        }

        Ok(RestoreResult {
            restored_sessions,
            skipped_sessions,
            failed_sessions,
            success,
            duration_ms: duration,
        })
    }

    /// Find all session files in a backup directory
    fn find_session_files(&self, backup_path: &Path, format: &ExportFormat) -> Result<Vec<PathBuf>, RuffError> {
        let extension = match format {
            ExportFormat::Json => "json",
            ExportFormat::Markdown => "md",
            ExportFormat::Html => "html",
            ExportFormat::PlainText => "txt",
        };

        let mut session_files = Vec::new();
        let entries = fs::read_dir(backup_path)
            .map_err(|e| RuffError::App(format!("Failed to read backup directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| RuffError::App(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(file_extension) = path.extension().and_then(|ext| ext.to_str()) {
                    if file_extension == extension && path.file_name().unwrap().to_str().unwrap() != "backup_metadata.json" {
                        session_files.push(path);
                    }
                }
            }
        }

        session_files.sort();
        Ok(session_files)
    }

    /// Restore a single session from a file
    async fn restore_session_from_file(
        &self,
        session_file: &Path,
        session_manager: &mut SessionManager,
        overwrite_existing: bool,
    ) -> Result<Option<SessionId>, RuffError> {
        // For JSON format, we can directly deserialize the session
        if session_file.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let content = fs::read_to_string(session_file)
                .map_err(|e| RuffError::App(format!("Failed to read session file: {}", e)))?;
            
            let session: ChatSession = serde_json::from_str(&content)
                .map_err(|e| RuffError::App(format!("Failed to parse session: {}", e)))?;

            // Check if session already exists
            if session_manager.session_exists(session.id) && !overwrite_existing {
                return Ok(None); // Skip existing session
            }

            // Add the session to the manager
            // Note: This assumes SessionManager has a method to add a pre-existing session
            // We'll need to implement this method in SessionManager
            session_manager.add_session(session.clone()).await?;
            
            Ok(Some(session.id))
        } else {
            // For other formats, we'd need to implement parsing logic
            // For now, return an error
            Err(RuffError::App(format!("Restore from {} format not yet implemented", 
                session_file.extension().and_then(|ext| ext.to_str()).unwrap_or("unknown"))))
        }
    }

    /// Extract session ID from filename (if possible)
    fn extract_session_id_from_filename(&self, file_path: &Path) -> Option<SessionId> {
        // Try to extract UUID from filename
        if let Some(filename) = file_path.file_stem().and_then(|name| name.to_str()) {
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

    /// List all available backups
    pub async fn list_backups(&self) -> Result<Vec<BackupMetadata>, RuffError> {
        let config = self.config.read().await;
        
        if !config.backup_path.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();
        let entries = fs::read_dir(&config.backup_path)
            .map_err(|e| RuffError::App(format!("Failed to read backup directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| RuffError::App(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            
            if path.is_dir() {
                let metadata_file = path.join("backup_metadata.json");
                if metadata_file.exists() {
                    match fs::read_to_string(&metadata_file) {
                        Ok(content) => {
                            match serde_json::from_str::<BackupMetadata>(&content) {
                                Ok(metadata) => backups.push(metadata),
                                Err(e) => eprintln!("Warning: Failed to parse backup metadata in {}: {}", path.display(), e),
                            }
                        }
                        Err(e) => eprintln!("Warning: Failed to read backup metadata in {}: {}", path.display(), e),
                    }
                }
            }
        }

        // Sort by creation date (newest first)
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        Ok(backups)
    }

    /// Delete a specific backup
    pub async fn delete_backup(&self, backup_id: Uuid) -> Result<(), RuffError> {
        let backups = self.list_backups().await?;
        
        let backup = backups.iter()
            .find(|b| b.backup_id == backup_id)
            .ok_or_else(|| RuffError::App(format!("Backup {} not found", backup_id)))?;

        if backup.backup_path.exists() {
            fs::remove_dir_all(&backup.backup_path)
                .map_err(|e| RuffError::App(format!("Failed to delete backup: {}", e)))?;
        }

        Ok(())
    }

    /// Clean up old backups based on max_backups configuration
    async fn cleanup_old_backups(&self) -> Result<(), RuffError> {
        let config = self.config.read().await;
        let backups = self.list_backups().await?;

        if backups.len() > config.max_backups as usize {
            let backups_to_delete = &backups[config.max_backups as usize..];
            
            for backup in backups_to_delete {
                if let Err(e) = self.delete_backup(backup.backup_id).await {
                    eprintln!("Warning: Failed to delete old backup {}: {}", backup.backup_id, e);
                }
            }
        }

        Ok(())
    }

    /// Get backup statistics
    pub async fn get_backup_statistics(&self) -> Result<BackupStatistics, RuffError> {
        let backups = self.list_backups().await?;
        let config = self.config.read().await;

        let total_backups = backups.len();
        let total_size: u64 = backups.iter().map(|b| b.total_size_bytes).sum();
        let oldest_backup = backups.iter().min_by_key(|b| b.created_at).map(|b| b.created_at);
        let newest_backup = backups.iter().max_by_key(|b| b.created_at).map(|b| b.created_at);

        Ok(BackupStatistics {
            total_backups,
            total_size_bytes: total_size,
            backup_directory: config.backup_path.clone(),
            oldest_backup,
            newest_backup,
            auto_backup_enabled: config.enabled,
            next_backup_due: if config.enabled {
                newest_backup.map(|last| last + ChronoDuration::hours(config.interval_hours as i64))
            } else {
                None
            },
        })
    }
}

/// Backup statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupStatistics {
    pub total_backups: usize,
    pub total_size_bytes: u64,
    pub backup_directory: PathBuf,
    pub oldest_backup: Option<DateTime<Local>>,
    pub newest_backup: Option<DateTime<Local>>,
    pub auto_backup_enabled: bool,
    pub next_backup_due: Option<DateTime<Local>>,
}

impl Default for BackupManager {
    fn default() -> Self {
        // This requires an ExportService, so we can't provide a meaningful default
        // Users should use BackupManager::new() instead
        panic!("BackupManager::default() is not supported. Use BackupManager::new() instead.");
    }
}

impl BackupManager {
    /// Create a new backup manager with default configuration
    pub async fn new_default() -> Result<Self, RuffError> {
        let export_directory = dirs::data_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("ruff")
            .join("exports");
        let export_service = Arc::new(ExportService::new(export_directory));
        
        Ok(Self {
            config: Arc::new(RwLock::new(BackupConfig::default())),
            backup_scheduler: None,
            export_service,
        })
    }

    /// Create a backup with CLI parameters
    pub async fn create_backup_cli(&self, _output_path: &Path, _include_config: bool, _include_plugins: bool, _compress: bool) -> Result<BackupCreateResult, RuffError> {
        // This would create a comprehensive backup
        // For now, return a placeholder result
        Ok(BackupCreateResult {
            session_count: 0,
            backup_size: 0,
        })
    }

    /// Preview restore from backup
    pub async fn preview_restore(&self, _backup_path: &Path) -> Result<RestorePreviewResult, RuffError> {
        // This would analyze the backup file and return preview information
        // For now, return a placeholder result
        Ok(RestorePreviewResult {
            session_count: 0,
            has_config: false,
            plugin_count: 0,
        })
    }

    /// Restore from backup
    pub async fn restore_backup_cli(&self, _backup_path: &Path) -> Result<RestoreBackupResult, RuffError> {
        // This would restore from the backup file
        // For now, return a placeholder result
        Ok(RestoreBackupResult {
            restored_count: 0,
        })
    }
}

/// Result of backup creation for CLI
#[derive(Debug, Clone)]
pub struct BackupCreateResult {
    pub session_count: usize,
    pub backup_size: u64,
}

/// Result of restore preview for CLI
#[derive(Debug, Clone)]
pub struct RestorePreviewResult {
    pub session_count: usize,
    pub has_config: bool,
    pub plugin_count: usize,
}

/// Result of backup restoration for CLI
#[derive(Debug, Clone)]
pub struct RestoreBackupResult {
    pub restored_count: usize,
}

