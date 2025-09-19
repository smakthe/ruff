//! Session metadata management

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use crate::events::SessionId;

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: SessionId,
    pub title: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub message_count: usize,
    pub total_tokens: u32,
}

/// Session tab representation for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTab {
    pub id: SessionId,
    pub title: String,
    pub is_active: bool,
    pub has_unsaved_changes: bool,
    pub last_activity: DateTime<Local>,
}

/// Session statistics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatistics {
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub archived_sessions: usize,
    pub total_messages: u32,
    pub total_tokens: u32,
    pub oldest_session: Option<DateTime<Local>>,
    pub most_recent_activity: Option<DateTime<Local>>,
}