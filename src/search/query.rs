//! Search query processing

use crate::events::SessionId;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Search query structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub filters: SearchFilters,
    pub limit: usize,
}

/// Search filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilters {
    pub session_ids: Option<Vec<SessionId>>,
    pub date_range: Option<(DateTime<Local>, DateTime<Local>)>,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub content: String,
    pub score: f32,
}
