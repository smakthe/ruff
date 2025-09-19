//! Import service implementation

use std::collections::HashMap;
use std::path::Path;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::events::{EventBus, SessionId};
use crate::session::manager::{ChatSession, Message, MessageRole, MessageMetadata, ModelConfig};
use crate::models::TokenUsage;
use crate::export::formats::ImportFormat;
use crate::export::validation::{DataValidator, ValidationResult};
use crate::RuffError;

/// Import service for handling data imports from various chat applications
pub struct ImportService {
    event_bus: EventBus,
    validator: DataValidator,
}

/// Import result containing information about the import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported_sessions: Vec<SessionId>,
    pub total_messages: usize,
    pub skipped_messages: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub preview_data: Option<ImportPreview>,
}

/// Preview data for imported conversations before actual import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub session_count: usize,
    pub total_messages: usize,
    pub sessions: Vec<SessionPreview>,
    pub format_detected: ImportFormat,
    pub validation_result: ValidationResult,
}

/// Preview information for a single session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPreview {
    pub title: String,
    pub message_count: usize,
    pub created_at: Option<DateTime<Local>>,
    pub model: Option<String>,
    pub first_message_preview: Option<String>,
}

/// Import options for controlling the import process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    pub merge_with_existing: bool,
    pub preserve_timestamps: bool,
    pub auto_generate_titles: bool,
    pub skip_invalid_messages: bool,
    pub max_message_length: Option<usize>,
    pub default_model: String,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            merge_with_existing: false,
            preserve_timestamps: true,
            auto_generate_titles: true,
            skip_invalid_messages: true,
            max_message_length: Some(100_000), // 100KB per message
            default_model: "openai-gpt3.5".to_string(),
        }
    }
}

/// External format structures for parsing different chat exports

/// ChatGPT export format structure
#[derive(Debug, Deserialize)]
struct ChatGptExport {
    title: String,
    create_time: Option<f64>,
    update_time: Option<f64>,
    mapping: HashMap<String, ChatGptNode>,
    moderation_results: Option<Vec<Value>>,
    current_node: Option<String>,
    plugin_ids: Option<Vec<String>>,
    conversation_id: Option<String>,
    conversation_template_id: Option<String>,
    gizmo_id: Option<String>,
    is_archived: Option<bool>,
    safe_urls: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ChatGptNode {
    id: String,
    message: Option<ChatGptMessage>,
    parent: Option<String>,
    children: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ChatGptMessage {
    id: String,
    author: ChatGptAuthor,
    create_time: Option<f64>,
    update_time: Option<f64>,
    content: ChatGptContent,
    status: Option<String>,
    end_turn: Option<bool>,
    weight: Option<f64>,
    metadata: Option<Value>,
    recipient: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatGptAuthor {
    role: String,
    name: Option<String>,
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ChatGptContent {
    content_type: String,
    parts: Vec<String>,
}

/// Claude export format structure
#[derive(Debug, Deserialize)]
struct ClaudeExport {
    uuid: String,
    name: String,
    summary: Option<String>,
    model: Option<String>,
    created_at: String,
    updated_at: String,
    chat_messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    uuid: String,
    text: String,
    sender: String,
    index: Option<i32>,
    created_at: String,
    updated_at: Option<String>,
    edited_at: Option<String>,
    chat_feedback: Option<Value>,
    attachments: Option<Vec<Value>>,
}

/// Generic JSON format for Ruff exports and other compatible formats
#[derive(Debug, Deserialize)]
struct GenericJsonExport {
    #[serde(flatten)]
    session: Option<ChatSession>,
    sessions: Option<Vec<ChatSession>>,
    title: Option<String>,
    messages: Option<Vec<GenericMessage>>,
    model: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenericMessage {
    role: String,
    content: String,
    timestamp: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

impl ImportService {
    /// Create a new import service
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            event_bus,
            validator: DataValidator::new(),
        }
    }

    /// Preview import data without actually importing
    pub async fn preview_import(&self, file_path: &Path, format: Option<ImportFormat>) -> Result<ImportPreview, RuffError> {
        let content = tokio::fs::read_to_string(file_path).await
            .map_err(|e| RuffError::ImportFailed { 
                reason: format!("Failed to read file: {}", e) 
            })?;

        let detected_format = if let Some(fmt) = format {
            fmt
        } else {
            self.detect_format(&content, file_path)?
        };

        let sessions = self.parse_content(&content, detected_format.clone())?;
        let validation_result = self.validator.validate_sessions(&sessions);

        let session_previews: Vec<SessionPreview> = sessions.iter().map(|session| {
            let first_message_preview = session.messages.first()
                .map(|msg| {
                    let preview = if msg.content.len() > 100 {
                        format!("{}...", &msg.content[..97])
                    } else {
                        msg.content.clone()
                    };
                    preview
                });

            SessionPreview {
                title: session.title.clone(),
                message_count: session.messages.len(),
                created_at: Some(session.created_at),
                model: Some(session.model.clone()),
                first_message_preview,
            }
        }).collect();

        let total_messages = sessions.iter().map(|s| s.messages.len()).sum();

        Ok(ImportPreview {
            session_count: sessions.len(),
            total_messages,
            sessions: session_previews,
            format_detected: detected_format,
            validation_result,
        })
    }

    /// Import conversations from a file
    pub async fn import_from_file(&mut self, file_path: &Path, format: Option<ImportFormat>, options: ImportOptions) -> Result<ImportResult, RuffError> {
        let content = tokio::fs::read_to_string(file_path).await
            .map_err(|e| RuffError::ImportFailed { 
                reason: format!("Failed to read file: {}", e) 
            })?;

        self.import_from_content(&content, format, options).await
    }

    /// Import conversations from content string
    pub async fn import_from_content(&mut self, content: &str, format: Option<ImportFormat>, options: ImportOptions) -> Result<ImportResult, RuffError> {
        let detected_format = if let Some(fmt) = format {
            fmt
        } else {
            self.detect_format(content, Path::new("unknown"))?
        };

        let sessions = self.parse_content(content, detected_format.clone())?;
        let validation_result = self.validator.validate_sessions(&sessions);

        let mut import_result = ImportResult {
            imported_sessions: Vec::new(),
            total_messages: 0,
            skipped_messages: 0,
            errors: validation_result.errors.clone(),
            warnings: validation_result.warnings.clone(),
            preview_data: None,
        };

        // Process each session
        for mut session in sessions {
            match self.process_session(&mut session, &options).await {
                Ok(session_id) => {
                    import_result.imported_sessions.push(session_id);
                    import_result.total_messages += session.messages.len();
                    
                    // Publish import event
                    if let Err(e) = self.event_bus.publish(crate::events::AppEvent::SessionImported { 
                        session_id, 
                        source_format: format!("{:?}", detected_format) 
                    }).await {
                        import_result.warnings.push(format!("Failed to publish import event: {}", e));
                    }
                }
                Err(e) => {
                    import_result.errors.push(format!("Failed to import session '{}': {}", session.title, e));
                }
            }
        }

        Ok(import_result)
    }

    /// Detect the format of the import data
    pub fn detect_format(&self, content: &str, file_path: &Path) -> Result<ImportFormat, RuffError> {
        // Try to parse as JSON first
        if let Ok(json_value) = serde_json::from_str::<Value>(content) {
            // Check for ChatGPT export format
            if json_value.get("mapping").is_some() && json_value.get("title").is_some() {
                return Ok(ImportFormat::ChatGptExport);
            }
            
            // Check for Claude export format
            if json_value.get("chat_messages").is_some() && json_value.get("uuid").is_some() {
                return Ok(ImportFormat::ClaudeExport);
            }
            
            // Check for Ruff/generic JSON format
            if json_value.get("session").is_some() || 
               json_value.get("sessions").is_some() || 
               json_value.get("messages").is_some() {
                return Ok(ImportFormat::Json);
            }
        }

        // Check file extension as fallback
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            match extension.to_lowercase().as_str() {
                "json" => Ok(ImportFormat::Json),
                _ => Err(RuffError::ImportFailed { 
                    reason: "Unable to detect import format".to_string() 
                }),
            }
        } else {
            Err(RuffError::ImportFailed { 
                reason: "Unable to detect import format from content or file extension".to_string() 
            })
        }
    }

    /// Parse content based on detected format
    fn parse_content(&self, content: &str, format: ImportFormat) -> Result<Vec<ChatSession>, RuffError> {
        match format {
            ImportFormat::ChatGptExport => self.parse_chatgpt_export(content),
            ImportFormat::ClaudeExport => self.parse_claude_export(content),
            ImportFormat::Json => self.parse_json_export(content),
        }
    }

    /// Parse ChatGPT export format
    fn parse_chatgpt_export(&self, content: &str) -> Result<Vec<ChatSession>, RuffError> {
        let export: ChatGptExport = serde_json::from_str(content)
            .map_err(|e| RuffError::ImportFailed { 
                reason: format!("Failed to parse ChatGPT export: {}", e) 
            })?;

        let session_id = Uuid::new_v4();
        let created_at = export.create_time
            .and_then(|ts| DateTime::from_timestamp(ts as i64, 0))
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(Local::now);
        
        let updated_at = export.update_time
            .and_then(|ts| DateTime::from_timestamp(ts as i64, 0))
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or(created_at);

        // Build message tree from mapping
        let mut messages = Vec::new();
        let mut processed_nodes = std::collections::HashSet::new();

        // Find root nodes (nodes without parents or with null parents)
        let root_nodes: Vec<_> = export.mapping.values()
            .filter(|node| node.parent.is_none() || node.parent.as_ref().map(|p| p.is_empty()).unwrap_or(true))
            .collect();

        // Process messages in conversation order
        for root_node in root_nodes {
            self.process_chatgpt_node_tree(root_node, &export.mapping, &mut messages, &mut processed_nodes)?;
        }

        // Filter out messages without content
        messages.retain(|msg| !msg.content.trim().is_empty());

        let session = ChatSession {
            id: session_id,
            title: export.title,
            created_at,
            updated_at,
            messages,
            model: "openai-gpt3.5".to_string(), // Default for ChatGPT exports
            system_prompt: None,
            model_config: ModelConfig::default(),
            total_tokens_used: TokenUsage::default(),
            tags: Vec::new(),
            is_archived: export.is_archived.unwrap_or(false),
            export_count: 0,
            message_count: 0,
            last_activity: updated_at,
        };

        Ok(vec![session])
    }

    /// Process ChatGPT node tree recursively
    fn process_chatgpt_node_tree(
        &self,
        node: &ChatGptNode,
        mapping: &HashMap<String, ChatGptNode>,
        messages: &mut Vec<Message>,
        processed: &mut std::collections::HashSet<String>,
    ) -> Result<(), RuffError> {
        if processed.contains(&node.id) {
            return Ok(());
        }
        processed.insert(node.id.clone());

        // Process current node if it has a message
        if let Some(ref msg) = node.message {
            if let Some(message) = self.convert_chatgpt_message(msg)? {
                messages.push(message);
            }
        }

        // Process children in order
        for child_id in &node.children {
            if let Some(child_node) = mapping.get(child_id) {
                self.process_chatgpt_node_tree(child_node, mapping, messages, processed)?;
            }
        }

        Ok(())
    }

    /// Convert ChatGPT message to Ruff message format
    fn convert_chatgpt_message(&self, msg: &ChatGptMessage) -> Result<Option<Message>, RuffError> {
        // Skip messages without content
        if msg.content.parts.is_empty() {
            return Ok(None);
        }

        let role = match msg.author.role.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            _ => return Ok(None), // Skip unknown roles
        };

        let content = msg.content.parts.join("\n");
        if content.trim().is_empty() {
            return Ok(None);
        }

        let timestamp = msg.create_time
            .and_then(|ts| DateTime::from_timestamp(ts as i64, 0))
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(Local::now);

        let edited_at = msg.update_time
            .filter(|&update_time| msg.create_time.map_or(false, |create_time| update_time > create_time))
            .and_then(|ts| DateTime::from_timestamp(ts as i64, 0))
            .map(|dt| dt.with_timezone(&Local));

        Ok(Some(Message {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp,
            edited_at,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "gpt-3.5-turbo".to_string(), // Default for ChatGPT
                temperature: 0.7,
                response_time_ms: 0,
                is_regenerated: false,
                regeneration_count: 0,
            },
        }))
    }

    /// Parse Claude export format
    fn parse_claude_export(&self, content: &str) -> Result<Vec<ChatSession>, RuffError> {
        let export: ClaudeExport = serde_json::from_str(content)
            .map_err(|e| RuffError::ImportFailed { 
                reason: format!("Failed to parse Claude export: {}", e) 
            })?;

        let session_id = Uuid::new_v4();
        let created_at = self.parse_timestamp(&export.created_at)?;
        let updated_at = self.parse_timestamp(&export.updated_at)?;

        let mut messages = Vec::new();
        for claude_msg in export.chat_messages {
            if let Some(message) = self.convert_claude_message(&claude_msg)? {
                messages.push(message);
            }
        }

        // Sort messages by index if available, otherwise by timestamp
        messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        let session = ChatSession {
            id: session_id,
            title: export.name,
            created_at,
            updated_at,
            messages,
            model: export.model.unwrap_or_else(|| "claude-3-sonnet".to_string()),
            system_prompt: None,
            model_config: ModelConfig::default(),
            total_tokens_used: TokenUsage::default(),
            tags: Vec::new(),
            is_archived: false,
            export_count: 0,
            message_count: 0,
            last_activity: updated_at,
        };

        Ok(vec![session])
    }

    /// Convert Claude message to Ruff message format
    fn convert_claude_message(&self, msg: &ClaudeMessage) -> Result<Option<Message>, RuffError> {
        if msg.text.trim().is_empty() {
            return Ok(None);
        }

        let role = match msg.sender.as_str() {
            "human" | "user" => MessageRole::User,
            "assistant" | "claude" => MessageRole::Assistant,
            "system" => MessageRole::System,
            _ => return Ok(None), // Skip unknown roles
        };

        let timestamp = self.parse_timestamp(&msg.created_at)?;
        let edited_at = msg.edited_at.as_ref()
            .map(|ts| self.parse_timestamp(ts))
            .transpose()?;

        Ok(Some(Message {
            id: Uuid::new_v4(),
            role,
            content: msg.text.clone(),
            timestamp,
            edited_at,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "claude-3-sonnet".to_string(), // Default for Claude
                temperature: 0.7,
                response_time_ms: 0,
                is_regenerated: false,
                regeneration_count: 0,
            },
        }))
    }

    /// Parse generic JSON export format
    fn parse_json_export(&self, content: &str) -> Result<Vec<ChatSession>, RuffError> {
        let export: GenericJsonExport = serde_json::from_str(content)
            .map_err(|e| RuffError::ImportFailed { 
                reason: format!("Failed to parse JSON export: {}", e) 
            })?;

        // Handle different JSON structures
        if let Some(session) = export.session {
            // Single session format
            Ok(vec![session])
        } else if let Some(sessions) = export.sessions {
            // Multiple sessions format
            Ok(sessions)
        } else if let Some(messages) = export.messages {
            // Messages-only format, create a session
            let session_id = Uuid::new_v4();
            let now = Local::now();
            
            let mut converted_messages = Vec::new();
            for generic_msg in messages {
                if let Some(message) = self.convert_generic_message(&generic_msg)? {
                    converted_messages.push(message);
                }
            }

            let session = ChatSession {
                id: session_id,
                title: export.title.unwrap_or_else(|| "Imported Conversation".to_string()),
                created_at: export.created_at.as_ref()
                    .and_then(|ts| self.parse_timestamp(ts).ok())
                    .unwrap_or(now),
                updated_at: export.updated_at.as_ref()
                    .and_then(|ts| self.parse_timestamp(ts).ok())
                    .unwrap_or(now),
                messages: converted_messages,
                model: export.model.unwrap_or_else(|| "openai-gpt3.5".to_string()),
                system_prompt: None,
                model_config: ModelConfig::default(),
                total_tokens_used: TokenUsage::default(),
                tags: Vec::new(),
                is_archived: false,
                export_count: 0,
                message_count: 0,
                last_activity: now,
            };

            Ok(vec![session])
        } else {
            Err(RuffError::ImportFailed { 
                reason: "No valid session or message data found in JSON".to_string() 
            })
        }
    }

    /// Convert generic message to Ruff message format
    fn convert_generic_message(&self, msg: &GenericMessage) -> Result<Option<Message>, RuffError> {
        if msg.content.trim().is_empty() {
            return Ok(None);
        }

        let role = match msg.role.to_lowercase().as_str() {
            "user" | "human" => MessageRole::User,
            "assistant" | "ai" | "bot" => MessageRole::Assistant,
            "system" => MessageRole::System,
            _ => return Ok(None), // Skip unknown roles
        };

        let timestamp = msg.timestamp.as_ref()
            .and_then(|ts| self.parse_timestamp(ts).ok())
            .unwrap_or_else(Local::now);

        Ok(Some(Message {
            id: Uuid::new_v4(),
            role,
            content: msg.content.clone(),
            timestamp,
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "unknown".to_string(),
                temperature: 0.7,
                response_time_ms: 0,
                is_regenerated: false,
                regeneration_count: 0,
            },
        }))
    }

    /// Parse timestamp from various string formats
    pub fn parse_timestamp(&self, timestamp_str: &str) -> Result<DateTime<Local>, RuffError> {
        // Try different timestamp formats
        let formats = [
            "%Y-%m-%dT%H:%M:%S%.fZ",      // ISO 8601 with microseconds
            "%Y-%m-%dT%H:%M:%SZ",         // ISO 8601 basic
            "%Y-%m-%dT%H:%M:%S%.f%z",     // ISO 8601 with timezone
            "%Y-%m-%dT%H:%M:%S%z",        // ISO 8601 with timezone, no microseconds
            "%Y-%m-%d %H:%M:%S",          // Simple format
            "%Y-%m-%d %H:%M:%S%.f",       // Simple format with microseconds
        ];

        for format in &formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(timestamp_str, format) {
                return Ok(Local.from_local_datetime(&dt).single().unwrap_or_else(Local::now));
            }
            
            if let Ok(dt) = DateTime::parse_from_str(timestamp_str, format) {
                return Ok(dt.with_timezone(&Local));
            }
        }

        // Try parsing as Unix timestamp
        if let Ok(timestamp) = timestamp_str.parse::<f64>() {
            if let Some(dt) = DateTime::from_timestamp(timestamp as i64, (timestamp.fract() * 1_000_000_000.0) as u32) {
                return Ok(dt.with_timezone(&Local));
            }
        }

        Err(RuffError::ImportFailed { 
            reason: format!("Unable to parse timestamp: {}", timestamp_str) 
        })
    }

    /// Process and validate a session before importing
    async fn process_session(&mut self, session: &mut ChatSession, options: &ImportOptions) -> Result<SessionId, RuffError> {
        // Generate new session ID
        session.id = Uuid::new_v4();
        
        // Auto-generate title if requested and title is generic
        if options.auto_generate_titles && (session.title == "New Session" || session.title == "Imported Conversation" || session.title.trim().is_empty()) {
            if let Some(first_user_msg) = session.messages.iter().find(|msg| matches!(msg.role, MessageRole::User)) {
                session.title = self.generate_title_from_content(&first_user_msg.content);
            }
        }

        // Process messages
        let mut processed_messages = Vec::new();
        for mut message in session.messages.drain(..) {
            // Generate new message ID
            message.id = Uuid::new_v4();
            
            // Validate message length
            if let Some(max_length) = options.max_message_length {
                if message.content.len() > max_length {
                    if options.skip_invalid_messages {
                        continue; // Skip overly long messages
                    } else {
                        message.content.truncate(max_length);
                    }
                }
            }

            // Set default model if not specified
            if message.metadata.model_used == "unknown" {
                message.metadata.model_used = options.default_model.clone();
            }

            // Preserve or update timestamps
            if !options.preserve_timestamps {
                message.timestamp = Local::now();
            }

            processed_messages.push(message);
        }

        session.messages = processed_messages;
        session.message_count = session.messages.len() as u32;

        // Update session metadata
        if session.model == "unknown" {
            session.model = options.default_model.clone();
        }

        session.last_activity = Local::now();
        session.updated_at = Local::now();

        Ok(session.id)
    }

    /// Generate a title from message content
    pub fn generate_title_from_content(&self, content: &str) -> String {
        let content = content.trim();
        
        if content.is_empty() {
            return "Imported Conversation".to_string();
        }
        
        // Take the first sentence or first 50 characters, whichever is shorter
        let first_sentence = content
            .split(&['.', '!', '?', '\n'][..])
            .next()
            .unwrap_or(content)
            .trim();
        
        let title = if first_sentence.len() > 50 {
            format!("{}...", &first_sentence[..47])
        } else {
            first_sentence.to_string()
        };
        
        // Clean up the title
        title
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ".,!?-_()[]{}".contains(*c))
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// Validate imported data integrity
    pub fn validate_import_data(&self, sessions: &[ChatSession]) -> ValidationResult {
        self.validator.validate_sessions(sessions)
    }

    /// Get supported import formats
    pub fn get_supported_formats(&self) -> Vec<ImportFormat> {
        vec![
            ImportFormat::Json,
            ImportFormat::ChatGptExport,
            ImportFormat::ClaudeExport,
        ]
    }
}

impl Default for ImportService {
    fn default() -> Self {
        Self::new(EventBus::new())
    }
}

impl ImportService {
    /// Create a new import service with default configuration
    pub async fn new_default() -> Result<Self, RuffError> {
        let event_bus = EventBus::new();
        let validator = DataValidator::new();
        
        Ok(Self {
            event_bus,
            validator,
        })
    }

    /// Preview import from file without actually importing
    pub async fn preview_import_cli(&self, _input_path: &Path, _format: Option<ImportFormat>) -> Result<ImportPreviewResult, RuffError> {
        // This would parse the file and return preview information
        // For now, return a placeholder result
        Ok(ImportPreviewResult {
            session_count: 1,
            message_count: 10,
            conflicts: vec![],
        })
    }

    /// Import sessions from file
    pub async fn import_sessions_cli(&self, _input_path: &Path, _format: Option<ImportFormat>, _merge: bool) -> Result<ImportSessionsResult, RuffError> {
        // This would parse the file and import sessions
        // For now, return a placeholder result
        Ok(ImportSessionsResult {
            imported_count: 1,
            skipped_count: 0,
        })
    }
}

/// Result of import preview operation for CLI
#[derive(Debug, Clone)]
pub struct ImportPreviewResult {
    pub session_count: usize,
    pub message_count: usize,
    pub conflicts: Vec<String>,
}

/// Result of import sessions operation for CLI
#[derive(Debug, Clone)]
pub struct ImportSessionsResult {
    pub imported_count: usize,
    pub skipped_count: usize,
}