//! Session manager implementation
//!
//! Provides the core SessionManager struct for handling multiple chat sessions

use crate::events::{AppEvent, EventBus, MessageId, SessionId};
use crate::models::TokenUsage;
use crate::session::metadata::{SessionMetadata, SessionStatistics, SessionTab};
use crate::session::search::{
    SessionSearchFilters, SessionSearchIndex, SessionSearchQuery, SessionSearchResult,
};
use crate::session::system_prompt::SystemPromptManager;
use crate::EnhancedError;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

/// Session manager for handling multiple chat sessions
pub struct SessionManager {
    sessions: HashMap<SessionId, ChatSession>,
    active_session: Option<SessionId>,
    event_bus: EventBus,
    storage_path: PathBuf,
    search_index: SessionSearchIndex,
    system_prompt_manager: SystemPromptManager,
}

/// Enhanced chat session structure
/// Note: Messages are stored separately in MessageManager for single source of truth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: SessionId,
    pub title: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    // Messages removed - use MessageManager as single source of truth
    pub model: String,
    pub system_prompt: Option<String>,
    pub model_config: SessionModelConfig,
    pub total_tokens_used: TokenUsage,
    pub tags: Vec<String>,
    pub is_archived: bool,
    pub export_count: u32,
    pub message_count: u32,
    pub last_activity: DateTime<Local>,
}

/// Temporary struct for importing sessions with embedded messages
/// Used only during import/export operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSessionWithMessages {
    pub id: SessionId,
    pub title: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub messages: Vec<Message>, // Included for import/export
    pub model: String,
    pub system_prompt: Option<String>,
    pub model_config: SessionModelConfig,
    pub total_tokens_used: TokenUsage,
    pub tags: Vec<String>,
    pub is_archived: bool,
    pub export_count: u32,
    pub message_count: u32,
    pub last_activity: DateTime<Local>,
}

impl ChatSessionWithMessages {
    /// Convert to ChatSession (without messages) and extract messages separately
    pub fn split(self) -> (ChatSession, Vec<Message>) {
        let session = ChatSession {
            id: self.id,
            title: self.title,
            created_at: self.created_at,
            updated_at: self.updated_at,
            model: self.model,
            system_prompt: self.system_prompt,
            model_config: self.model_config,
            total_tokens_used: self.total_tokens_used,
            tags: self.tags,
            is_archived: self.is_archived,
            export_count: self.export_count,
            message_count: self.message_count,
            last_activity: self.last_activity,
        };
        (session, self.messages)
    }
}

/// Message structure for session storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Local>,
    pub edited_at: Option<DateTime<Local>>,
    pub token_usage: Option<TokenUsage>,
    pub parent_id: Option<MessageId>,
    pub children: Vec<MessageId>,
    pub metadata: MessageMetadata,
}

/// Message role enumeration with case-insensitive deserialization support
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MessageRole {
    #[serde(rename = "user", alias = "User", alias = "USER")]
    User,

    #[serde(
        rename = "assistant",
        alias = "Assistant",
        alias = "ASSISTANT",
        alias = "ai"
    )]
    Assistant,

    #[serde(rename = "system", alias = "System", alias = "SYSTEM")]
    System,
}

impl MessageRole {
    /// Convert from string, case-insensitive
    pub fn from_str_case_insensitive(s: &str) -> Result<Self, crate::EnhancedError> {
        match s.to_lowercase().as_str() {
            "user" => Ok(MessageRole::User),
            "assistant" | "ai" => Ok(MessageRole::Assistant),
            "system" => Ok(MessageRole::System),
            _ => Err(crate::EnhancedError::parsing(format!(
                "Invalid role: {}",
                s
            ))),
        }
    }

    /// Convert to lowercase string (for serialization)
    pub fn to_lowercase_string(&self) -> String {
        match self {
            MessageRole::User => "user".to_string(),
            MessageRole::Assistant => "assistant".to_string(),
            MessageRole::System => "system".to_string(),
        }
    }
}

/// Message metadata for tracking additional information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub model_used: String,
    pub temperature: f32,
    pub response_time_ms: u64,
    pub is_regenerated: bool,
    pub regeneration_count: u32,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            model_used: String::new(),
            temperature: 0.0,
            response_time_ms: 0,
            is_regenerated: false,
            regeneration_count: 0,
        }
    }
}

impl Message {
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content: content.into(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata::default(),
        }
    }
}

/// Model configuration for sessions (parameters used during chat)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionModelConfig {
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub custom_endpoint: Option<String>,
}

impl Default for SessionModelConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 4096,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            custom_endpoint: None,
        }
    }
}

impl Default for TokenUsage {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
        }
    }
}

impl SessionManager {
    /// Create a new session manager with storage path
    pub fn new(event_bus: EventBus, storage_path: PathBuf) -> Self {
        Self {
            sessions: HashMap::new(),
            active_session: None,
            event_bus,
            storage_path,
            search_index: SessionSearchIndex::new(),
            system_prompt_manager: SystemPromptManager::default(),
        }
    }

    /// Create a new session manager with default configuration
    pub async fn new_default() -> Result<Self, EnhancedError> {
        let storage_path = dirs::data_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .join("ruff")
            .join("sessions");

        let event_bus = EventBus::new();
        let mut manager = Self::new(event_bus, storage_path);
        manager.initialize().await?;
        Ok(manager)
    }

    /// Initialize the session manager and load existing sessions
    pub async fn initialize(&mut self) -> Result<(), EnhancedError> {
        // Ensure storage directory exists
        fs::create_dir_all(&self.storage_path).await.map_err(|e| {
            EnhancedError::storage(format!("Failed to create storage directory: {}", e))
        })?;

        // Load existing sessions
        self.load_sessions().await?;

        // Index all loaded sessions for search
        self.rebuild_search_index();

        Ok(())
    }

    /// Create a new session
    pub async fn create_session(
        &mut self,
        title: Option<String>,
    ) -> Result<SessionId, EnhancedError> {
        let session_id = Uuid::new_v4();
        let now = Local::now();

        let session = ChatSession {
            id: session_id,
            title: title.unwrap_or_else(|| "New Session".to_string()),
            created_at: now,
            updated_at: now,
            model: "openai-gpt3.5".to_string(), // Default model
            system_prompt: None,
            model_config: SessionModelConfig::default(),
            total_tokens_used: TokenUsage::default(),
            tags: Vec::new(),
            is_archived: false,
            export_count: 0,
            message_count: 0,
            last_activity: now,
        };

        self.sessions.insert(session_id, session.clone());

        // Index the session for search (without messages - messages indexed separately by MessageManager)
        self.search_index.index_session(&session, &[]);

        // Persist the session
        self.save_session(session_id).await?;

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionCreated(session_id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(session_id)
    }

    /// Switch to a different session
    pub async fn switch_session(&mut self, id: SessionId) -> Result<(), EnhancedError> {
        if !self.sessions.contains_key(&id) {
            return Err(
                EnhancedError::session(format!("Session not found: {}", id)).with_session(id)
            );
        }

        // Save current session state before switching
        if let Some(current_id) = self.active_session {
            self.save_session(current_id).await?;
        }

        // Update last activity for the session we're switching to
        if let Some(session) = self.sessions.get_mut(&id) {
            session.last_activity = Local::now();
            session.updated_at = Local::now();
        }

        self.active_session = Some(id);

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionSwitched(id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(())
    }

    /// Get the currently active session
    pub fn get_active_session(&self) -> Option<&ChatSession> {
        self.active_session.and_then(|id| self.sessions.get(&id))
    }

    /// Get the currently active session mutably
    pub fn get_active_session_mut(&mut self) -> Option<&mut ChatSession> {
        self.active_session
            .and_then(|id| self.sessions.get_mut(&id))
    }

    /// Get a session by ID
    pub fn get_session(&self, id: SessionId) -> Option<&ChatSession> {
        self.sessions.get(&id)
    }

    /// Get a session by ID mutably
    pub fn get_session_mut(&mut self, id: SessionId) -> Option<&mut ChatSession> {
        self.sessions.get_mut(&id)
    }

    /// Get all sessions
    pub fn get_all_sessions(&self) -> Vec<&ChatSession> {
        self.sessions.values().collect()
    }

    /// Get all session IDs
    pub fn get_session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().cloned().collect()
    }

    /// Delete a session
    pub async fn delete_session(&mut self, id: SessionId) -> Result<(), EnhancedError> {
        if !self.sessions.contains_key(&id) {
            return Err(
                EnhancedError::session(format!("Session not found: {}", id)).with_session(id)
            );
        }

        // If this is the active session, clear the active session
        if self.active_session == Some(id) {
            self.active_session = None;
        }

        // Remove from memory
        self.sessions.remove(&id);

        // Remove from search index
        self.search_index.remove_session(id);

        // Remove from storage
        let session_file = self.get_session_file_path(id);
        if session_file.exists() {
            fs::remove_file(session_file).await.map_err(|e| {
                EnhancedError::storage(format!("Failed to delete session file: {}", e))
            })?;
        }

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionDeleted(id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(())
    }

    /// Save a specific session to storage
    pub async fn save_session(&self, id: SessionId) -> Result<(), EnhancedError> {
        let session = self.sessions.get(&id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", id)).with_session(id)
        })?;

        // Ensure storage directory exists
        fs::create_dir_all(&self.storage_path).await.map_err(|e| {
            EnhancedError::storage(format!("Failed to create storage directory: {}", e))
        })?;

        let session_file = self.get_session_file_path(id);
        let session_data =
            serde_json::to_string_pretty(session).map_err(|e| EnhancedError::from(e))?;

        fs::write(session_file, session_data)
            .await
            .map_err(|e| EnhancedError::storage(format!("Failed to save session: {}", e)))?;

        Ok(())
    }

    /// Save all sessions to storage
    pub async fn save_all_sessions(&self) -> Result<(), EnhancedError> {
        for &id in self.sessions.keys() {
            self.save_session(id).await?;
        }
        Ok(())
    }

    /// Load all sessions from storage
    async fn load_sessions(&mut self) -> Result<(), EnhancedError> {
        if !self.storage_path.exists() {
            return Ok(()); // No sessions to load
        }

        let mut entries = fs::read_dir(&self.storage_path).await.map_err(|e| {
            EnhancedError::storage(format!("Failed to read sessions directory: {}", e))
        })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| EnhancedError::storage(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_session_from_file(&path).await {
                    Ok(session) => {
                        self.sessions.insert(session.id, session);
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to load session from {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single session from file
    async fn load_session_from_file(
        &self,
        path: &std::path::Path,
    ) -> Result<ChatSession, EnhancedError> {
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| EnhancedError::storage(format!("Failed to read session file: {}", e)))?;

        let session: ChatSession =
            serde_json::from_str(&content).map_err(|e| EnhancedError::from(e))?;

        Ok(session)
    }

    /// Get the file path for a session
    fn get_session_file_path(&self, id: SessionId) -> PathBuf {
        self.storage_path.join(format!("{}.json", id))
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Check if a session exists
    pub fn session_exists(&self, id: SessionId) -> bool {
        self.sessions.contains_key(&id)
    }

    /// Get the active session ID
    pub fn get_active_session_id(&self) -> Option<SessionId> {
        self.active_session
    }

    /// Search sessions with the given query
    pub fn search_sessions(&self, query: &SessionSearchQuery) -> Vec<SessionSearchResult> {
        self.search_index.search(query)
    }

    /// Filter sessions based on criteria
    pub fn filter_sessions(&self, filters: &SessionSearchFilters) -> Vec<SessionSearchResult> {
        self.search_index.filter_sessions(filters)
    }

    /// Get sessions by tags
    pub fn get_sessions_by_tags(&self, tags: &[String]) -> Vec<SessionSearchResult> {
        self.search_index.get_sessions_by_tags(tags)
    }

    /// Add a tag to a session
    pub async fn add_tag_to_session(
        &mut self,
        session_id: SessionId,
        tag: String,
    ) -> Result<(), EnhancedError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        if !session.tags.contains(&tag) {
            session.tags.push(tag);
            session.updated_at = Local::now();

            // Update search index
            self.search_index.index_session(session, &[]);

            // Save session
            self.save_session(session_id).await?;
        }

        Ok(())
    }

    /// Remove a tag from a session
    pub async fn remove_tag_from_session(
        &mut self,
        session_id: SessionId,
        tag: &str,
    ) -> Result<(), EnhancedError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        if let Some(pos) = session.tags.iter().position(|t| t == tag) {
            session.tags.remove(pos);
            session.updated_at = Local::now();

            // Update search index
            self.search_index.index_session(session, &[]);

            // Save session
            self.save_session(session_id).await?;
        }

        Ok(())
    }

    /// Update session in search index (call this when session content changes)
    pub fn update_session_index(&mut self, session_id: SessionId) {
        if let Some(session) = self.sessions.get(&session_id) {
            self.search_index.index_session(session, &[]);
        }
    }

    /// Rebuild the entire search index
    pub fn rebuild_search_index(&mut self) {
        self.search_index.clear();
        for session in self.sessions.values() {
            self.search_index.index_session(session, &[]);
        }
    }

    /// Get all unique tags across all sessions
    pub fn get_all_tags(&self) -> Vec<String> {
        let mut all_tags = std::collections::HashSet::new();
        for session in self.sessions.values() {
            for tag in &session.tags {
                all_tags.insert(tag.clone());
            }
        }
        let mut tags: Vec<String> = all_tags.into_iter().collect();
        tags.sort();
        tags
    }

    /// Get all unique models across all sessions
    pub fn get_all_models(&self) -> Vec<String> {
        let mut all_models = std::collections::HashSet::new();
        for session in self.sessions.values() {
            all_models.insert(session.model.clone());
        }
        let mut models: Vec<String> = all_models.into_iter().collect();
        models.sort();
        models
    }

    /// Rename a session with validation
    pub async fn rename_session(
        &mut self,
        session_id: SessionId,
        new_title: String,
    ) -> Result<(), EnhancedError> {
        // Validate the new title
        let trimmed_title = new_title.trim();
        if trimmed_title.is_empty() {
            return Err(EnhancedError::config("Session title cannot be empty"));
        }

        if trimmed_title.len() > 200 {
            return Err(EnhancedError::config(
                "Session title cannot exceed 200 characters",
            ));
        }

        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        let old_title = session.title.clone();
        session.title = trimmed_title.to_string();
        session.updated_at = Local::now();

        // Update search index
        self.search_index.index_session(session, &[]);

        // Save session
        self.save_session(session_id).await?;

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionRenamed {
                id: session_id,
                old_title,
                new_title: trimmed_title.to_string(),
            })
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(())
    }

    /// Generate an auto-title from the first user message
    /// Requires MessageManager to access messages
    pub async fn auto_generate_title(
        &mut self,
        session_id: SessionId,
        message_manager: &crate::message::manager::MessageManager,
    ) -> Result<String, EnhancedError> {
        let session = self.sessions.get(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        // Get messages from MessageManager and find first user message
        let messages = message_manager.get_session_messages(session_id);
        let first_user_message = messages
            .iter()
            .find(|msg| matches!(msg.role, MessageRole::User));

        let auto_title = if let Some(message) = first_user_message {
            self.generate_title_from_content(&message.content)
        } else {
            format!("Session {}", session.created_at.format("%Y-%m-%d %H:%M"))
        };

        // Apply the auto-generated title
        self.rename_session(session_id, auto_title.clone()).await?;

        Ok(auto_title)
    }

    /// Generate a title from message content
    fn generate_title_from_content(&self, content: &str) -> String {
        let content = content.trim();

        // If content is empty, use a default title
        if content.is_empty() {
            return "New Conversation".to_string();
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

    /// Sync session messages - NO LONGER NEEDED
    /// Messages are now stored only in MessageManager (single source of truth)
    /// This method is deprecated and will be removed
    #[deprecated(note = "Messages are now stored only in MessageManager")]
    pub fn sync_session_messages(
        &mut self,
        _session_id: SessionId,
        _messages: Vec<Message>,
    ) -> Result<(), EnhancedError> {
        // No-op: Messages are stored in MessageManager only
        Ok(())
    }

    /// Update session metadata (message count, token usage, etc.)
    /// Requires MessageManager to access current messages
    pub async fn update_session_metadata(
        &mut self,
        session_id: SessionId,
        message_manager: &crate::message::manager::MessageManager,
    ) -> Result<(), EnhancedError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        // Get messages from MessageManager (single source of truth)
        let messages = message_manager.get_session_messages(session_id);

        // Update message count
        session.message_count = messages.len() as u32;

        // Update total token usage
        let mut total_input_tokens = 0;
        let mut total_output_tokens = 0;

        for message in &messages {
            if let Some(token_usage) = &message.token_usage {
                total_input_tokens += token_usage.input_tokens;
                total_output_tokens += token_usage.output_tokens;
            }
        }

        session.total_tokens_used = TokenUsage {
            input_tokens: total_input_tokens,
            output_tokens: total_output_tokens,
            total_tokens: total_input_tokens + total_output_tokens,
        };

        // Update last activity
        session.last_activity = Local::now();
        session.updated_at = Local::now();

        // Update search index
        self.search_index.index_session(session, &[]);

        // Save session
        self.save_session(session_id).await?;

        Ok(())
    }

    /// Get session metadata summary
    pub fn get_session_metadata(&self, session_id: SessionId) -> Option<SessionMetadata> {
        let session = self.sessions.get(&session_id)?;

        Some(SessionMetadata {
            id: session.id,
            title: session.title.clone(),
            created_at: session.created_at,
            updated_at: session.updated_at,
            message_count: session.message_count as usize,
            total_tokens: session.total_tokens_used.total_tokens,
        })
    }

    /// Get session tabs for UI display
    pub fn get_session_tabs(&self) -> Vec<SessionTab> {
        let mut tabs: Vec<SessionTab> = self
            .sessions
            .values()
            .filter(|session| !session.is_archived)
            .map(|session| SessionTab {
                id: session.id,
                title: session.title.clone(),
                is_active: self.active_session == Some(session.id),
                has_unsaved_changes: false, // This would be determined by the UI layer
                last_activity: session.last_activity,
            })
            .collect();

        // Sort by last activity (most recent first)
        tabs.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        tabs
    }

    /// Archive a session (soft delete)
    pub async fn archive_session(&mut self, session_id: SessionId) -> Result<(), EnhancedError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        session.is_archived = true;
        session.updated_at = Local::now();

        // If this is the active session, clear the active session
        if self.active_session == Some(session_id) {
            self.active_session = None;
        }

        // Update search index
        self.search_index.index_session(session, &[]);

        // Save session
        self.save_session(session_id).await?;

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionArchived(session_id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(())
    }

    /// Restore an archived session
    pub async fn restore_session(&mut self, session_id: SessionId) -> Result<(), EnhancedError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        session.is_archived = false;
        session.updated_at = Local::now();
        session.last_activity = Local::now();

        // Update search index
        self.search_index.index_session(session, &[]);

        // Save session
        self.save_session(session_id).await?;

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionRestored(session_id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(())
    }

    /// Get archived sessions
    pub fn get_archived_sessions(&self) -> Vec<&ChatSession> {
        self.sessions
            .values()
            .filter(|session| session.is_archived)
            .collect()
    }

    /// Get active (non-archived) sessions
    pub fn get_active_sessions(&self) -> Vec<&ChatSession> {
        self.sessions
            .values()
            .filter(|session| !session.is_archived)
            .collect()
    }

    /// Add a pre-existing session (used for imports/restores)
    pub async fn add_session(&mut self, session: ChatSession) -> Result<(), EnhancedError> {
        let session_id = session.id;

        // Check if session already exists
        if self.sessions.contains_key(&session_id) {
            return Err(
                EnhancedError::session(format!("Session already exists: {}", session_id))
                    .with_session(session_id),
            );
        }

        // Add to memory
        self.sessions.insert(session_id, session.clone());

        // Index the session for search (without messages - messages indexed separately by MessageManager)
        self.search_index.index_session(&session, &[]);

        // Persist the session
        self.save_session(session_id).await?;

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionCreated(session_id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(())
    }

    /// Update system prompt for a session
    pub async fn update_session_system_prompt(
        &mut self,
        session_id: SessionId,
        system_prompt: Option<String>,
    ) -> Result<(), EnhancedError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        // Validate system prompt if provided
        if let Some(ref prompt) = system_prompt {
            self.system_prompt_manager
                .validate_prompt(prompt)
                .map_err(|errors| {
                    EnhancedError::config(format!("Invalid system prompt: {}", errors.join("; ")))
                })?;
        }

        session.system_prompt = system_prompt;
        session.updated_at = Local::now();

        // Update search index
        self.search_index.index_session(session, &[]);

        // Save session
        self.save_session(session_id).await?;

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionUpdated(session_id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(())
    }

    /// Apply system prompt template to a session
    pub async fn apply_system_prompt_template(
        &mut self,
        session_id: SessionId,
        template_id: uuid::Uuid,
        variables: &std::collections::HashMap<String, String>,
    ) -> Result<(), EnhancedError> {
        let applied_prompt = self
            .system_prompt_manager
            .apply_template(template_id, variables)?;
        self.update_session_system_prompt(session_id, Some(applied_prompt))
            .await
    }

    /// Get system prompt manager
    pub fn get_system_prompt_manager(&self) -> &SystemPromptManager {
        &self.system_prompt_manager
    }

    /// Get system prompt manager mutably
    pub fn get_system_prompt_manager_mut(&mut self) -> &mut SystemPromptManager {
        &mut self.system_prompt_manager
    }

    /// Update model configuration for a session
    pub async fn update_session_model_config(
        &mut self,
        session_id: SessionId,
        model_config: SessionModelConfig,
    ) -> Result<(), EnhancedError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            EnhancedError::session(format!("Session not found: {}", session_id))
                .with_session(session_id)
        })?;

        session.model_config = model_config;
        session.updated_at = Local::now();

        // Update search index
        self.search_index.index_session(session, &[]);

        // Save session
        self.save_session(session_id).await?;

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionUpdated(session_id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(())
    }

    /// Get session statistics
    pub fn get_session_statistics(&self) -> SessionStatistics {
        let total_sessions = self.sessions.len();
        let active_sessions = self.sessions.values().filter(|s| !s.is_archived).count();
        let archived_sessions = total_sessions - active_sessions;

        let total_messages: u32 = self.sessions.values().map(|s| s.message_count).sum();
        let total_tokens: u32 = self
            .sessions
            .values()
            .map(|s| s.total_tokens_used.total_tokens)
            .sum();

        let oldest_session = self
            .sessions
            .values()
            .min_by_key(|s| s.created_at)
            .map(|s| s.created_at);

        let most_recent_activity = self
            .sessions
            .values()
            .max_by_key(|s| s.last_activity)
            .map(|s| s.last_activity);

        SessionStatistics {
            total_sessions,
            active_sessions,
            archived_sessions,
            total_messages,
            total_tokens,
            oldest_session,
            most_recent_activity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::search::SessionSortBy;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration};

    async fn create_test_session_manager() -> (SessionManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("sessions");
        let event_bus = EventBus::new();
        let mut manager = SessionManager::new(event_bus, storage_path);
        manager.initialize().await.unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_session_manager_creation() {
        let (manager, _temp_dir) = create_test_session_manager().await;
        assert_eq!(manager.session_count(), 0);
        assert!(manager.get_active_session().is_none());
    }

    #[tokio::test]
    async fn test_create_session() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Test Session".to_string()))
            .await
            .unwrap();

        assert_eq!(manager.session_count(), 1);
        assert!(manager.session_exists(session_id));

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.title, "Test Session");
        assert_eq!(session.id, session_id);
        assert_eq!(session.message_count, 0);
        assert_eq!(session.model, "openai-gpt3.5");
        assert!(!session.is_archived);
    }

    #[tokio::test]
    async fn test_create_session_with_default_title() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager.create_session(None).await.unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.title, "New Session");
    }

    #[tokio::test]
    async fn test_switch_session() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Session 1".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Session 2".to_string()))
            .await
            .unwrap();

        // Switch to session 1
        manager.switch_session(session1_id).await.unwrap();
        assert_eq!(manager.get_active_session_id(), Some(session1_id));
        assert_eq!(manager.get_active_session().unwrap().title, "Session 1");

        // Switch to session 2
        manager.switch_session(session2_id).await.unwrap();
        assert_eq!(manager.get_active_session_id(), Some(session2_id));
        assert_eq!(manager.get_active_session().unwrap().title, "Session 2");
    }

    #[tokio::test]
    async fn test_switch_to_nonexistent_session() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let nonexistent_id = Uuid::new_v4();
        let result = manager.switch_session(nonexistent_id).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_get_active_session() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        // No active session initially
        assert!(manager.get_active_session().is_none());

        let session_id = manager
            .create_session(Some("Active Session".to_string()))
            .await
            .unwrap();
        manager.switch_session(session_id).await.unwrap();

        let active_session = manager.get_active_session().unwrap();
        assert_eq!(active_session.title, "Active Session");
        assert_eq!(active_session.id, session_id);
    }

    #[tokio::test]
    async fn test_delete_session() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("To Delete".to_string()))
            .await
            .unwrap();
        manager.switch_session(session_id).await.unwrap();

        assert_eq!(manager.session_count(), 1);
        assert_eq!(manager.get_active_session_id(), Some(session_id));

        manager.delete_session(session_id).await.unwrap();

        assert_eq!(manager.session_count(), 0);
        assert!(manager.get_active_session().is_none());
        assert!(!manager.session_exists(session_id));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_session() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let nonexistent_id = Uuid::new_v4();
        let result = manager.delete_session(nonexistent_id).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_session_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("sessions");

        let session_id = {
            let event_bus = EventBus::new();
            let mut manager = SessionManager::new(event_bus, storage_path.clone());
            manager.initialize().await.unwrap();

            let id = manager
                .create_session(Some("Persistent Session".to_string()))
                .await
                .unwrap();
            manager.save_session(id).await.unwrap();
            id
        };

        // Create a new manager and load sessions
        let event_bus = EventBus::new();
        let mut manager = SessionManager::new(event_bus, storage_path);
        manager.initialize().await.unwrap();

        assert_eq!(manager.session_count(), 1);
        assert!(manager.session_exists(session_id));

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.title, "Persistent Session");
    }

    #[tokio::test]
    async fn test_get_all_sessions() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Session 1".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Session 2".to_string()))
            .await
            .unwrap();
        let session3_id = manager
            .create_session(Some("Session 3".to_string()))
            .await
            .unwrap();

        let all_sessions = manager.get_all_sessions();
        assert_eq!(all_sessions.len(), 3);

        let session_ids: Vec<SessionId> = all_sessions.iter().map(|s| s.id).collect();
        assert!(session_ids.contains(&session1_id));
        assert!(session_ids.contains(&session2_id));
        assert!(session_ids.contains(&session3_id));
    }

    #[tokio::test]
    async fn test_get_session_ids() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Session 1".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Session 2".to_string()))
            .await
            .unwrap();

        let session_ids = manager.get_session_ids();
        assert_eq!(session_ids.len(), 2);
        assert!(session_ids.contains(&session1_id));
        assert!(session_ids.contains(&session2_id));
    }

    #[tokio::test]
    async fn test_session_last_activity_update() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Activity Test".to_string()))
            .await
            .unwrap();
        let initial_activity = manager.get_session(session_id).unwrap().last_activity;

        // Wait a bit to ensure time difference
        sleep(Duration::from_millis(10)).await;

        manager.switch_session(session_id).await.unwrap();
        let updated_activity = manager.get_session(session_id).unwrap().last_activity;

        assert!(updated_activity > initial_activity);
    }

    #[tokio::test]
    async fn test_event_publishing() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = Arc::clone(&event_count);

        let _handle = manager
            .event_bus
            .subscribe(move |event| {
                match event {
                    AppEvent::SessionCreated(_)
                    | AppEvent::SessionSwitched(_)
                    | AppEvent::SessionDeleted(_) => {
                        event_count_clone.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => {}
                }
                Ok(())
            })
            .await;

        let session_id = manager
            .create_session(Some("Event Test".to_string()))
            .await
            .unwrap();
        manager.switch_session(session_id).await.unwrap();
        manager.delete_session(session_id).await.unwrap();

        // Give time for event processing
        sleep(Duration::from_millis(10)).await;

        assert_eq!(event_count.load(Ordering::SeqCst), 3); // Create, Switch, Delete
    }

    #[tokio::test]
    async fn test_save_all_sessions() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let _session1_id = manager
            .create_session(Some("Session 1".to_string()))
            .await
            .unwrap();
        let _session2_id = manager
            .create_session(Some("Session 2".to_string()))
            .await
            .unwrap();

        // This should not fail
        manager.save_all_sessions().await.unwrap();
    }

    #[tokio::test]
    async fn test_mutable_session_access() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Mutable Test".to_string()))
            .await
            .unwrap();
        manager.switch_session(session_id).await.unwrap();

        // Test mutable access to active session
        {
            let active_session = manager.get_active_session_mut().unwrap();
            active_session.title = "Modified Title".to_string();
        }

        assert_eq!(
            manager.get_active_session().unwrap().title,
            "Modified Title"
        );

        // Test mutable access by ID
        {
            let session = manager.get_session_mut(session_id).unwrap();
            session.model = "openai-gpt4".to_string();
        }

        assert_eq!(
            manager.get_session(session_id).unwrap().model,
            "openai-gpt4"
        );
    }

    #[tokio::test]
    async fn test_session_search_by_title() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let _session1_id = manager
            .create_session(Some("Machine Learning Discussion".to_string()))
            .await
            .unwrap();
        let _session2_id = manager
            .create_session(Some("Rust Programming Help".to_string()))
            .await
            .unwrap();
        let _session3_id = manager
            .create_session(Some("Python Data Analysis".to_string()))
            .await
            .unwrap();

        let query = SessionSearchQuery {
            text: "machine".to_string(),
            filters: SessionSearchFilters::default(),
            sort_by: SessionSortBy::Relevance,
            limit: 10,
        };

        let results = manager.search_sessions(&query);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Machine Learning Discussion");
    }

    #[tokio::test]
    async fn test_session_search_fuzzy_matching() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let _session_id = manager
            .create_session(Some("JavaScript Development".to_string()))
            .await
            .unwrap();

        let query = SessionSearchQuery {
            text: "javascrpt".to_string(), // Intentional typo
            filters: SessionSearchFilters::default(),
            sort_by: SessionSortBy::Relevance,
            limit: 10,
        };

        let results = manager.search_sessions(&query);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "JavaScript Development");
    }

    #[tokio::test]
    async fn test_session_tag_management() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Tagged Session".to_string()))
            .await
            .unwrap();

        // Add tags
        manager
            .add_tag_to_session(session_id, "programming".to_string())
            .await
            .unwrap();
        manager
            .add_tag_to_session(session_id, "rust".to_string())
            .await
            .unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.tags.len(), 2);
        assert!(session.tags.contains(&"programming".to_string()));
        assert!(session.tags.contains(&"rust".to_string()));

        // Remove a tag
        manager
            .remove_tag_from_session(session_id, "programming")
            .await
            .unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.tags.len(), 1);
        assert!(session.tags.contains(&"rust".to_string()));
        assert!(!session.tags.contains(&"programming".to_string()));
    }

    #[tokio::test]
    async fn test_session_search_by_tags() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Session 1".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Session 2".to_string()))
            .await
            .unwrap();
        let _session3_id = manager
            .create_session(Some("Session 3".to_string()))
            .await
            .unwrap();

        manager
            .add_tag_to_session(session1_id, "rust".to_string())
            .await
            .unwrap();
        manager
            .add_tag_to_session(session2_id, "python".to_string())
            .await
            .unwrap();

        let results = manager.get_sessions_by_tags(&["rust".to_string()]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, session1_id);
    }

    #[tokio::test]
    async fn test_session_filter_by_model() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let _session1_id = manager
            .create_session(Some("GPT Session".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Claude Session".to_string()))
            .await
            .unwrap();

        // Change model for session2
        {
            let session = manager.get_session_mut(session2_id).unwrap();
            session.model = "anthropic-claude3".to_string();
        }
        manager.update_session_index(session2_id);

        let filters = SessionSearchFilters {
            models: Some(vec!["anthropic-claude3".to_string()]),
            ..Default::default()
        };

        let results = manager.filter_sessions(&filters);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, session2_id);
    }

    #[tokio::test]
    async fn test_session_filter_by_date_range() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Old Session".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("New Session".to_string()))
            .await
            .unwrap();

        // Modify creation date for session1 to be older
        {
            let session = manager.get_session_mut(session1_id).unwrap();
            session.created_at = Local::now() - chrono::Duration::days(30);
        }
        manager.update_session_index(session1_id);

        let start_date = Local::now() - chrono::Duration::days(7);
        let end_date = Local::now() + chrono::Duration::days(1);

        let filters = SessionSearchFilters {
            date_range: Some((start_date, end_date)),
            ..Default::default()
        };

        let results = manager.filter_sessions(&filters);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, session2_id);
    }

    #[tokio::test]
    async fn test_get_all_tags_and_models() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Session 1".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Session 2".to_string()))
            .await
            .unwrap();

        manager
            .add_tag_to_session(session1_id, "rust".to_string())
            .await
            .unwrap();
        manager
            .add_tag_to_session(session1_id, "programming".to_string())
            .await
            .unwrap();
        manager
            .add_tag_to_session(session2_id, "python".to_string())
            .await
            .unwrap();

        // Change model for session2
        {
            let session = manager.get_session_mut(session2_id).unwrap();
            session.model = "anthropic-claude3".to_string();
        }

        let all_tags = manager.get_all_tags();
        assert_eq!(all_tags.len(), 3);
        assert!(all_tags.contains(&"rust".to_string()));
        assert!(all_tags.contains(&"programming".to_string()));
        assert!(all_tags.contains(&"python".to_string()));

        let all_models = manager.get_all_models();
        assert_eq!(all_models.len(), 2);
        assert!(all_models.contains(&"openai-gpt3.5".to_string()));
        assert!(all_models.contains(&"anthropic-claude3".to_string()));
    }

    #[tokio::test]
    async fn test_search_index_rebuild() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Test Session".to_string()))
            .await
            .unwrap();
        manager
            .add_tag_to_session(session_id, "test".to_string())
            .await
            .unwrap();

        // Verify search works
        let results = manager.get_sessions_by_tags(&["test".to_string()]);
        assert_eq!(results.len(), 1);

        // Rebuild index
        manager.rebuild_search_index();

        // Verify search still works after rebuild
        let results = manager.get_sessions_by_tags(&["test".to_string()]);
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_session_search_sorting() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Alpha Session".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Beta Session".to_string()))
            .await
            .unwrap();
        let session3_id = manager
            .create_session(Some("Gamma Session".to_string()))
            .await
            .unwrap();

        // Add different message counts
        {
            let session1 = manager.get_session_mut(session1_id).unwrap();
            session1.message_count = 10;
        }
        {
            let session2 = manager.get_session_mut(session2_id).unwrap();
            session2.message_count = 5;
        }
        {
            let session3 = manager.get_session_mut(session3_id).unwrap();
            session3.message_count = 15;
        }

        manager.rebuild_search_index();

        // Test sorting by title
        let query = SessionSearchQuery {
            text: "".to_string(),
            filters: SessionSearchFilters::default(),
            sort_by: SessionSortBy::Title,
            limit: 10,
        };

        let results = manager.search_sessions(&query);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].title, "Alpha Session");
        assert_eq!(results[1].title, "Beta Session");
        assert_eq!(results[2].title, "Gamma Session");

        // Test sorting by message count
        let query = SessionSearchQuery {
            text: "".to_string(),
            filters: SessionSearchFilters::default(),
            sort_by: SessionSortBy::MessageCount,
            limit: 10,
        };

        let results = manager.search_sessions(&query);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].message_count, 15); // Gamma
        assert_eq!(results[1].message_count, 10); // Alpha
        assert_eq!(results[2].message_count, 5); // Beta
    }

    #[tokio::test]
    async fn test_session_renaming() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Original Title".to_string()))
            .await
            .unwrap();

        // Test successful rename
        manager
            .rename_session(session_id, "New Title".to_string())
            .await
            .unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.title, "New Title");

        // Test rename with whitespace trimming
        manager
            .rename_session(session_id, "  Trimmed Title  ".to_string())
            .await
            .unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.title, "Trimmed Title");
    }

    #[tokio::test]
    async fn test_session_rename_validation() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Test Session".to_string()))
            .await
            .unwrap();

        // Test empty title
        let result = manager.rename_session(session_id, "".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));

        // Test whitespace-only title
        let result = manager.rename_session(session_id, "   ".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));

        // Test title too long
        let long_title = "a".repeat(201);
        let result = manager.rename_session(session_id, long_title).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot exceed 200 characters"));
    }

    #[tokio::test]
    async fn test_auto_title_generation() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;
        let mut message_manager = crate::message::manager::MessageManager::new(EventBus::new());

        let session_id = manager
            .create_session(Some("Test Session".to_string()))
            .await
            .unwrap();

        // Add a user message through MessageManager
        let message = Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "How do I implement a binary search tree in Rust?".to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };
        message_manager
            .add_message(session_id, message)
            .await
            .unwrap();

        let auto_title = manager
            .auto_generate_title(session_id, &message_manager)
            .await
            .unwrap();

        assert!(auto_title.contains("binary search tree"));
        assert_eq!(manager.get_session(session_id).unwrap().title, auto_title);
    }

    #[tokio::test]
    async fn test_auto_title_generation_long_content() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;
        let mut message_manager = crate::message::manager::MessageManager::new(EventBus::new());

        let session_id = manager
            .create_session(Some("Test Session".to_string()))
            .await
            .unwrap();

        // Add a user message with long content through MessageManager
        let message = Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "This is a very long message that should be truncated when generating an auto title because it exceeds the maximum length limit".to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };
        message_manager
            .add_message(session_id, message)
            .await
            .unwrap();

        let auto_title = manager
            .auto_generate_title(session_id, &message_manager)
            .await
            .unwrap();

        assert!(auto_title.len() <= 50);
        assert!(auto_title.ends_with("..."));
    }

    #[tokio::test]
    async fn test_session_metadata_update() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;
        let mut message_manager = crate::message::manager::MessageManager::new(EventBus::new());

        let session_id = manager
            .create_session(Some("Metadata Test".to_string()))
            .await
            .unwrap();

        // Add messages with token usage through MessageManager
        let message1 = Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "Test message 1".to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: Some(TokenUsage {
                input_tokens: 10,
                output_tokens: 0,
                total_tokens: 10,
            }),
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };
        message_manager
            .add_message(session_id, message1)
            .await
            .unwrap();

        let message2 = Message {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content: "Test response 1".to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: Some(TokenUsage {
                input_tokens: 0,
                output_tokens: 20,
                total_tokens: 20,
            }),
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test".to_string(),
                temperature: 0.7,
                response_time_ms: 200,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };
        message_manager
            .add_message(session_id, message2)
            .await
            .unwrap();

        manager
            .update_session_metadata(session_id, &message_manager)
            .await
            .unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert_eq!(session.message_count, 2);
        assert_eq!(session.total_tokens_used.input_tokens, 10);
        assert_eq!(session.total_tokens_used.output_tokens, 20);
        assert_eq!(session.total_tokens_used.total_tokens, 30);
    }

    #[tokio::test]
    async fn test_session_archiving() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Archive Test".to_string()))
            .await
            .unwrap();
        manager.switch_session(session_id).await.unwrap();

        assert!(!manager.get_session(session_id).unwrap().is_archived);
        assert_eq!(manager.get_active_session_id(), Some(session_id));

        // Archive the session
        manager.archive_session(session_id).await.unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert!(session.is_archived);
        assert!(manager.get_active_session_id().is_none()); // Active session should be cleared

        // Restore the session
        manager.restore_session(session_id).await.unwrap();

        let session = manager.get_session(session_id).unwrap();
        assert!(!session.is_archived);
    }

    #[tokio::test]
    async fn test_get_session_tabs() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Session 1".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Session 2".to_string()))
            .await
            .unwrap();
        let session3_id = manager
            .create_session(Some("Session 3".to_string()))
            .await
            .unwrap();

        // Archive one session
        manager.archive_session(session3_id).await.unwrap();

        // Set active session
        manager.switch_session(session1_id).await.unwrap();

        let tabs = manager.get_session_tabs();

        // Should only include non-archived sessions
        assert_eq!(tabs.len(), 2);

        // Find the active tab
        let active_tab = tabs.iter().find(|tab| tab.is_active).unwrap();
        assert_eq!(active_tab.id, session1_id);

        // Verify all tabs have the correct structure
        for tab in &tabs {
            assert!(!tab.title.is_empty());
            assert!(tab.id == session1_id || tab.id == session2_id);
        }
    }

    #[tokio::test]
    async fn test_session_statistics() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Session 1".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Session 2".to_string()))
            .await
            .unwrap();
        let session3_id = manager
            .create_session(Some("Session 3".to_string()))
            .await
            .unwrap();

        // Add some messages and update metadata
        {
            let session1 = manager.get_session_mut(session1_id).unwrap();
            session1.message_count = 5;
            session1.total_tokens_used = TokenUsage {
                input_tokens: 100,
                output_tokens: 150,
                total_tokens: 250,
            };
        }

        {
            let session2 = manager.get_session_mut(session2_id).unwrap();
            session2.message_count = 3;
            session2.total_tokens_used = TokenUsage {
                input_tokens: 50,
                output_tokens: 75,
                total_tokens: 125,
            };
        }

        // Archive one session
        manager.archive_session(session3_id).await.unwrap();

        let stats = manager.get_session_statistics();

        assert_eq!(stats.total_sessions, 3);
        assert_eq!(stats.active_sessions, 2);
        assert_eq!(stats.archived_sessions, 1);
        assert_eq!(stats.total_messages, 8); // 5 + 3 + 0
        assert_eq!(stats.total_tokens, 375); // 250 + 125 + 0
        assert!(stats.oldest_session.is_some());
        assert!(stats.most_recent_activity.is_some());
    }

    #[tokio::test]
    async fn test_get_archived_and_active_sessions() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session1_id = manager
            .create_session(Some("Active Session".to_string()))
            .await
            .unwrap();
        let session2_id = manager
            .create_session(Some("Archived Session".to_string()))
            .await
            .unwrap();

        manager.archive_session(session2_id).await.unwrap();

        let active_sessions = manager.get_active_sessions();
        let archived_sessions = manager.get_archived_sessions();

        assert_eq!(active_sessions.len(), 1);
        assert_eq!(archived_sessions.len(), 1);

        assert_eq!(active_sessions[0].id, session1_id);
        assert_eq!(archived_sessions[0].id, session2_id);
    }

    #[tokio::test]
    async fn test_get_session_metadata() {
        let (mut manager, _temp_dir) = create_test_session_manager().await;

        let session_id = manager
            .create_session(Some("Metadata Session".to_string()))
            .await
            .unwrap();

        let metadata = manager.get_session_metadata(session_id).unwrap();

        assert_eq!(metadata.id, session_id);
        assert_eq!(metadata.title, "Metadata Session");
        assert_eq!(metadata.message_count, 0);
        assert_eq!(metadata.total_tokens, 0);

        // Test with non-existent session
        let non_existent_id = Uuid::new_v4();
        assert!(manager.get_session_metadata(non_existent_id).is_none());
    }
}

impl SessionManager {
    /// List sessions with filtering and sorting for CLI
    pub async fn list_sessions(
        &self,
        filter: Option<&str>,
        archived: bool,
        sort: &str,
    ) -> Result<Vec<SessionListItem>, EnhancedError> {
        let mut sessions: Vec<SessionListItem> = self
            .sessions
            .values()
            .filter(|session| {
                // Filter by archived status
                if archived != session.is_archived {
                    return false;
                }

                // Filter by title pattern if provided
                if let Some(filter_pattern) = filter {
                    let pattern_lower = filter_pattern.to_lowercase();
                    return session.title.to_lowercase().contains(&pattern_lower)
                        || session
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&pattern_lower));
                }

                true
            })
            .map(|session| SessionListItem {
                id: session.id,
                title: session.title.clone(),
                message_count: session.message_count,
                updated_at: session.updated_at,
                created_at: session.created_at,
                is_archived: session.is_archived,
                tags: session.tags.clone(),
            })
            .collect();

        // Sort sessions
        match sort {
            "created" => sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
            "updated" => sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
            "title" => sessions.sort_by(|a, b| a.title.cmp(&b.title)),
            "messages" => sessions.sort_by(|a, b| b.message_count.cmp(&a.message_count)),
            _ => sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)), // Default to updated
        }

        Ok(sessions)
    }

    /// Create a session with CLI parameters
    pub async fn create_session_cli(
        &mut self,
        title: Option<String>,
        system_prompt: Option<String>,
        model: Option<String>,
    ) -> Result<SessionId, EnhancedError> {
        let session_id = Uuid::new_v4();
        let now = Local::now();

        let session = ChatSession {
            id: session_id,
            title: title.unwrap_or_else(|| "New Session".to_string()),
            created_at: now,
            updated_at: now,
            model: model.unwrap_or_else(|| "openai-gpt3.5".to_string()),
            system_prompt,
            model_config: SessionModelConfig::default(),
            total_tokens_used: TokenUsage::default(),
            tags: Vec::new(),
            is_archived: false,
            export_count: 0,
            message_count: 0,
            last_activity: now,
        };

        self.sessions.insert(session_id, session.clone());

        // Index the session for search (without messages - messages indexed separately by MessageManager)
        self.search_index.index_session(&session, &[]);

        // Persist the session
        self.save_session(session_id).await?;

        // Publish event
        self.event_bus
            .publish(AppEvent::SessionCreated(session_id))
            .await
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

        Ok(session_id)
    }

    /// Unarchive a session (alias for restore_session for CLI consistency)
    pub async fn unarchive_session(&mut self, session_id: SessionId) -> Result<(), EnhancedError> {
        self.restore_session(session_id).await
    }

    /// Search sessions for CLI with simplified parameters
    pub async fn search_sessions_cli(
        &self,
        query: &str,
        _content: bool,
        limit: usize,
    ) -> Result<Vec<SessionSearchResultItem>, EnhancedError> {
        let search_query = SessionSearchQuery {
            text: query.to_string(),
            filters: SessionSearchFilters {
                include_archived: false,
                tags: None,
                date_range: None,
                models: None,
                max_message_count: None,
                min_message_count: None,
            },
            sort_by: crate::session::search::SessionSortBy::Relevance,
            limit,
        };

        let results = self.search_index.search(&search_query);

        let items = results
            .into_iter()
            .map(|result| SessionSearchResultItem {
                session_id: result.session_id,
                title: result.title,
                snippet: if result.content_preview.is_empty() {
                    "No content preview available".to_string()
                } else {
                    result.content_preview
                },
                relevance_score: result.relevance_score,
            })
            .collect();

        Ok(items)
    }

    /// Get a session for CLI display
    pub async fn get_session_for_cli(
        &self,
        session_id: SessionId,
    ) -> Result<ChatSession, EnhancedError> {
        self.sessions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| EnhancedError::unknown(format!("Session {} not found", session_id)))
    }
}

/// Session list item for CLI display
#[derive(Debug, Clone)]
pub struct SessionListItem {
    pub id: SessionId,
    pub title: String,
    pub message_count: u32,
    pub updated_at: DateTime<Local>,
    pub created_at: DateTime<Local>,
    pub is_archived: bool,
    pub tags: Vec<String>,
}

/// Session search result item for CLI display
#[derive(Debug, Clone)]
pub struct SessionSearchResultItem {
    pub session_id: SessionId,
    pub title: String,
    pub snippet: String,
    pub relevance_score: f32,
}
