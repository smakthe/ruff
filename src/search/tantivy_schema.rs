//! Tantivy schema definitions for message search indexing

use crate::events::{MessageId, SessionId};
use crate::session::manager::{Message, MessageRole};
use chrono::{DateTime, Local};
use tantivy::schema::*;
use tantivy::TantivyDocument;

/// Schema definition for message indexing
#[derive(Clone)]
pub struct MessageIndexSchema {
    pub schema: Schema,
    pub content_field: Field,
    pub role_field: Field,
    pub session_id_field: Field,
    pub message_id_field: Field,
    pub timestamp_field: Field,
    pub model_used_field: Field,
}

impl MessageIndexSchema {
    /// Create a new message index schema
    pub fn new() -> Self {
        let mut schema_builder = Schema::builder();

        // Full-text searchable content with term positions for phrase queries
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();
        let content_field = schema_builder.add_text_field("content", text_options);

        // Filterable fields as string (not analyzed)
        let string_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic),
            )
            .set_stored();

        let role_field = schema_builder.add_text_field("role", string_options.clone());
        let session_id_field = schema_builder.add_text_field("session_id", string_options.clone());
        let message_id_field = schema_builder.add_text_field("message_id", string_options.clone());
        let model_used_field = schema_builder.add_text_field("model_used", string_options);

        // Sortable and filterable timestamp (stored as i64 Unix timestamp)
        let timestamp_field = schema_builder.add_i64_field("timestamp", INDEXED | STORED | FAST);

        Self {
            schema: schema_builder.build(),
            content_field,
            role_field,
            session_id_field,
            message_id_field,
            timestamp_field,
            model_used_field,
        }
    }

    /// Convert a Message into a Tantivy Document
    pub fn message_to_document(&self, session_id: SessionId, message: &Message) -> TantivyDocument {
        let mut doc = TantivyDocument::new();

        // Add content (full-text searchable)
        doc.add_text(self.content_field, &message.content);

        // Add role
        let role_str = match message.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        };
        doc.add_text(self.role_field, role_str);

        // Add session and message IDs
        doc.add_text(self.session_id_field, session_id.to_string());
        doc.add_text(self.message_id_field, message.id.to_string());

        // Add timestamp (Unix timestamp in seconds)
        doc.add_i64(self.timestamp_field, message.timestamp.timestamp());

        // Add model used
        doc.add_text(self.model_used_field, &message.metadata.model_used);

        doc
    }

    /// Extract session_id from a Tantivy document
    pub fn extract_session_id(&self, doc: &TantivyDocument) -> Option<SessionId> {
        doc.get_first(self.session_id_field)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
    }

    /// Extract message_id from a Tantivy document
    pub fn extract_message_id(&self, doc: &TantivyDocument) -> Option<MessageId> {
        doc.get_first(self.message_id_field)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
    }

    /// Extract content from a Tantivy document
    pub fn extract_content(&self, doc: &TantivyDocument) -> Option<String> {
        doc.get_first(self.content_field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Extract role from a Tantivy document
    pub fn extract_role(&self, doc: &TantivyDocument) -> Option<MessageRole> {
        doc.get_first(self.role_field)
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "user" => Some(MessageRole::User),
                "assistant" => Some(MessageRole::Assistant),
                "system" => Some(MessageRole::System),
                _ => None,
            })
    }

    /// Extract timestamp from a Tantivy document
    pub fn extract_timestamp(&self, doc: &TantivyDocument) -> Option<DateTime<Local>> {
        doc.get_first(self.timestamp_field)
            .and_then(|v| v.as_i64())
            .and_then(|ts| {
                use chrono::TimeZone;
                Local.timestamp_opt(ts, 0).single()
            })
    }

    /// Extract model_used from a Tantivy document
    pub fn extract_model_used(&self, doc: &TantivyDocument) -> Option<String> {
        doc.get_first(self.model_used_field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

impl Default for MessageIndexSchema {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::manager::MessageMetadata;
    use uuid::Uuid;

    #[test]
    fn test_schema_creation() {
        let schema = MessageIndexSchema::new();
        assert!(schema.schema.num_fields() > 0);
    }

    #[test]
    fn test_message_to_document() {
        let schema = MessageIndexSchema::new();
        let session_id = Uuid::new_v4();
        let message = Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "Hello world".to_string(),
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
        };

        let doc = schema.message_to_document(session_id, &message);

        // Verify document has fields
        assert!(doc.get_first(schema.content_field).is_some());
        assert!(doc.get_first(schema.role_field).is_some());
        assert!(doc.get_first(schema.session_id_field).is_some());
        assert!(doc.get_first(schema.message_id_field).is_some());
        assert!(doc.get_first(schema.timestamp_field).is_some());
    }

    #[test]
    fn test_extract_fields() {
        let schema = MessageIndexSchema::new();
        let session_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let content = "Test message";

        let message = Message {
            id: message_id,
            role: MessageRole::Assistant,
            content: content.to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "gpt-4".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };

        let doc = schema.message_to_document(session_id, &message);

        // Extract and verify
        assert_eq!(schema.extract_session_id(&doc), Some(session_id));
        assert_eq!(schema.extract_message_id(&doc), Some(message_id));
        assert_eq!(schema.extract_content(&doc), Some(content.to_string()));
        assert_eq!(schema.extract_role(&doc), Some(MessageRole::Assistant));
        assert!(schema.extract_timestamp(&doc).is_some());
        assert_eq!(schema.extract_model_used(&doc), Some("gpt-4".to_string()));
    }
}
