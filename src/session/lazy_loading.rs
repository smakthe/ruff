//! Lazy loading system for sessions and messages
//! 
//! This module provides lazy loading capabilities to improve performance
//! when dealing with large sessions and message histories.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::events::{SessionId, MessageId};
use crate::session::manager::{ChatSession, Message};
use crate::RuffError;

/// Lazy session loader that loads sessions on demand
pub struct LazySessionLoader {
    /// Cache of loaded sessions
    session_cache: Arc<RwLock<HashMap<SessionId, Arc<ChatSession>>>>,
    /// Session metadata for quick access without loading full session
    session_metadata: Arc<RwLock<HashMap<SessionId, SessionMetadata>>>,
    /// Storage path for session files
    storage_path: PathBuf,
    /// Maximum number of sessions to keep in cache
    max_cache_size: usize,
    /// LRU tracking for cache eviction
    access_order: Arc<RwLock<Vec<SessionId>>>,
}

/// Lightweight session metadata for lazy loading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: SessionId,
    pub title: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub message_count: u32,
    pub total_tokens: u32,
    pub model: String,
    pub tags: Vec<String>,
    pub is_archived: bool,
    pub file_size: u64,
}

/// Lazy message loader for large message histories
pub struct LazyMessageLoader {
    /// Cache of loaded message chunks
    message_cache: Arc<RwLock<HashMap<MessageChunkId, Arc<MessageChunk>>>>,
    /// Message index for quick lookups
    message_index: Arc<RwLock<HashMap<SessionId, MessageIndex>>>,
    /// Storage path for message chunks
    storage_path: PathBuf,
    /// Messages per chunk
    chunk_size: usize,
    /// Maximum number of chunks to keep in cache
    max_cache_size: usize,
}

/// Message chunk for lazy loading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageChunk {
    pub id: MessageChunkId,
    pub session_id: SessionId,
    pub chunk_index: usize,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Local>,
}

/// Message index for efficient lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageIndex {
    pub session_id: SessionId,
    pub total_messages: usize,
    pub chunks: Vec<MessageChunkInfo>,
    pub message_to_chunk: HashMap<MessageId, usize>,
}

/// Information about a message chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageChunkInfo {
    pub chunk_id: MessageChunkId,
    pub chunk_index: usize,
    pub message_count: usize,
    pub start_timestamp: DateTime<Local>,
    pub end_timestamp: DateTime<Local>,
    pub file_size: u64,
}

type MessageChunkId = Uuid;

impl LazySessionLoader {
    /// Create a new lazy session loader
    pub fn new(storage_path: PathBuf, max_cache_size: usize) -> Self {
        Self {
            session_cache: Arc::new(RwLock::new(HashMap::new())),
            session_metadata: Arc::new(RwLock::new(HashMap::new())),
            storage_path,
            max_cache_size,
            access_order: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize the loader by scanning for session metadata
    pub async fn initialize(&self) -> Result<(), RuffError> {
        let mut metadata_map = HashMap::new();

        if !self.storage_path.exists() {
            return Ok(());
        }

        let mut entries = fs::read_dir(&self.storage_path).await
            .map_err(|e| RuffError::App(format!("Failed to read sessions directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| RuffError::App(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_session_metadata(&path).await {
                    Ok(metadata) => {
                        metadata_map.insert(metadata.id, metadata);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load session metadata from {}: {}", path.display(), e);
                    }
                }
            }
        }

        let mut session_metadata = self.session_metadata.write().unwrap();
        *session_metadata = metadata_map;

        Ok(())
    }

    /// Load session metadata from file without loading the full session
    async fn load_session_metadata(&self, path: &std::path::Path) -> Result<SessionMetadata, RuffError> {
        let content = fs::read_to_string(path).await
            .map_err(|e| RuffError::App(format!("Failed to read session file: {}", e)))?;

        // Parse just enough to get metadata
        let session: ChatSession = serde_json::from_str(&content)
            .map_err(|e| RuffError::App(format!("Failed to deserialize session: {}", e)))?;

        let file_size = fs::metadata(path).await
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(SessionMetadata {
            id: session.id,
            title: session.title,
            created_at: session.created_at,
            updated_at: session.updated_at,
            message_count: session.message_count,
            total_tokens: session.total_tokens_used.total_tokens,
            model: session.model,
            tags: session.tags,
            is_archived: session.is_archived,
            file_size,
        })
    }

    /// Get session metadata without loading the full session
    pub fn get_session_metadata(&self, session_id: SessionId) -> Option<SessionMetadata> {
        let metadata = self.session_metadata.read().unwrap();
        metadata.get(&session_id).cloned()
    }

    /// Get all session metadata
    pub fn get_all_session_metadata(&self) -> Vec<SessionMetadata> {
        let metadata = self.session_metadata.read().unwrap();
        metadata.values().cloned().collect()
    }

    /// Load a session on demand
    pub async fn load_session(&self, session_id: SessionId) -> Result<Arc<ChatSession>, RuffError> {
        // Check cache first
        {
            let cache = self.session_cache.read().unwrap();
            if let Some(session) = cache.get(&session_id) {
                self.update_access_order(session_id);
                return Ok(Arc::clone(session));
            }
        }

        // Load from storage
        let session_file = self.get_session_file_path(session_id);
        let content = fs::read_to_string(&session_file).await
            .map_err(|e| RuffError::App(format!("Failed to read session file: {}", e)))?;

        let session: ChatSession = serde_json::from_str(&content)
            .map_err(|e| RuffError::App(format!("Failed to deserialize session: {}", e)))?;

        let session_arc = Arc::new(session);

        // Add to cache
        self.add_to_cache(session_id, Arc::clone(&session_arc));

        Ok(session_arc)
    }

    /// Add session to cache with LRU eviction
    fn add_to_cache(&self, session_id: SessionId, session: Arc<ChatSession>) {
        let mut cache = self.session_cache.write().unwrap();
        
        // Evict if cache is full
        if cache.len() >= self.max_cache_size {
            self.evict_lru_session(&mut cache);
        }

        cache.insert(session_id, session);
        self.update_access_order(session_id);
    }

    /// Evict least recently used session from cache
    fn evict_lru_session(&self, cache: &mut HashMap<SessionId, Arc<ChatSession>>) {
        let mut access_order = self.access_order.write().unwrap();
        
        if let Some(lru_session_id) = access_order.first().copied() {
            cache.remove(&lru_session_id);
            access_order.retain(|&id| id != lru_session_id);
        }
    }

    /// Update access order for LRU tracking
    fn update_access_order(&self, session_id: SessionId) {
        let mut access_order = self.access_order.write().unwrap();
        
        // Remove if already present
        access_order.retain(|&id| id != session_id);
        
        // Add to end (most recently used)
        access_order.push(session_id);
    }

    /// Get session file path
    fn get_session_file_path(&self, session_id: SessionId) -> PathBuf {
        self.storage_path.join(format!("{}.json", session_id))
    }

    /// Check if session is cached
    pub fn is_session_cached(&self, session_id: SessionId) -> bool {
        let cache = self.session_cache.read().unwrap();
        cache.contains_key(&session_id)
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        let cache = self.session_cache.read().unwrap();
        let metadata = self.session_metadata.read().unwrap();
        
        CacheStats {
            cached_sessions: cache.len(),
            total_sessions: metadata.len(),
            cache_hit_ratio: 0.0, // Would need to track hits/misses
            memory_usage_bytes: 0, // Would need to calculate actual memory usage
        }
    }

    /// Clear cache
    pub fn clear_cache(&self) {
        let mut cache = self.session_cache.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();
        
        cache.clear();
        access_order.clear();
    }

    /// Preload sessions based on usage patterns
    pub async fn preload_sessions(&self, session_ids: Vec<SessionId>) -> Result<(), RuffError> {
        for session_id in session_ids {
            if !self.is_session_cached(session_id) {
                if let Err(e) = self.load_session(session_id).await {
                    eprintln!("Warning: Failed to preload session {}: {}", session_id, e);
                }
            }
        }
        Ok(())
    }
}

impl LazyMessageLoader {
    /// Create a new lazy message loader
    pub fn new(storage_path: PathBuf, chunk_size: usize, max_cache_size: usize) -> Self {
        Self {
            message_cache: Arc::new(RwLock::new(HashMap::new())),
            message_index: Arc::new(RwLock::new(HashMap::new())),
            storage_path,
            chunk_size,
            max_cache_size,
        }
    }

    /// Initialize message indices for all sessions
    pub async fn initialize(&self) -> Result<(), RuffError> {
        // Create storage directory if it doesn't exist
        fs::create_dir_all(&self.storage_path).await
            .map_err(|e| RuffError::App(format!("Failed to create message storage directory: {}", e)))?;

        // Load existing message indices
        self.load_message_indices().await?;

        Ok(())
    }

    /// Load message indices from storage
    async fn load_message_indices(&self) -> Result<(), RuffError> {
        let index_file = self.storage_path.join("message_indices.json");
        
        if !index_file.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&index_file).await
            .map_err(|e| RuffError::App(format!("Failed to read message indices: {}", e)))?;

        let indices: HashMap<SessionId, MessageIndex> = serde_json::from_str(&content)
            .map_err(|e| RuffError::App(format!("Failed to deserialize message indices: {}", e)))?;

        let mut message_index = self.message_index.write().unwrap();
        *message_index = indices;

        Ok(())
    }

    /// Save message indices to storage
    async fn save_message_indices(&self) -> Result<(), RuffError> {
        let index_file = self.storage_path.join("message_indices.json");
        let message_index = self.message_index.read().unwrap();
        
        let content = serde_json::to_string_pretty(&*message_index)
            .map_err(|e| RuffError::App(format!("Failed to serialize message indices: {}", e)))?;

        fs::write(&index_file, content).await
            .map_err(|e| RuffError::App(format!("Failed to save message indices: {}", e)))?;

        Ok(())
    }

    /// Create message chunks for a session
    pub async fn create_message_chunks(&self, session_id: SessionId, messages: Vec<Message>) -> Result<(), RuffError> {
        let chunks: Vec<MessageChunk> = messages
            .chunks(self.chunk_size)
            .enumerate()
            .map(|(index, chunk_messages)| {
                let chunk_id = Uuid::new_v4();
                MessageChunk {
                    id: chunk_id,
                    session_id,
                    chunk_index: index,
                    messages: chunk_messages.to_vec(),
                    created_at: Local::now(),
                }
            })
            .collect();

        // Save chunks to storage
        for chunk in &chunks {
            self.save_message_chunk(chunk).await?;
        }

        // Create and save message index
        let mut message_to_chunk = HashMap::new();
        let chunk_infos: Vec<MessageChunkInfo> = chunks
            .iter()
            .map(|chunk| {
                // Map messages to chunk index
                for message in &chunk.messages {
                    message_to_chunk.insert(message.id, chunk.chunk_index);
                }

                let start_timestamp = chunk.messages.first().map(|m| m.timestamp).unwrap_or_else(Local::now);
                let end_timestamp = chunk.messages.last().map(|m| m.timestamp).unwrap_or_else(Local::now);

                MessageChunkInfo {
                    chunk_id: chunk.id,
                    chunk_index: chunk.chunk_index,
                    message_count: chunk.messages.len(),
                    start_timestamp,
                    end_timestamp,
                    file_size: 0, // Would be calculated from actual file
                }
            })
            .collect();

        let message_index = MessageIndex {
            session_id,
            total_messages: messages.len(),
            chunks: chunk_infos,
            message_to_chunk,
        };

        // Update index
        {
            let mut indices = self.message_index.write().unwrap();
            indices.insert(session_id, message_index);
        }

        // Save indices
        self.save_message_indices().await?;

        Ok(())
    }

    /// Load a message chunk on demand
    pub async fn load_message_chunk(&self, session_id: SessionId, chunk_index: usize) -> Result<Arc<MessageChunk>, RuffError> {
        // Get chunk ID from index
        let chunk_id = {
            let indices = self.message_index.read().unwrap();
            let session_index = indices.get(&session_id)
                .ok_or_else(|| RuffError::App(format!("No message index for session {}", session_id)))?;
            
            session_index.chunks.get(chunk_index)
                .ok_or_else(|| RuffError::App(format!("Chunk {} not found for session {}", chunk_index, session_id)))?
                .chunk_id
        };

        // Check cache first
        {
            let cache = self.message_cache.read().unwrap();
            if let Some(chunk) = cache.get(&chunk_id) {
                return Ok(Arc::clone(chunk));
            }
        }

        // Load from storage
        let chunk_file = self.get_chunk_file_path(session_id, chunk_id);
        let content = fs::read_to_string(&chunk_file).await
            .map_err(|e| RuffError::App(format!("Failed to read message chunk: {}", e)))?;

        let chunk: MessageChunk = serde_json::from_str(&content)
            .map_err(|e| RuffError::App(format!("Failed to deserialize message chunk: {}", e)))?;

        let chunk_arc = Arc::new(chunk);

        // Add to cache
        self.add_chunk_to_cache(chunk_id, Arc::clone(&chunk_arc));

        Ok(chunk_arc)
    }

    /// Get a specific message by ID
    pub async fn get_message(&self, session_id: SessionId, message_id: MessageId) -> Result<Option<Message>, RuffError> {
        // Find which chunk contains the message
        let chunk_index = {
            let indices = self.message_index.read().unwrap();
            let session_index = indices.get(&session_id)
                .ok_or_else(|| RuffError::App(format!("No message index for session {}", session_id)))?;
            
            session_index.message_to_chunk.get(&message_id).copied()
        };

        if let Some(chunk_index) = chunk_index {
            let chunk = self.load_message_chunk(session_id, chunk_index).await?;
            Ok(chunk.messages.iter().find(|m| m.id == message_id).cloned())
        } else {
            Ok(None)
        }
    }

    /// Get messages in a range (for virtual scrolling)
    pub async fn get_message_range(&self, session_id: SessionId, start: usize, count: usize) -> Result<Vec<Message>, RuffError> {
        let indices = self.message_index.read().unwrap();
        let session_index = indices.get(&session_id)
            .ok_or_else(|| RuffError::App(format!("No message index for session {}", session_id)))?;

        let mut messages = Vec::new();
        let end = (start + count).min(session_index.total_messages);

        // Determine which chunks we need
        let start_chunk = start / self.chunk_size;
        let end_chunk = (end - 1) / self.chunk_size;

        for chunk_index in start_chunk..=end_chunk {
            if chunk_index < session_index.chunks.len() {
                let chunk = self.load_message_chunk(session_id, chunk_index).await?;
                
                // Calculate which messages from this chunk we need
                let chunk_start = chunk_index * self.chunk_size;
                let _chunk_end = chunk_start + chunk.messages.len();
                
                let range_start = start.saturating_sub(chunk_start);
                let range_end = (end - chunk_start).min(chunk.messages.len());
                
                if range_start < range_end {
                    messages.extend_from_slice(&chunk.messages[range_start..range_end]);
                }
            }
        }

        Ok(messages)
    }

    /// Save a message chunk to storage
    async fn save_message_chunk(&self, chunk: &MessageChunk) -> Result<(), RuffError> {
        let chunk_file = self.get_chunk_file_path(chunk.session_id, chunk.id);
        
        // Ensure directory exists
        if let Some(parent) = chunk_file.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| RuffError::App(format!("Failed to create chunk directory: {}", e)))?;
        }

        let content = serde_json::to_string_pretty(chunk)
            .map_err(|e| RuffError::App(format!("Failed to serialize message chunk: {}", e)))?;

        fs::write(&chunk_file, content).await
            .map_err(|e| RuffError::App(format!("Failed to save message chunk: {}", e)))?;

        Ok(())
    }

    /// Get chunk file path
    fn get_chunk_file_path(&self, session_id: SessionId, chunk_id: MessageChunkId) -> PathBuf {
        self.storage_path
            .join(session_id.to_string())
            .join(format!("{}.json", chunk_id))
    }

    /// Add chunk to cache with LRU eviction
    fn add_chunk_to_cache(&self, chunk_id: MessageChunkId, chunk: Arc<MessageChunk>) {
        let mut cache = self.message_cache.write().unwrap();
        
        // Evict if cache is full
        if cache.len() >= self.max_cache_size {
            // Simple eviction - remove first entry (should be LRU in practice)
            if let Some(first_key) = cache.keys().next().copied() {
                cache.remove(&first_key);
            }
        }

        cache.insert(chunk_id, chunk);
    }

    /// Get message index for a session
    pub fn get_message_index(&self, session_id: SessionId) -> Option<MessageIndex> {
        let indices = self.message_index.read().unwrap();
        indices.get(&session_id).cloned()
    }

    /// Get total message count for a session
    pub fn get_message_count(&self, session_id: SessionId) -> usize {
        let indices = self.message_index.read().unwrap();
        indices.get(&session_id)
            .map(|index| index.total_messages)
            .unwrap_or(0)
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cached_sessions: usize,
    pub total_sessions: usize,
    pub cache_hit_ratio: f64,
    pub memory_usage_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::session::manager::{MessageRole, MessageMetadata};
    use crate::models::TokenUsage;

    fn create_test_message(id: MessageId, content: &str) -> Message {
        Message {
            id,
            role: MessageRole::User,
            content: content.to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: Some(TokenUsage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
            }),
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test-model".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        }
    }

    #[tokio::test]
    async fn test_lazy_session_loader_creation() {
        let temp_dir = TempDir::new().unwrap();
        let loader = LazySessionLoader::new(temp_dir.path().to_path_buf(), 10);
        
        loader.initialize().await.unwrap();
        assert_eq!(loader.get_all_session_metadata().len(), 0);
    }

    #[tokio::test]
    async fn test_lazy_message_loader_creation() {
        let temp_dir = TempDir::new().unwrap();
        let loader = LazyMessageLoader::new(temp_dir.path().to_path_buf(), 100, 10);
        
        loader.initialize().await.unwrap();
        
        let session_id = Uuid::new_v4();
        assert_eq!(loader.get_message_count(session_id), 0);
    }

    #[tokio::test]
    async fn test_message_chunking() {
        let temp_dir = TempDir::new().unwrap();
        let loader = LazyMessageLoader::new(temp_dir.path().to_path_buf(), 2, 10);
        loader.initialize().await.unwrap();

        let session_id = Uuid::new_v4();
        let messages = vec![
            create_test_message(Uuid::new_v4(), "Message 1"),
            create_test_message(Uuid::new_v4(), "Message 2"),
            create_test_message(Uuid::new_v4(), "Message 3"),
            create_test_message(Uuid::new_v4(), "Message 4"),
            create_test_message(Uuid::new_v4(), "Message 5"),
        ];

        loader.create_message_chunks(session_id, messages.clone()).await.unwrap();

        // Should create 3 chunks (2, 2, 1 messages)
        let index = loader.get_message_index(session_id).unwrap();
        assert_eq!(index.chunks.len(), 3);
        assert_eq!(index.total_messages, 5);

        // Test loading chunks
        let chunk0 = loader.load_message_chunk(session_id, 0).await.unwrap();
        assert_eq!(chunk0.messages.len(), 2);
        assert_eq!(chunk0.messages[0].content, "Message 1");

        let chunk2 = loader.load_message_chunk(session_id, 2).await.unwrap();
        assert_eq!(chunk2.messages.len(), 1);
        assert_eq!(chunk2.messages[0].content, "Message 5");
    }

    #[tokio::test]
    async fn test_message_range_loading() {
        let temp_dir = TempDir::new().unwrap();
        let loader = LazyMessageLoader::new(temp_dir.path().to_path_buf(), 2, 10);
        loader.initialize().await.unwrap();

        let session_id = Uuid::new_v4();
        let messages = vec![
            create_test_message(Uuid::new_v4(), "Message 1"),
            create_test_message(Uuid::new_v4(), "Message 2"),
            create_test_message(Uuid::new_v4(), "Message 3"),
            create_test_message(Uuid::new_v4(), "Message 4"),
            create_test_message(Uuid::new_v4(), "Message 5"),
        ];

        loader.create_message_chunks(session_id, messages.clone()).await.unwrap();

        // Load range spanning multiple chunks
        let range_messages = loader.get_message_range(session_id, 1, 3).await.unwrap();
        assert_eq!(range_messages.len(), 3);
        assert_eq!(range_messages[0].content, "Message 2");
        assert_eq!(range_messages[1].content, "Message 3");
        assert_eq!(range_messages[2].content, "Message 4");
    }

    #[tokio::test]
    async fn test_get_specific_message() {
        let temp_dir = TempDir::new().unwrap();
        let loader = LazyMessageLoader::new(temp_dir.path().to_path_buf(), 2, 10);
        loader.initialize().await.unwrap();

        let session_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let messages = vec![
            create_test_message(Uuid::new_v4(), "Message 1"),
            create_test_message(message_id, "Target Message"),
            create_test_message(Uuid::new_v4(), "Message 3"),
        ];

        loader.create_message_chunks(session_id, messages.clone()).await.unwrap();

        let found_message = loader.get_message(session_id, message_id).await.unwrap();
        assert!(found_message.is_some());
        assert_eq!(found_message.unwrap().content, "Target Message");

        // Test non-existent message
        let not_found = loader.get_message(session_id, Uuid::new_v4()).await.unwrap();
        assert!(not_found.is_none());
    }
}