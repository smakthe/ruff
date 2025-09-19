//! Message operations

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::events::MessageId;
use crate::session::manager::{Message, MessageRole, MessageMetadata};
use crate::models::TokenUsage;
use crate::RuffError;

/// Message operations handler for advanced message manipulations
pub struct MessageOperations {
    /// Version history for messages
    version_history: std::collections::HashMap<MessageId, Vec<MessageVersion>>,
}

/// Message version for tracking edits and regenerations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageVersion {
    pub id: MessageId,
    pub version: u32,
    pub content: String,
    pub timestamp: DateTime<Local>,
    pub metadata: MessageMetadata,
    pub token_usage: Option<TokenUsage>,
}

/// Message operation result
#[derive(Debug, Clone)]
pub struct MessageOperationResult {
    pub message_id: MessageId,
    pub operation: MessageOperation,
    pub success: bool,
    pub error: Option<String>,
}

/// Types of message operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageOperation {
    Create,
    Edit,
    Delete,
    Regenerate,
    Copy,
    Thread,
}

impl MessageOperations {
    pub fn new() -> Self {
        Self {
            version_history: std::collections::HashMap::new(),
        }
    }

    /// Create a new message with proper initialization
    pub fn create_message(
        &self,
        role: MessageRole,
        content: String,
        parent_id: Option<MessageId>,
        model_used: String,
        temperature: f32,
    ) -> Message {
        Message {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used,
                temperature,
                response_time_ms: 0,
                is_regenerated: false,
                regeneration_count: 0,
            },
        }
    }

    /// Create a system message
    pub fn create_system_message(&self, content: String) -> Message {
        self.create_message(
            MessageRole::System,
            content,
            None,
            "system".to_string(),
            0.0,
        )
    }

    /// Create a user message
    pub fn create_user_message(&self, content: String, parent_id: Option<MessageId>) -> Message {
        self.create_message(
            MessageRole::User,
            content,
            parent_id,
            "user".to_string(),
            0.0,
        )
    }

    /// Create an assistant message
    pub fn create_assistant_message(
        &self,
        content: String,
        parent_id: Option<MessageId>,
        model_used: String,
        temperature: f32,
    ) -> Message {
        self.create_message(
            MessageRole::Assistant,
            content,
            parent_id,
            model_used,
            temperature,
        )
    }

    /// Save a version of a message before editing
    pub fn save_message_version(&mut self, message: &Message) {
        let version = MessageVersion {
            id: message.id,
            version: self.get_next_version_number(message.id),
            content: message.content.clone(),
            timestamp: message.timestamp,
            metadata: message.metadata.clone(),
            token_usage: message.token_usage.clone(),
        };

        self.version_history
            .entry(message.id)
            .or_insert_with(Vec::new)
            .push(version);
    }

    /// Get version history for a message
    pub fn get_message_versions(&self, message_id: MessageId) -> Vec<&MessageVersion> {
        self.version_history
            .get(&message_id)
            .map(|versions| versions.iter().collect())
            .unwrap_or_default()
    }

    /// Get the latest version number for a message
    fn get_next_version_number(&self, message_id: MessageId) -> u32 {
        self.version_history
            .get(&message_id)
            .map(|versions| versions.len() as u32 + 1)
            .unwrap_or(1)
    }

    /// Restore a message to a previous version
    pub fn restore_message_version(
        &self,
        message: &mut Message,
        version: u32,
    ) -> Result<(), RuffError> {
        let versions = self.version_history
            .get(&message.id)
            .ok_or_else(|| RuffError::App("No version history found for message".to_string()))?;

        let target_version = versions
            .iter()
            .find(|v| v.version == version)
            .ok_or_else(|| RuffError::App(format!("Version {} not found", version)))?;

        message.content = target_version.content.clone();
        message.metadata = target_version.metadata.clone();
        message.token_usage = target_version.token_usage.clone();
        message.edited_at = Some(Local::now());

        Ok(())
    }

    /// Prepare a message for regeneration
    pub fn prepare_for_regeneration(&mut self, message: &mut Message) {
        // Save current version before regeneration
        self.save_message_version(message);

        // Update metadata
        message.metadata.is_regenerated = true;
        message.metadata.regeneration_count += 1;
        message.edited_at = Some(Local::now());
    }

    /// Validate message content
    pub fn validate_message_content(&self, content: &str) -> Result<(), RuffError> {
        if content.trim().is_empty() {
            return Err(RuffError::App("Message content cannot be empty".to_string()));
        }

        if content.len() > 100_000 {
            return Err(RuffError::App("Message content exceeds maximum length".to_string()));
        }

        Ok(())
    }

    /// Sanitize message content
    pub fn sanitize_message_content(&self, content: String) -> String {
        // Remove null bytes and other control characters except newlines and tabs
        content
            .chars()
            .filter(|&c| c == '\n' || c == '\t' || c >= ' ')
            .collect()
    }

    /// Calculate message statistics
    pub fn calculate_message_stats(&self, message: &Message) -> MessageStats {
        let content = &message.content;
        let word_count = content.split_whitespace().count();
        let char_count = content.chars().count();
        let line_count = content.lines().count();
        
        // Estimate reading time (average 200 words per minute)
        let estimated_reading_time_seconds = (word_count as f64 / 200.0 * 60.0) as u32;

        MessageStats {
            word_count,
            char_count,
            line_count,
            estimated_reading_time_seconds,
            has_code_blocks: content.contains("```"),
            has_links: content.contains("http://") || content.contains("https://"),
            version_count: self.version_history
                .get(&message.id)
                .map(|v| v.len())
                .unwrap_or(0),
        }
    }

    /// Clear version history for a message
    pub fn clear_message_versions(&mut self, message_id: MessageId) {
        self.version_history.remove(&message_id);
    }

    /// Clear all version history
    pub fn clear_all_versions(&mut self) {
        self.version_history.clear();
    }

    /// Get total version count across all messages
    pub fn get_total_version_count(&self) -> usize {
        self.version_history.values().map(|v| v.len()).sum()
    }

    /// Get messages with version history
    pub fn get_messages_with_versions(&self) -> Vec<MessageId> {
        self.version_history.keys().cloned().collect()
    }
}

/// Message statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStats {
    pub word_count: usize,
    pub char_count: usize,
    pub line_count: usize,
    pub estimated_reading_time_seconds: u32,
    pub has_code_blocks: bool,
    pub has_links: bool,
    pub version_count: usize,
}

impl Default for MessageOperations {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_message() {
        let ops = MessageOperations::new();
        let message = ops.create_user_message("Hello, world!".to_string(), None);

        assert_eq!(message.role, MessageRole::User);
        assert_eq!(message.content, "Hello, world!");
        assert_eq!(message.parent_id, None);
        assert_eq!(message.metadata.model_used, "user");
    }

    #[test]
    fn test_create_system_message() {
        let ops = MessageOperations::new();
        let message = ops.create_system_message("You are a helpful assistant.".to_string());

        assert_eq!(message.role, MessageRole::System);
        assert_eq!(message.content, "You are a helpful assistant.");
        assert_eq!(message.metadata.model_used, "system");
    }

    #[test]
    fn test_create_assistant_message() {
        let ops = MessageOperations::new();
        let message = ops.create_assistant_message(
            "Hello! How can I help you?".to_string(),
            None,
            "gpt-4".to_string(),
            0.7,
        );

        assert_eq!(message.role, MessageRole::Assistant);
        assert_eq!(message.content, "Hello! How can I help you?");
        assert_eq!(message.metadata.model_used, "gpt-4");
        assert_eq!(message.metadata.temperature, 0.7);
    }

    #[test]
    fn test_save_and_get_message_versions() {
        let mut ops = MessageOperations::new();
        let mut message = ops.create_user_message("Original content".to_string(), None);

        // Save initial version
        ops.save_message_version(&message);

        // Modify message and save another version
        message.content = "Modified content".to_string();
        ops.save_message_version(&message);

        let versions = ops.get_message_versions(message.id);
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].content, "Original content");
        assert_eq!(versions[1].content, "Modified content");
        assert_eq!(versions[0].version, 1);
        assert_eq!(versions[1].version, 2);
    }

    #[test]
    fn test_restore_message_version() {
        let mut ops = MessageOperations::new();
        let mut message = ops.create_user_message("Original content".to_string(), None);

        // Save initial version
        ops.save_message_version(&message);

        // Modify message
        message.content = "Modified content".to_string();

        // Restore to version 1
        ops.restore_message_version(&mut message, 1).unwrap();

        assert_eq!(message.content, "Original content");
        assert!(message.edited_at.is_some());
    }

    #[test]
    fn test_restore_nonexistent_version() {
        let ops = MessageOperations::new();
        let mut message = ops.create_user_message("Content".to_string(), None);

        let result = ops.restore_message_version(&mut message, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No version history"));
    }

    #[test]
    fn test_prepare_for_regeneration() {
        let mut ops = MessageOperations::new();
        let mut message = ops.create_assistant_message(
            "Original response".to_string(),
            None,
            "gpt-4".to_string(),
            0.7,
        );

        assert!(!message.metadata.is_regenerated);
        assert_eq!(message.metadata.regeneration_count, 0);

        ops.prepare_for_regeneration(&mut message);

        assert!(message.metadata.is_regenerated);
        assert_eq!(message.metadata.regeneration_count, 1);
        assert!(message.edited_at.is_some());

        // Should have saved a version
        let versions = ops.get_message_versions(message.id);
        assert_eq!(versions.len(), 1);
    }

    #[test]
    fn test_validate_message_content() {
        let ops = MessageOperations::new();

        // Valid content
        assert!(ops.validate_message_content("Hello, world!").is_ok());

        // Empty content
        assert!(ops.validate_message_content("").is_err());
        assert!(ops.validate_message_content("   ").is_err());

        // Too long content
        let long_content = "a".repeat(100_001);
        assert!(ops.validate_message_content(&long_content).is_err());
    }

    #[test]
    fn test_sanitize_message_content() {
        let ops = MessageOperations::new();

        let input = "Hello\x00world\x01with\x02control\x03chars\nand\ttabs";
        let sanitized = ops.sanitize_message_content(input.to_string());
        
        assert_eq!(sanitized, "Helloworldwithcontrolchars\nand\ttabs");
        assert!(!sanitized.contains('\x00'));
        assert!(sanitized.contains('\n'));
        assert!(sanitized.contains('\t'));
    }

    #[test]
    fn test_calculate_message_stats() {
        let ops = MessageOperations::new();
        let message = ops.create_user_message(
            "Hello world!\nThis is a test message with ```code``` and https://example.com".to_string(),
            None,
        );

        let stats = ops.calculate_message_stats(&message);

        assert!(stats.word_count > 0);
        assert!(stats.char_count > 0);
        assert_eq!(stats.line_count, 2);
        assert!(stats.has_code_blocks);
        assert!(stats.has_links);
        assert_eq!(stats.version_count, 0);
    }

    #[test]
    fn test_clear_message_versions() {
        let mut ops = MessageOperations::new();
        let message = ops.create_user_message("Test content".to_string(), None);

        ops.save_message_version(&message);
        assert_eq!(ops.get_message_versions(message.id).len(), 1);

        ops.clear_message_versions(message.id);
        assert_eq!(ops.get_message_versions(message.id).len(), 0);
    }

    #[test]
    fn test_clear_all_versions() {
        let mut ops = MessageOperations::new();
        let message1 = ops.create_user_message("Test 1".to_string(), None);
        let message2 = ops.create_user_message("Test 2".to_string(), None);

        ops.save_message_version(&message1);
        ops.save_message_version(&message2);

        assert_eq!(ops.get_total_version_count(), 2);

        ops.clear_all_versions();
        assert_eq!(ops.get_total_version_count(), 0);
    }

    #[test]
    fn test_get_messages_with_versions() {
        let mut ops = MessageOperations::new();
        let message1 = ops.create_user_message("Test 1".to_string(), None);
        let message2 = ops.create_user_message("Test 2".to_string(), None);

        ops.save_message_version(&message1);
        ops.save_message_version(&message2);

        let messages_with_versions = ops.get_messages_with_versions();
        assert_eq!(messages_with_versions.len(), 2);
        assert!(messages_with_versions.contains(&message1.id));
        assert!(messages_with_versions.contains(&message2.id));
    }
}