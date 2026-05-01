use crate::{
    events::SessionId,
    session::manager::{ChatSession, Message},
    EnhancedError,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Unified search query that can search sessions, messages, or both
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSearchQuery {
    /// Search text
    pub text: String,

    /// What to search
    pub scope: SearchScope,

    /// Optional filters
    pub filters: SearchFilters,

    /// Maximum number of results
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchScope {
    /// Search only in session metadata (titles, tags, etc.)
    Sessions,

    /// Search only in message content
    Messages,

    /// Search both sessions and messages
    Both,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    /// Filter by session IDs
    pub session_ids: Option<Vec<SessionId>>,

    /// Filter by date range
    pub date_range: Option<DateRange>,

    /// Filter by model
    pub model: Option<String>,

    /// Filter by tags
    pub tags: Option<Vec<String>>,

    /// Filter by message role (user, assistant, system)
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: chrono::DateTime<chrono::Utc>,
}

/// Unified search result
#[derive(Debug, Clone)]
pub enum SearchResult {
    Session(SessionSearchResult),
    Message(MessageSearchResult),
}

#[derive(Debug, Clone)]
pub struct SessionSearchResult {
    pub session: ChatSession,
    pub score: f32,
    pub matching_fields: Vec<String>, // Which fields matched (title, tags, etc.)
}

#[derive(Debug, Clone)]
pub struct MessageSearchResult {
    pub session_id: SessionId,
    pub message: Message,
    pub score: f32,
    pub snippet: String, // Highlighted snippet showing match context
}

/// Unified search backend trait
#[async_trait]
pub trait SearchBackend: Send + Sync {
    /// Index a session (metadata only)
    async fn index_session(&self, session: &ChatSession) -> Result<(), EnhancedError>;

    /// Index a message
    async fn index_message(
        &self,
        session_id: SessionId,
        message: &Message,
    ) -> Result<(), EnhancedError>;

    /// Remove a session from index
    async fn remove_session(&self, session_id: SessionId) -> Result<(), EnhancedError>;

    /// Remove a message from index
    async fn remove_message(
        &self,
        session_id: SessionId,
        message_id: uuid::Uuid,
    ) -> Result<(), EnhancedError>;

    /// Search using unified query
    async fn search(&self, query: &UnifiedSearchQuery) -> Result<Vec<SearchResult>, EnhancedError>;

    /// Commit changes (for backends that require explicit commit)
    async fn commit(&self) -> Result<(), EnhancedError>;
}
