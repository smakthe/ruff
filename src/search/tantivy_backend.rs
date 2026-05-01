//! Tantivy-based message search backend

use std::collections::HashMap;
use std::ops::Bound;
use std::path::PathBuf;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::{BooleanQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::Term;
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument};
use tokio::sync::{Mutex, RwLock};

use crate::events::{MessageId, SessionId};
use crate::message::search::{MessageSearchQuery, MessageSearchResult};
use crate::search::tantivy_schema::MessageIndexSchema;
use crate::session::manager::{ChatSession, Message, MessageRole};
use crate::EnhancedError;

/// Tantivy-backed message search index
pub struct TantivyMessageSearchIndex {
    index: Index,
    index_writer: Arc<Mutex<IndexWriter>>,
    reader: IndexReader,
    schema: MessageIndexSchema,
    sessions: Arc<RwLock<HashMap<SessionId, ChatSession>>>,
}

impl TantivyMessageSearchIndex {
    /// Create a new Tantivy-based message search index
    pub fn new(index_path: PathBuf) -> Result<Self, EnhancedError> {
        let schema_def = MessageIndexSchema::new();

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&index_path).map_err(|e| {
            EnhancedError::storage(format!("Failed to create index directory: {}", e))
        })?;

        // Open or create MmapDirectory
        let dir = MmapDirectory::open(&index_path)
            .map_err(|e| EnhancedError::storage(format!("Failed to open directory: {}", e)))?;

        // Check if index exists and open or create
        let index = if Index::exists(&dir).map_err(|e| {
            EnhancedError::storage(format!("Failed to check index existence: {}", e))
        })? {
            Index::open(dir)
                .map_err(|e| EnhancedError::storage(format!("Failed to open index: {}", e)))?
        } else {
            Index::create(dir, schema_def.schema.clone(), Default::default())
                .map_err(|e| EnhancedError::storage(format!("Failed to create index: {}", e)))?
        };

        // Create writer with 50MB heap
        let index_writer = index
            .writer(50_000_000)
            .map_err(|e| EnhancedError::storage(format!("Failed to create writer: {}", e)))?;

        // Create reader with manual reload policy
        let reader = index
            .reader()
            .map_err(|e| EnhancedError::storage(format!("Failed to create reader: {}", e)))?;

        Ok(Self {
            index,
            index_writer: Arc::new(Mutex::new(index_writer)),
            reader,
            schema: schema_def,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Index a message
    pub async fn index_message(
        &self,
        session_id: SessionId,
        message: &Message,
    ) -> Result<(), EnhancedError> {
        let doc = self.schema.message_to_document(session_id, message);

        let writer = self.index_writer.lock().await;
        writer
            .add_document(doc)
            .map_err(|e| EnhancedError::storage(format!("Failed to add document: {}", e)))?;

        Ok(())
    }

    /// Commit all pending changes
    pub async fn commit(&self) -> Result<(), EnhancedError> {
        let mut writer = self.index_writer.lock().await;
        writer
            .commit()
            .map_err(|e| EnhancedError::storage(format!("Failed to commit: {}", e)))?;

        // Reload reader to see changes
        self.reader
            .reload()
            .map_err(|e| EnhancedError::storage(format!("Failed to reload reader: {}", e)))?;

        Ok(())
    }

    /// Remove a message from the index
    pub async fn remove_message(
        &self,
        _session_id: SessionId,
        message_id: MessageId,
    ) -> Result<(), EnhancedError> {
        let term = Term::from_field_text(self.schema.message_id_field, &message_id.to_string());

        let mut writer = self.index_writer.lock().await;
        writer.delete_term(term);
        writer
            .commit()
            .map_err(|e| EnhancedError::storage(format!("Failed to commit deletion: {}", e)))?;

        // Reload reader
        self.reader
            .reload()
            .map_err(|e| EnhancedError::storage(format!("Failed to reload reader: {}", e)))?;

        Ok(())
    }

    /// Remove all messages for a session
    pub async fn remove_session(&self, session_id: SessionId) -> Result<(), EnhancedError> {
        self.sessions.write().await.remove(&session_id);

        let term = Term::from_field_text(self.schema.session_id_field, &session_id.to_string());

        let mut writer = self.index_writer.lock().await;
        writer.delete_term(term);
        writer.commit().map_err(|e| {
            EnhancedError::storage(format!("Failed to commit session deletion: {}", e))
        })?;

        // Reload reader
        self.reader
            .reload()
            .map_err(|e| EnhancedError::storage(format!("Failed to reload reader: {}", e)))?;

        Ok(())
    }

    /// Search for messages
    pub fn search(
        &self,
        query: &MessageSearchQuery,
    ) -> Result<Vec<MessageSearchResult>, EnhancedError> {
        let searcher = self.reader.searcher();

        // Build query
        let tantivy_query = self.build_query(query)?;

        // Search with top docs collector
        let top_docs = searcher
            .search(&tantivy_query, &TopDocs::with_limit(query.limit))
            .map_err(|e| EnhancedError::unknown(format!("Search failed: {}", e)))?;

        // Convert results
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc = searcher
                .doc(doc_address)
                .map_err(|e| EnhancedError::unknown(format!("Failed to retrieve doc: {}", e)))?;

            if let Some(result) = self.doc_to_search_result(&retrieved_doc, score, query)? {
                results.push(result);
            }
        }

        Ok(results)
    }

    async fn search_sessions(
        &self,
        query: &crate::search::unified::UnifiedSearchQuery,
    ) -> Vec<crate::search::unified::SearchResult> {
        let sessions = self.sessions.read().await;
        let query_text = query.text.trim().to_lowercase();
        let mut results = Vec::new();

        for session in sessions.values() {
            if let Some(session_ids) = &query.filters.session_ids {
                if !session_ids.contains(&session.id) {
                    continue;
                }
            }

            if let Some(model) = &query.filters.model {
                if &session.model != model {
                    continue;
                }
            }

            if let Some(tags) = &query.filters.tags {
                if !tags.iter().all(|tag| session.tags.contains(tag)) {
                    continue;
                }
            }

            if let Some(date_range) = &query.filters.date_range {
                let created_at = session.created_at.with_timezone(&chrono::Utc);
                if created_at < date_range.start || created_at > date_range.end {
                    continue;
                }
            }

            let mut matching_fields = Vec::new();
            let mut score = 0.0;

            if query_text.is_empty() {
                score = 1.0;
            } else {
                let title = session.title.to_lowercase();
                if title.contains(&query_text) {
                    matching_fields.push("title".to_string());
                    score += 2.0;
                }

                if session
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&query_text))
                {
                    matching_fields.push("tags".to_string());
                    score += 1.5;
                }

                if session.model.to_lowercase().contains(&query_text) {
                    matching_fields.push("model".to_string());
                    score += 1.0;
                }

                if session
                    .system_prompt
                    .as_deref()
                    .map(|prompt| prompt.to_lowercase().contains(&query_text))
                    .unwrap_or(false)
                {
                    matching_fields.push("system_prompt".to_string());
                    score += 0.75;
                }
            }

            if score > 0.0 {
                results.push(crate::search::unified::SearchResult::Session(
                    crate::search::unified::SessionSearchResult {
                        session: session.clone(),
                        score,
                        matching_fields,
                    },
                ));
            }
        }

        results.sort_by(|a, b| {
            let a_score = match a {
                crate::search::unified::SearchResult::Session(result) => result.score,
                crate::search::unified::SearchResult::Message(result) => result.score,
            };
            let b_score = match b {
                crate::search::unified::SearchResult::Session(result) => result.score,
                crate::search::unified::SearchResult::Message(result) => result.score,
            };
            b_score
                .partial_cmp(&a_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(query.limit);
        results
    }

    /// Build Tantivy query from search parameters
    fn build_query(&self, query: &MessageSearchQuery) -> Result<Box<dyn Query>, EnhancedError> {
        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        // Text query (required)
        if !query.text.is_empty() {
            let query_parser = QueryParser::for_index(&self.index, vec![self.schema.content_field]);
            let text_query = query_parser
                .parse_query(&query.text)
                .map_err(|e| EnhancedError::unknown(format!("Invalid query: {}", e)))?;
            subqueries.push((Occur::Must, text_query));
        }

        // Session ID filter
        if let Some(ref session_ids) = query.session_ids {
            let mut session_subqueries = Vec::new();
            for session_id in session_ids {
                let term =
                    Term::from_field_text(self.schema.session_id_field, &session_id.to_string());
                let term_query: Box<dyn Query> =
                    Box::new(TermQuery::new(term, IndexRecordOption::Basic));
                session_subqueries.push((Occur::Should, term_query));
            }
            if !session_subqueries.is_empty() {
                subqueries.push((Occur::Must, Box::new(BooleanQuery::new(session_subqueries))));
            }
        }

        // Role filter
        if let Some(ref roles) = query.roles {
            let mut role_subqueries = Vec::new();
            for role in roles {
                let role_str = match role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                };
                let term = Term::from_field_text(self.schema.role_field, role_str);
                let term_query: Box<dyn Query> =
                    Box::new(TermQuery::new(term, IndexRecordOption::Basic));
                role_subqueries.push((Occur::Should, term_query));
            }
            if !role_subqueries.is_empty() {
                subqueries.push((Occur::Must, Box::new(BooleanQuery::new(role_subqueries))));
            }
        }

        // Date range filter
        if let Some((start, end)) = query.date_range {
            use tantivy::query::RangeQuery;
            let start_ts = start.timestamp();
            let end_ts = end.timestamp();
            let range_query = RangeQuery::new_i64_bounds(
                "timestamp".to_string(),
                Bound::Included(start_ts),
                Bound::Included(end_ts),
            );
            subqueries.push((Occur::Must, Box::new(range_query)));
        }

        // Combine all queries
        if subqueries.is_empty() {
            return Err(EnhancedError::unknown("Empty query".to_string()));
        }

        Ok(Box::new(BooleanQuery::new(subqueries)))
    }

    /// Convert Tantivy document to search result
    fn doc_to_search_result(
        &self,
        doc: &TantivyDocument,
        score: f32,
        query: &MessageSearchQuery,
    ) -> Result<Option<MessageSearchResult>, EnhancedError> {
        let session_id = self
            .schema
            .extract_session_id(doc)
            .ok_or_else(|| EnhancedError::unknown("Missing session_id in document".to_string()))?;

        let message_id = self
            .schema
            .extract_message_id(doc)
            .ok_or_else(|| EnhancedError::unknown("Missing message_id in document".to_string()))?;

        let content = self
            .schema
            .extract_content(doc)
            .ok_or_else(|| EnhancedError::unknown("Missing content in document".to_string()))?;

        let role = self
            .schema
            .extract_role(doc)
            .ok_or_else(|| EnhancedError::unknown("Missing role in document".to_string()))?;

        let timestamp = self
            .schema
            .extract_timestamp(doc)
            .ok_or_else(|| EnhancedError::unknown("Missing timestamp in document".to_string()))?;

        let model_used = self.schema.extract_model_used(doc);

        // Create snippet (first 200 chars)
        let content_snippet = if content.len() > 200 {
            format!("{}...", &content[..200])
        } else {
            content.clone()
        };

        // Extract highlights (simple word matching for now)
        let highlights = self.extract_highlights(&content, &query.text);

        Ok(Some(MessageSearchResult {
            session_id,
            message_id,
            content_snippet,
            full_content: if query.include_metadata {
                Some(content)
            } else {
                None
            },
            relevance_score: score,
            timestamp,
            role,
            model_used,
            highlights,
        }))
    }

    /// Extract highlighted terms from content
    fn extract_highlights(&self, content: &str, query_text: &str) -> Vec<String> {
        let query_words: Vec<&str> = query_text.split_whitespace().collect();
        let mut highlights = Vec::new();

        for word in query_words {
            if content.to_lowercase().contains(&word.to_lowercase()) {
                highlights.push(word.to_string());
            }
        }

        highlights
    }

    /// Get index statistics
    pub fn get_statistics(&self) -> Result<IndexStatistics, EnhancedError> {
        let searcher = self.reader.searcher();
        let segment_metas = searcher.segment_readers();

        let total_documents: usize = segment_metas
            .iter()
            .map(|reader| reader.num_docs() as usize)
            .sum();

        let num_segments = segment_metas.len();

        // Estimate size (rough approximation)
        let estimated_size_bytes = total_documents * 1024;

        Ok(IndexStatistics {
            total_documents,
            num_segments,
            estimated_size_bytes,
        })
    }

    /// Clear the entire index
    pub async fn clear(&self) -> Result<(), EnhancedError> {
        let mut writer = self.index_writer.lock().await;
        writer
            .delete_all_documents()
            .map_err(|e| EnhancedError::storage(format!("Failed to clear index: {}", e)))?;
        writer
            .commit()
            .map_err(|e| EnhancedError::storage(format!("Failed to commit clear: {}", e)))?;

        // Reload reader
        self.reader
            .reload()
            .map_err(|e| EnhancedError::storage(format!("Failed to reload reader: {}", e)))?;

        Ok(())
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStatistics {
    pub total_documents: usize,
    pub num_segments: usize,
    pub estimated_size_bytes: usize,
}

// Implement SearchBackend trait for unified search interface
#[async_trait::async_trait]
impl crate::search::unified::SearchBackend for TantivyMessageSearchIndex {
    async fn index_session(
        &self,
        session: &crate::session::manager::ChatSession,
    ) -> Result<(), EnhancedError> {
        self.sessions
            .write()
            .await
            .insert(session.id, session.clone());
        Ok(())
    }

    async fn index_message(
        &self,
        session_id: SessionId,
        message: &crate::session::manager::Message,
    ) -> Result<(), EnhancedError> {
        self.index_message(session_id, message).await
    }

    async fn remove_session(&self, session_id: SessionId) -> Result<(), EnhancedError> {
        self.remove_session(session_id).await
    }

    async fn remove_message(
        &self,
        session_id: SessionId,
        message_id: uuid::Uuid,
    ) -> Result<(), EnhancedError> {
        self.remove_message(session_id, message_id).await
    }

    async fn search(
        &self,
        query: &crate::search::unified::UnifiedSearchQuery,
    ) -> Result<Vec<crate::search::unified::SearchResult>, EnhancedError> {
        match query.scope {
            crate::search::unified::SearchScope::Messages => {
                // Convert unified query to MessageSearchQuery
                let msg_query = crate::message::search::MessageSearchQuery {
                    text: query.text.clone(),
                    session_ids: query.filters.session_ids.clone(),
                    roles: None, // Map from query.filters.role if needed
                    date_range: None,
                    limit: query.limit,
                    include_metadata: true,
                };

                // Search using existing implementation
                let results = self.search(&msg_query)?;

                // Convert results to unified format
                let unified_results = results
                    .into_iter()
                    .map(|r| {
                        // Reconstruct minimal Message from search result
                        let message = crate::session::manager::Message {
                            id: r.message_id,
                            role: r.role,
                            content: r.full_content.clone().unwrap_or(r.content_snippet.clone()),
                            timestamp: r.timestamp,
                            edited_at: None,
                            token_usage: None,
                            parent_id: None,
                            children: vec![],
                            metadata: crate::session::manager::MessageMetadata {
                                model_used: r.model_used.clone().unwrap_or_default(),
                                temperature: 0.0,
                                response_time_ms: 0,
                                is_regenerated: false,
                                regeneration_count: 0,
                            },
                        };

                        crate::search::unified::SearchResult::Message(
                            crate::search::unified::MessageSearchResult {
                                session_id: r.session_id,
                                message,
                                score: r.relevance_score,
                                snippet: r.content_snippet,
                            },
                        )
                    })
                    .collect();

                Ok(unified_results)
            }
            crate::search::unified::SearchScope::Sessions => Ok(self.search_sessions(query).await),
            crate::search::unified::SearchScope::Both => {
                let mut session_results = self.search_sessions(query).await;
                let msg_query = crate::message::search::MessageSearchQuery {
                    text: query.text.clone(),
                    session_ids: query.filters.session_ids.clone(),
                    roles: None,
                    date_range: None,
                    limit: query.limit,
                    include_metadata: true,
                };

                let mut message_results: Vec<_> = self
                    .search(&msg_query)?
                    .into_iter()
                    .map(|r| {
                        let message = crate::session::manager::Message {
                            id: r.message_id,
                            role: r.role,
                            content: r.full_content.clone().unwrap_or(r.content_snippet.clone()),
                            timestamp: r.timestamp,
                            edited_at: None,
                            token_usage: None,
                            parent_id: None,
                            children: vec![],
                            metadata: crate::session::manager::MessageMetadata {
                                model_used: r.model_used.clone().unwrap_or_default(),
                                temperature: 0.0,
                                response_time_ms: 0,
                                is_regenerated: false,
                                regeneration_count: 0,
                            },
                        };

                        crate::search::unified::SearchResult::Message(
                            crate::search::unified::MessageSearchResult {
                                session_id: r.session_id,
                                message,
                                score: r.relevance_score,
                                snippet: r.content_snippet,
                            },
                        )
                    })
                    .collect();

                session_results.append(&mut message_results);
                session_results.truncate(query.limit);
                Ok(session_results)
            }
        }
    }

    async fn commit(&self) -> Result<(), EnhancedError> {
        self.commit().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::manager::MessageMetadata;
    use chrono::Local;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_message(content: &str, role: MessageRole) -> Message {
        Message {
            id: Uuid::new_v4(),
            role,
            content: content.to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
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
    async fn test_index_and_search() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let index = TantivyMessageSearchIndex::new(index_path).unwrap();
        let session_id = Uuid::new_v4();

        // Index a message
        let msg = create_test_message("hello world rust programming", MessageRole::User);
        index.index_message(session_id, &msg).await.unwrap();
        index.commit().await.unwrap();

        // Search
        let query = MessageSearchQuery {
            text: "rust".to_string(),
            ..Default::default()
        };

        let results = index.search(&query).unwrap();

        assert!(!results.is_empty());
        assert!(results[0].content_snippet.contains("rust"));
    }

    #[tokio::test]
    async fn test_session_filter() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let index = TantivyMessageSearchIndex::new(index_path).unwrap();
        let session1 = Uuid::new_v4();
        let session2 = Uuid::new_v4();

        // Index messages in different sessions
        let msg1 = create_test_message("message in session 1", MessageRole::User);
        let msg2 = create_test_message("message in session 2", MessageRole::User);

        index.index_message(session1, &msg1).await.unwrap();
        index.index_message(session2, &msg2).await.unwrap();
        index.commit().await.unwrap();

        // Search with session filter
        let query = MessageSearchQuery {
            text: "message".to_string(),
            session_ids: Some(vec![session1]),
            ..Default::default()
        };

        let results = index.search(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, session1);
    }

    #[tokio::test]
    async fn test_remove_message() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let index = TantivyMessageSearchIndex::new(index_path).unwrap();
        let session_id = Uuid::new_v4();
        let msg = create_test_message("test message", MessageRole::User);
        let msg_id = msg.id;

        // Index and search
        index.index_message(session_id, &msg).await.unwrap();
        index.commit().await.unwrap();

        let query = MessageSearchQuery {
            text: "test".to_string(),
            ..Default::default()
        };
        let results = index.search(&query).unwrap();
        assert_eq!(results.len(), 1);

        // Remove and verify
        index.remove_message(session_id, msg_id).await.unwrap();

        let results = index.search(&query).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_index_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let index = TantivyMessageSearchIndex::new(index_path).unwrap();
        let session_id = Uuid::new_v4();

        // Index multiple messages
        for i in 0..5 {
            let msg = create_test_message(&format!("message {}", i), MessageRole::User);
            index.index_message(session_id, &msg).await.unwrap();
        }
        index.commit().await.unwrap();

        let stats = index.get_statistics().unwrap();
        assert_eq!(stats.total_documents, 5);
    }
}
