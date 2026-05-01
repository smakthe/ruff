//! Backup management functionality

use chrono::{DateTime, Duration as ChronoDuration, Local};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::events::{EventBus, SessionId};
use crate::export::formats::{ExportFormat, ExportOptions};
use crate::export::import::{ImportOptions, ImportService};
use crate::export::service::{BulkExportRequest, ExportService};
use crate::message::manager::MessageManager;
use crate::session::manager::{ChatSession, SessionManager};
use crate::EnhancedError;

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
    pub async fn initialize(&mut self) -> Result<(), EnhancedError> {
        let config = self.config.read().await;

        // Create backup directory if it doesn't exist
        if !config.backup_path.exists() {
            fs::create_dir_all(&config.backup_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to create backup directory: {}", e))
            })?;
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
    pub async fn update_config(&mut self, new_config: BackupConfig) -> Result<(), EnhancedError> {
        // Validate configuration
        if new_config.interval_hours == 0 {
            return Err(EnhancedError::unknown(
                "Backup interval must be greater than 0".to_string(),
            ));
        }

        if new_config.max_backups == 0 {
            return Err(EnhancedError::unknown(
                "Max backups must be greater than 0".to_string(),
            ));
        }

        // Create backup directory if it doesn't exist
        if !new_config.backup_path.exists() {
            fs::create_dir_all(&new_config.backup_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to create backup directory: {}", e))
            })?;
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
    async fn start_scheduler(&mut self) -> Result<(), EnhancedError> {
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
                println!(
                    "Scheduled backup triggered at {}",
                    Local::now().format("%Y-%m-%d %H:%M:%S")
                );
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

    /// Calculate SHA-256 checksum of a directory
    fn calculate_directory_checksum(&self, dir_path: &Path) -> Result<String, EnhancedError> {
        let mut hasher = Sha256::new();
        let mut file_paths: Vec<PathBuf> = Vec::new();

        // Collect all files recursively
        self.collect_files_recursive(dir_path, &mut file_paths)?;

        // Sort for consistent ordering
        file_paths.sort();

        // Hash each file
        for file_path in file_paths {
            if file_path.file_name().and_then(|name| name.to_str()) == Some("backup_metadata.json")
            {
                continue;
            }

            // Hash the relative path first (for structure integrity)
            let relative_path = file_path.strip_prefix(dir_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to get relative path: {}", e))
            })?;
            hasher.update(relative_path.to_string_lossy().as_bytes());

            // Hash the file content
            let mut file = File::open(&file_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to open file for checksum: {}", e))
            })?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).map_err(|e| {
                EnhancedError::storage(format!("Failed to read file for checksum: {}", e))
            })?;
            hasher.update(&buffer);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Collect all files in a directory recursively
    fn collect_files_recursive(
        &self,
        dir_path: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), EnhancedError> {
        if !dir_path.is_dir() {
            return Ok(());
        }

        let entries = fs::read_dir(dir_path)
            .map_err(|e| EnhancedError::storage(format!("Failed to read directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                EnhancedError::storage(format!("Failed to read directory entry: {}", e))
            })?;
            let path = entry.path();

            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                self.collect_files_recursive(&path, files)?;
            }
        }

        Ok(())
    }

    /// Compress a directory into a .tar.gz archive
    fn compress_backup_directory(&self, dir_path: &Path) -> Result<PathBuf, EnhancedError> {
        let compressed_path = dir_path.with_extension("tar.gz");

        // Create the compressed file
        let tar_gz = File::create(&compressed_path).map_err(|e| {
            EnhancedError::storage(format!("Failed to create compressed file: {}", e))
        })?;
        let enc = GzEncoder::new(tar_gz, Compression::default());
        let mut tar = tar::Builder::new(enc);

        // Add all files to the archive
        let dir_name = dir_path
            .file_name()
            .ok_or_else(|| EnhancedError::storage("Invalid directory path".to_string()))?;

        tar.append_dir_all(dir_name, dir_path).map_err(|e| {
            EnhancedError::storage(format!("Failed to add directory to archive: {}", e))
        })?;

        tar.finish()
            .map_err(|e| EnhancedError::storage(format!("Failed to finalize archive: {}", e)))?;

        // Remove the original directory
        fs::remove_dir_all(dir_path).map_err(|e| {
            EnhancedError::storage(format!("Failed to remove uncompressed backup: {}", e))
        })?;

        Ok(compressed_path)
    }

    /// Decompress a .tar.gz archive
    fn decompress_backup_archive(&self, archive_path: &Path) -> Result<PathBuf, EnhancedError> {
        let file = File::open(archive_path).map_err(|e| {
            EnhancedError::storage(format!("Failed to open compressed backup: {}", e))
        })?;

        let tar = GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);

        // Extract to the parent directory
        let parent_dir = archive_path
            .parent()
            .ok_or_else(|| EnhancedError::storage("Invalid archive path".to_string()))?;

        archive
            .unpack(parent_dir)
            .map_err(|e| EnhancedError::storage(format!("Failed to extract archive: {}", e)))?;

        // Determine the extracted directory name
        let dir_name = archive_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| EnhancedError::storage("Invalid archive filename".to_string()))?;

        // Remove .tar from name if present
        let dir_name = if dir_name.ends_with(".tar") {
            &dir_name[..dir_name.len() - 4]
        } else {
            dir_name
        };

        Ok(parent_dir.join(dir_name))
    }

    /// Create a full backup of all sessions
    pub async fn create_backup(
        &self,
        sessions: &[ChatSession],
        message_manager: &crate::message::manager::MessageManager,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<BackupResult, EnhancedError> {
        let start_time = SystemTime::now();
        let config = self.config.read().await.clone();

        let backup_id = Uuid::new_v4();
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let backup_dir = config
            .backup_path
            .join(format!("backup_{}_{}", timestamp, backup_id));

        // Create backup directory
        fs::create_dir_all(&backup_dir).map_err(|e| {
            EnhancedError::storage(format!("Failed to create backup directory: {}", e))
        })?;

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
                callback(
                    processed,
                    actual_total,
                    format!("Backing up session: {}", session.title),
                );
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

            match self.export_service.export_sessions(
                &[session.clone()],
                export_request,
                message_manager,
            ) {
                Ok(results) => {
                    for result in results {
                        session_ids.push(session.id);
                        total_size += result.size_bytes;
                    }
                }
                Err(e) => {
                    return Err(EnhancedError::unknown(format!(
                        "Failed to backup session {}: {}",
                        session.id, e
                    )));
                }
            }

            processed += 1;
        }

        // Calculate checksum before compression
        if let Some(callback) = &progress_callback {
            callback(
                actual_total,
                actual_total,
                "Calculating checksum...".to_string(),
            );
        }

        let checksum = self.calculate_directory_checksum(&backup_dir)?;

        // Create backup metadata
        let mut metadata = BackupMetadata {
            backup_id,
            created_at: Local::now(),
            session_count: session_ids.len(),
            total_size_bytes: total_size,
            backup_path: backup_dir.clone(),
            format: config.backup_format.clone(),
            compressed: config.compress_backups,
            checksum: Some(checksum),
        };

        // Save backup metadata
        let metadata_file = backup_dir.join("backup_metadata.json");
        let metadata_json =
            serde_json::to_string_pretty(&metadata).map_err(|e| EnhancedError::from(e))?;

        fs::write(&metadata_file, metadata_json).map_err(|e| {
            EnhancedError::storage(format!("Failed to write backup metadata: {}", e))
        })?;

        // Compress backup if enabled
        if config.compress_backups {
            if let Some(callback) = &progress_callback {
                callback(
                    actual_total,
                    actual_total,
                    "Compressing backup...".to_string(),
                );
            }

            let compressed_path = self.compress_backup_directory(&backup_dir)?;

            // Update metadata with compressed path
            metadata.backup_path = compressed_path;
        }

        // Clean up old backups
        self.cleanup_old_backups().await?;

        let duration = start_time
            .elapsed()
            .map_err(|e| EnhancedError::unknown(format!("Failed to calculate duration: {}", e)))?
            .as_millis() as u64;

        if let Some(callback) = &progress_callback {
            callback(
                actual_total,
                actual_total,
                "Backup completed successfully".to_string(),
            );
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
    ) -> Result<RestoreResult, EnhancedError> {
        let start_time = SystemTime::now();

        if !backup_path.exists() {
            return Err(EnhancedError::storage(format!(
                "Backup path does not exist: {}",
                backup_path.display()
            )));
        }

        // Check if the backup is compressed
        let actual_backup_path = if backup_path.extension().and_then(|e| e.to_str()) == Some("gz") {
            if let Some(callback) = &progress_callback {
                callback(0, 100, "Decompressing backup...".to_string());
            }

            // Decompress the archive
            let decompressed_path = self.decompress_backup_archive(backup_path)?;
            decompressed_path
        } else {
            backup_path.to_path_buf()
        };

        // Load backup metadata
        let metadata_file = actual_backup_path.join("backup_metadata.json");
        if !metadata_file.exists() {
            return Err(EnhancedError::storage(format!(
                "Backup metadata not found in: {}",
                actual_backup_path.display()
            )));
        }

        let metadata_content = fs::read_to_string(&metadata_file).map_err(|e| {
            EnhancedError::storage(format!("Failed to read backup metadata: {}", e))
        })?;

        let metadata: BackupMetadata = serde_json::from_str(&metadata_content).map_err(|e| {
            EnhancedError::parsing(format!("Failed to parse backup metadata: {}", e))
        })?;

        // Verify checksum if present
        if let Some(expected_checksum) = &metadata.checksum {
            if let Some(callback) = &progress_callback {
                callback(10, 100, "Verifying backup integrity...".to_string());
            }

            let actual_checksum = self.calculate_directory_checksum(&actual_backup_path)?;
            if actual_checksum != *expected_checksum {
                return Err(EnhancedError::storage(format!(
                    "Backup checksum mismatch! Expected: {}, Got: {}. Backup may be corrupted.",
                    expected_checksum, actual_checksum
                )));
            }
        }

        // Find all session files in the backup
        let session_files = self.find_session_files(&actual_backup_path, &metadata.format)?;

        let mut restored_sessions = Vec::new();
        let mut skipped_sessions = Vec::new();
        let mut failed_sessions = Vec::new();
        let mut processed = 0;
        let total_files = session_files.len();

        for session_file in session_files {
            if let Some(callback) = &progress_callback {
                callback(
                    processed,
                    total_files,
                    format!("Restoring: {}", session_file.display()),
                );
            }

            match self
                .restore_session_from_file(&session_file, session_manager, overwrite_existing)
                .await
            {
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
                    let session_id = self
                        .extract_session_id_from_filename(&session_file)
                        .unwrap_or_else(|| Uuid::new_v4()); // Fallback ID
                    failed_sessions.push((session_id, e.to_string()));
                }
            }

            processed += 1;
        }

        let duration = start_time
            .elapsed()
            .map_err(|e| EnhancedError::unknown(format!("Failed to calculate duration: {}", e)))?
            .as_millis() as u64;

        let success = failed_sessions.is_empty();

        if let Some(callback) = &progress_callback {
            let message = if success {
                format!(
                    "Restore completed: {} restored, {} skipped",
                    restored_sessions.len(),
                    skipped_sessions.len()
                )
            } else {
                format!(
                    "Restore completed with errors: {} restored, {} skipped, {} failed",
                    restored_sessions.len(),
                    skipped_sessions.len(),
                    failed_sessions.len()
                )
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
    fn find_session_files(
        &self,
        backup_path: &Path,
        format: &ExportFormat,
    ) -> Result<Vec<PathBuf>, EnhancedError> {
        let extension = match format {
            ExportFormat::Json => "json",
            ExportFormat::Markdown => "md",
            ExportFormat::Html => "html",
            ExportFormat::PlainText => "txt",
        };

        let mut session_files = Vec::new();
        let entries = fs::read_dir(backup_path).map_err(|e| {
            EnhancedError::storage(format!("Failed to read backup directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                EnhancedError::storage(format!("Failed to read directory entry: {}", e))
            })?;
            let path = entry.path();

            if path.is_file() {
                if let Some(file_extension) = path.extension().and_then(|ext| ext.to_str()) {
                    if file_extension == extension
                        && path.file_name().unwrap().to_str().unwrap() != "backup_metadata.json"
                    {
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
    ) -> Result<Option<SessionId>, EnhancedError> {
        // For JSON format, we can directly deserialize the session
        if session_file.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let content = fs::read_to_string(session_file).map_err(|e| {
                EnhancedError::storage(format!("Failed to read session file: {}", e))
            })?;

            let session: ChatSession = serde_json::from_str(&content)
                .map_err(|e| EnhancedError::unknown(format!("Failed to parse session: {}", e)))?;

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
            Err(EnhancedError::unknown(format!(
                "Unsupported restore format: {}",
                session_file
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("unknown")
            )))
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
    pub async fn list_backups(&self) -> Result<Vec<BackupMetadata>, EnhancedError> {
        let config = self.config.read().await;

        if !config.backup_path.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();
        let entries = fs::read_dir(&config.backup_path).map_err(|e| {
            EnhancedError::storage(format!("Failed to read backup directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                EnhancedError::storage(format!("Failed to read directory entry: {}", e))
            })?;
            let path = entry.path();

            if path.is_dir() {
                let metadata_file = path.join("backup_metadata.json");
                if metadata_file.exists() {
                    match fs::read_to_string(&metadata_file) {
                        Ok(content) => match serde_json::from_str::<BackupMetadata>(&content) {
                            Ok(metadata) => backups.push(metadata),
                            Err(e) => eprintln!(
                                "Warning: Failed to parse backup metadata in {}: {}",
                                path.display(),
                                e
                            ),
                        },
                        Err(e) => eprintln!(
                            "Warning: Failed to read backup metadata in {}: {}",
                            path.display(),
                            e
                        ),
                    }
                }
            }
        }

        // Sort by creation date (newest first)
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    /// Delete a specific backup
    pub async fn delete_backup(&self, backup_id: Uuid) -> Result<(), EnhancedError> {
        let backups = self.list_backups().await?;

        let backup = backups
            .iter()
            .find(|b| b.backup_id == backup_id)
            .ok_or_else(|| EnhancedError::unknown(format!("Backup {} not found", backup_id)))?;

        if backup.backup_path.exists() {
            fs::remove_dir_all(&backup.backup_path)
                .map_err(|e| EnhancedError::storage(format!("Failed to delete backup: {}", e)))?;
        }

        Ok(())
    }

    /// Clean up old backups based on max_backups configuration
    async fn cleanup_old_backups(&self) -> Result<(), EnhancedError> {
        let config = self.config.read().await;
        let backups = self.list_backups().await?;

        if backups.len() > config.max_backups as usize {
            let backups_to_delete = &backups[config.max_backups as usize..];

            for backup in backups_to_delete {
                if let Err(e) = self.delete_backup(backup.backup_id).await {
                    eprintln!(
                        "Warning: Failed to delete old backup {}: {}",
                        backup.backup_id, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Get backup statistics
    pub async fn get_backup_statistics(&self) -> Result<BackupStatistics, EnhancedError> {
        let backups = self.list_backups().await?;
        let config = self.config.read().await;

        let total_backups = backups.len();
        let total_size: u64 = backups.iter().map(|b| b.total_size_bytes).sum();
        let oldest_backup = backups
            .iter()
            .min_by_key(|b| b.created_at)
            .map(|b| b.created_at);
        let newest_backup = backups
            .iter()
            .max_by_key(|b| b.created_at)
            .map(|b| b.created_at);

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
    pub async fn new_default() -> Result<Self, EnhancedError> {
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
    pub async fn create_backup_cli(
        &self,
        _output_path: &Path,
        _include_config: bool,
        _include_plugins: bool,
        _compress: bool,
    ) -> Result<BackupCreateResult, EnhancedError> {
        let output_path = _output_path;
        let include_config = _include_config;
        let include_plugins = _include_plugins;
        let compress = _compress;

        let session_manager = SessionManager::new_default().await?;
        let message_manager = MessageManager::new_default()?;
        let sessions: Vec<ChatSession> = session_manager
            .get_all_sessions()
            .into_iter()
            .cloned()
            .collect();
        let session_ids: Vec<SessionId> = sessions.iter().map(|session| session.id).collect();

        let working_dir = if compress {
            let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
            parent.join(format!(".ruff-backup-{}", Uuid::new_v4()))
        } else {
            output_path.to_path_buf()
        };

        fs::create_dir_all(&working_dir).map_err(|e| {
            EnhancedError::storage(format!("Failed to create backup directory: {}", e))
        })?;

        let export_request = BulkExportRequest {
            session_ids,
            format: ExportFormat::Json,
            options: ExportOptions {
                include_metadata: true,
                include_timestamps: true,
                include_model_info: true,
                include_token_usage: true,
                pretty_format: true,
            },
            output_directory: Some(working_dir.clone()),
        };

        let export_results =
            self.export_service
                .export_sessions(&sessions, export_request, &message_manager)?;
        let total_size: u64 = export_results.iter().map(|result| result.size_bytes).sum();

        if include_config {
            if let Some(config_dir) = dirs::config_dir().map(|dir| dir.join("ruff")) {
                if config_dir.exists() {
                    self.copy_dir_recursive(&config_dir, &working_dir.join("config"))?;
                }
            }
        }

        if include_plugins {
            if let Some(plugin_dir) = dirs::config_dir().map(|dir| dir.join("ruff").join("plugins"))
            {
                if plugin_dir.exists() {
                    self.copy_dir_recursive(&plugin_dir, &working_dir.join("plugins"))?;
                }
            }
        }

        let metadata = BackupMetadata {
            backup_id: Uuid::new_v4(),
            created_at: Local::now(),
            session_count: export_results.len(),
            total_size_bytes: total_size,
            backup_path: if compress {
                output_path.to_path_buf()
            } else {
                working_dir.clone()
            },
            format: ExportFormat::Json,
            compressed: compress,
            checksum: Some(self.calculate_directory_checksum(&working_dir)?),
        };

        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(working_dir.join("backup_metadata.json"), metadata_json).map_err(|e| {
            EnhancedError::storage(format!("Failed to write backup metadata: {}", e))
        })?;

        let backup_size = if compress {
            self.compress_directory_to_path(&working_dir, output_path)?;
            let size = fs::metadata(output_path)
                .map_err(|e| {
                    EnhancedError::storage(format!("Failed to stat backup archive: {}", e))
                })?
                .len();
            fs::remove_dir_all(&working_dir).map_err(|e| {
                EnhancedError::storage(format!(
                    "Failed to remove temporary backup directory: {}",
                    e
                ))
            })?;
            size
        } else {
            self.directory_size(&working_dir)?
        };

        Ok(BackupCreateResult {
            session_count: export_results.len(),
            backup_size,
        })
    }

    /// Preview restore from backup
    pub async fn preview_restore(
        &self,
        _backup_path: &Path,
    ) -> Result<RestorePreviewResult, EnhancedError> {
        let backup_path = _backup_path;
        let actual_backup_path =
            if backup_path.extension().and_then(|ext| ext.to_str()) == Some("gz") {
                self.decompress_backup_archive(backup_path)?
            } else {
                backup_path.to_path_buf()
            };

        let metadata_path = actual_backup_path.join("backup_metadata.json");
        let metadata = if metadata_path.exists() {
            let content = fs::read_to_string(&metadata_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to read backup metadata: {}", e))
            })?;
            Some(serde_json::from_str::<BackupMetadata>(&content)?)
        } else {
            None
        };

        let session_files = self.find_session_files(&actual_backup_path, &ExportFormat::Json)?;
        let has_config = actual_backup_path.join("config").exists();
        let plugin_count = if actual_backup_path.join("plugins").exists() {
            fs::read_dir(actual_backup_path.join("plugins"))
                .map_err(|e| {
                    EnhancedError::storage(format!("Failed to read plugin backup directory: {}", e))
                })?
                .filter_map(Result::ok)
                .count()
        } else {
            0
        };

        Ok(RestorePreviewResult {
            session_count: metadata
                .map(|m| m.session_count)
                .unwrap_or(session_files.len()),
            has_config,
            plugin_count,
        })
    }

    /// Restore from backup
    pub async fn restore_backup_cli(
        &self,
        _backup_path: &Path,
    ) -> Result<RestoreBackupResult, EnhancedError> {
        let backup_path = _backup_path;
        let actual_backup_path =
            if backup_path.extension().and_then(|ext| ext.to_str()) == Some("gz") {
                self.decompress_backup_archive(backup_path)?
            } else {
                backup_path.to_path_buf()
            };

        let metadata_path = actual_backup_path.join("backup_metadata.json");
        if metadata_path.exists() {
            let content = fs::read_to_string(&metadata_path).map_err(|e| {
                EnhancedError::storage(format!("Failed to read backup metadata: {}", e))
            })?;
            let metadata: BackupMetadata = serde_json::from_str(&content)?;

            if let Some(expected_checksum) = metadata.checksum {
                let actual_checksum = self.calculate_directory_checksum(&actual_backup_path)?;
                if expected_checksum != actual_checksum {
                    return Err(EnhancedError::storage(format!(
                        "Backup checksum mismatch! Expected: {}, Got: {}",
                        expected_checksum, actual_checksum
                    )));
                }
            }
        }

        let mut session_manager = SessionManager::new_default().await?;
        let mut message_manager = MessageManager::new_default()?;
        let mut import_service = ImportService::new(EventBus::new());
        let mut restored_count = 0;

        for session_file in self.find_session_files(&actual_backup_path, &ExportFormat::Json)? {
            let result = import_service
                .import_from_file_into_managers(
                    &session_file,
                    Some(crate::export::formats::ImportFormat::Json),
                    ImportOptions::default(),
                    &mut session_manager,
                    &mut message_manager,
                )
                .await?;
            restored_count += result.imported_sessions.len();
        }

        Ok(RestoreBackupResult { restored_count })
    }

    fn compress_directory_to_path(
        &self,
        source_dir: &Path,
        archive_path: &Path,
    ) -> Result<(), EnhancedError> {
        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                EnhancedError::storage(format!("Failed to create backup archive directory: {}", e))
            })?;
        }

        let tar_gz = File::create(archive_path).map_err(|e| {
            EnhancedError::storage(format!("Failed to create compressed backup: {}", e))
        })?;
        let encoder = GzEncoder::new(tar_gz, Compression::default());
        let mut archive = tar::Builder::new(encoder);
        let mut dir_name = archive_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("backup")
            .to_string();
        if dir_name.ends_with(".tar") {
            dir_name.truncate(dir_name.len() - 4);
        }

        archive.append_dir_all(&dir_name, source_dir).map_err(|e| {
            EnhancedError::storage(format!("Failed to add files to backup archive: {}", e))
        })?;
        archive.finish().map_err(|e| {
            EnhancedError::storage(format!("Failed to finish backup archive: {}", e))
        })?;

        Ok(())
    }

    fn copy_dir_recursive(&self, source: &Path, destination: &Path) -> Result<(), EnhancedError> {
        fs::create_dir_all(destination).map_err(|e| {
            EnhancedError::storage(format!("Failed to create backup copy directory: {}", e))
        })?;

        for entry in fs::read_dir(source).map_err(|e| {
            EnhancedError::storage(format!("Failed to read directory for backup: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                EnhancedError::storage(format!("Failed to read directory entry for backup: {}", e))
            })?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());

            if source_path.is_dir() {
                self.copy_dir_recursive(&source_path, &destination_path)?;
            } else {
                fs::copy(&source_path, &destination_path).map_err(|e| {
                    EnhancedError::storage(format!("Failed to copy backup file: {}", e))
                })?;
            }
        }

        Ok(())
    }

    fn directory_size(&self, path: &Path) -> Result<u64, EnhancedError> {
        let mut files = Vec::new();
        self.collect_files_recursive(path, &mut files)?;

        files.into_iter().try_fold(0u64, |total, file| {
            let size = fs::metadata(&file)
                .map_err(|e| EnhancedError::storage(format!("Failed to stat backup file: {}", e)))?
                .len();
            Ok(total + size)
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
