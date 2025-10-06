//! Session search functionality

use std::collections::HashMap;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use crate::events::SessionId;
use crate::session::manager::ChatSession;

/// Search index for sessions with full-text and metadata search capabilities
pub struct SessionSearchIndex {
    /// Fuzzy matcher for title and content matching
    matcher: SkimMatcherV2,
    /// Indexed session data for fast searching
    indexed_sessions: HashMap<SessionId, IndexedSession>,
}

/// Indexed session data for search
#[derive(Debug, Clone)]
struct IndexedSession {
    pub id: SessionId,
    pub title: String,
    pub content_preview: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub last_activity: DateTime<Local>,
    pub message_count: u32,
    pub model: String,
    pub is_archived: bool,
}

/// Session search result with relevance scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSearchResult {
    pub session_id: SessionId,
    pub title: String,
    pub content_preview: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub last_activity: DateTime<Local>,
    pub message_count: u32,
    pub model: String,
    pub is_archived: bool,
    pub relevance_score: f32,
    pub match_type: MatchType,
}

/// Type of match found during search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    Title,
    Content,
    Tag,
    Model,
    Combined,
}

/// Search query for sessions
#[derive(Debug, Clone)]
pub struct SessionSearchQuery {
    pub text: String,
    pub filters: SessionSearchFilters,
    pub sort_by: SessionSortBy,
    pub limit: usize,
}

/// Filters for session search
#[derive(Debug, Clone)]
pub struct SessionSearchFilters {
    pub tags: Option<Vec<String>>,
    pub models: Option<Vec<String>>,
    pub date_range: Option<(DateTime<Local>, DateTime<Local>)>,
    pub min_message_count: Option<u32>,
    pub max_message_count: Option<u32>,
    pub include_archived: bool,
}

/// Sort options for session search results
#[derive(Debug, Clone)]
pub enum SessionSortBy {
    Relevance,
    CreatedAt,
    UpdatedAt,
    LastActivity,
    MessageCount,
    Title,
}

impl Default for SessionSearchFilters {
    fn default() -> Self {
        Self {
            tags: None,
            models: None,
            date_range: None,
            min_message_count: None,
            max_message_count: None,
            include_archived: false,
        }
    }
}

impl SessionSearchIndex {
    /// Create a new session search index
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
            indexed_sessions: HashMap::new(),
        }
    }
    
    /// Add or update a session in the search index
    pub fn index_session(&mut self, session: &ChatSession, messages: &[crate::session::manager::Message]) {
        let content_preview = self.generate_content_preview(messages);
        
        let indexed_session = IndexedSession {
            id: session.id,
            title: session.title.clone(),
            content_preview,
            tags: session.tags.clone(),
            created_at: session.created_at,
            updated_at: session.updated_at,
            last_activity: session.last_activity,
            message_count: session.message_count,
            model: session.model.clone(),
            is_archived: session.is_archived,
        };
        
        self.indexed_sessions.insert(session.id, indexed_session);
    }
    
    /// Remove a session from the search index
    pub fn remove_session(&mut self, session_id: SessionId) {
        self.indexed_sessions.remove(&session_id);
    }
    
    /// Search sessions with the given query
    pub fn search(&self, query: &SessionSearchQuery) -> Vec<SessionSearchResult> {
        let mut results = Vec::new();
        
        for indexed_session in self.indexed_sessions.values() {
            // Apply filters first
            if !self.matches_filters(indexed_session, &query.filters) {
                continue;
            }
            
            // Calculate relevance score
            if let Some((score, match_type)) = self.calculate_relevance(indexed_session, &query.text) {
                results.push(SessionSearchResult {
                    session_id: indexed_session.id,
                    title: indexed_session.title.clone(),
                    content_preview: indexed_session.content_preview.clone(),
                    tags: indexed_session.tags.clone(),
                    created_at: indexed_session.created_at,
                    updated_at: indexed_session.updated_at,
                    last_activity: indexed_session.last_activity,
                    message_count: indexed_session.message_count,
                    model: indexed_session.model.clone(),
                    is_archived: indexed_session.is_archived,
                    relevance_score: score,
                    match_type,
                });
            }
        }
        
        // Sort results
        self.sort_results(&mut results, &query.sort_by);
        
        // Apply limit
        if query.limit > 0 && results.len() > query.limit {
            results.truncate(query.limit);
        }
        
        results
    }
    
    /// Get all sessions matching filters (without text search)
    pub fn filter_sessions(&self, filters: &SessionSearchFilters) -> Vec<SessionSearchResult> {
        let mut results = Vec::new();
        
        for indexed_session in self.indexed_sessions.values() {
            if self.matches_filters(indexed_session, filters) {
                results.push(SessionSearchResult {
                    session_id: indexed_session.id,
                    title: indexed_session.title.clone(),
                    content_preview: indexed_session.content_preview.clone(),
                    tags: indexed_session.tags.clone(),
                    created_at: indexed_session.created_at,
                    updated_at: indexed_session.updated_at,
                    last_activity: indexed_session.last_activity,
                    message_count: indexed_session.message_count,
                    model: indexed_session.model.clone(),
                    is_archived: indexed_session.is_archived,
                    relevance_score: 1.0, // No text matching, so all have equal relevance
                    match_type: MatchType::Combined,
                });
            }
        }
        
        results
    }
    
    /// Get sessions by tags
    pub fn get_sessions_by_tags(&self, tags: &[String]) -> Vec<SessionSearchResult> {
        let mut results = Vec::new();
        
        for indexed_session in self.indexed_sessions.values() {
            if indexed_session.is_archived {
                continue;
            }
            
            let has_matching_tag = tags.iter().any(|tag| {
                indexed_session.tags.iter().any(|session_tag| {
                    session_tag.to_lowercase().contains(&tag.to_lowercase())
                })
            });
            
            if has_matching_tag {
                results.push(SessionSearchResult {
                    session_id: indexed_session.id,
                    title: indexed_session.title.clone(),
                    content_preview: indexed_session.content_preview.clone(),
                    tags: indexed_session.tags.clone(),
                    created_at: indexed_session.created_at,
                    updated_at: indexed_session.updated_at,
                    last_activity: indexed_session.last_activity,
                    message_count: indexed_session.message_count,
                    model: indexed_session.model.clone(),
                    is_archived: indexed_session.is_archived,
                    relevance_score: 1.0,
                    match_type: MatchType::Tag,
                });
            }
        }
        
        results
    }
    
    /// Get the total number of indexed sessions
    pub fn session_count(&self) -> usize {
        self.indexed_sessions.len()
    }
    
    /// Clear all indexed sessions
    pub fn clear(&mut self) {
        self.indexed_sessions.clear();
    }
    
    /// Generate a content preview from messages
    fn generate_content_preview(&self, messages: &[crate::session::manager::Message]) -> String {
        if messages.is_empty() {
            return "No messages".to_string();
        }

        // Take the first few messages and create a preview
        let preview_messages: Vec<String> = messages
            .iter()
            .take(3)
            .map(|msg| {
                let content = if msg.content.len() > 100 {
                    format!("{}...", &msg.content[..100])
                } else {
                    msg.content.clone()
                };
                format!("{}: {}", msg.role.to_string(), content)
            })
            .collect();
        
        preview_messages.join(" | ")
    }
    
    /// Check if a session matches the given filters
    fn matches_filters(&self, session: &IndexedSession, filters: &SessionSearchFilters) -> bool {
        // Check archived filter
        if !filters.include_archived && session.is_archived {
            return false;
        }
        
        // Check tags filter
        if let Some(filter_tags) = &filters.tags {
            let has_matching_tag = filter_tags.iter().any(|filter_tag| {
                session.tags.iter().any(|session_tag| {
                    session_tag.to_lowercase().contains(&filter_tag.to_lowercase())
                })
            });
            if !has_matching_tag {
                return false;
            }
        }
        
        // Check models filter
        if let Some(filter_models) = &filters.models {
            if !filter_models.contains(&session.model) {
                return false;
            }
        }
        
        // Check date range filter
        if let Some((start_date, end_date)) = &filters.date_range {
            if session.created_at < *start_date || session.created_at > *end_date {
                return false;
            }
        }
        
        // Check message count filters
        if let Some(min_count) = filters.min_message_count {
            if session.message_count < min_count {
                return false;
            }
        }
        
        if let Some(max_count) = filters.max_message_count {
            if session.message_count > max_count {
                return false;
            }
        }
        
        true
    }
    
    /// Calculate relevance score for a session against search text
    fn calculate_relevance(&self, session: &IndexedSession, search_text: &str) -> Option<(f32, MatchType)> {
        if search_text.trim().is_empty() {
            return Some((1.0, MatchType::Combined));
        }
        
        let search_lower = search_text.to_lowercase();
        let mut best_score = 0.0;
        let mut match_type = MatchType::Combined;
        
        // Check title match (highest weight)
        if let Some(score) = self.matcher.fuzzy_match(&session.title, &search_text) {
            let normalized_score = (score as f32) / 100.0 * 2.0; // Title matches get 2x weight
            if normalized_score > best_score {
                best_score = normalized_score;
                match_type = MatchType::Title;
            }
        }
        
        // Check exact title match (even higher weight)
        if session.title.to_lowercase().contains(&search_lower) {
            let score = 3.0; // Exact matches get highest weight
            if score > best_score {
                best_score = score;
                match_type = MatchType::Title;
            }
        }
        
        // Check content match
        if let Some(score) = self.matcher.fuzzy_match(&session.content_preview, &search_text) {
            let normalized_score = (score as f32) / 100.0;
            if normalized_score > best_score {
                best_score = normalized_score;
                match_type = MatchType::Content;
            }
        }
        
        // Check tag matches
        for tag in &session.tags {
            if let Some(score) = self.matcher.fuzzy_match(tag, &search_text) {
                let normalized_score = (score as f32) / 100.0 * 1.5; // Tag matches get 1.5x weight
                if normalized_score > best_score {
                    best_score = normalized_score;
                    match_type = MatchType::Tag;
                }
            }
            
            if tag.to_lowercase().contains(&search_lower) {
                let score = 2.0; // Exact tag matches get high weight
                if score > best_score {
                    best_score = score;
                    match_type = MatchType::Tag;
                }
            }
        }
        
        // Check model match
        if session.model.to_lowercase().contains(&search_lower) {
            let score = 1.0;
            if score > best_score {
                best_score = score;
                match_type = MatchType::Model;
            }
        }
        
        if best_score > 0.0 {
            Some((best_score, match_type))
        } else {
            None
        }
    }
    
    /// Sort search results based on the specified criteria
    fn sort_results(&self, results: &mut [SessionSearchResult], sort_by: &SessionSortBy) {
        match sort_by {
            SessionSortBy::Relevance => {
                results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
            }
            SessionSortBy::CreatedAt => {
                results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            }
            SessionSortBy::UpdatedAt => {
                results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            }
            SessionSortBy::LastActivity => {
                results.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
            }
            SessionSortBy::MessageCount => {
                results.sort_by(|a, b| b.message_count.cmp(&a.message_count));
            }
            SessionSortBy::Title => {
                results.sort_by(|a, b| a.title.cmp(&b.title));
            }
        }
    }
}

impl Default for SessionSearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

// Helper trait for converting MessageRole to string
impl ToString for crate::session::manager::MessageRole {
    fn to_string(&self) -> String {
        match self {
            crate::session::manager::MessageRole::User => "User".to_string(),
            crate::session::manager::MessageRole::Assistant => "Assistant".to_string(),
            crate::session::manager::MessageRole::System => "System".to_string(),
        }
    }
}