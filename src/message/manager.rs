//! Message manager implementation

use std::collections::HashMap;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use arboard::Clipboard;

use crate::events::{EventBus, AppEvent, SessionId, MessageId};
use crate::session::manager::{Message, MessageRole, MessageMetadata};
use crate::models::TokenUsage;
use crate::message::operations::MessageVersion;
use crate::RuffError;

/// Conversation message for API formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Local>,
}

/// Message store for managing message data
#[derive(Debug, Clone)]
pub struct MessageStore {
    /// Messages indexed by session ID and message ID
    messages: HashMap<SessionId, HashMap<MessageId, Message>>,
    /// Message threading relationships
    threads: HashMap<MessageId, Vec<MessageId>>, // parent -> children
    /// Reverse thread lookup
    parents: HashMap<MessageId, MessageId>, // child -> parent
}

impl MessageStore {
    pub fn new() -> Self {
        Self {
            messages: HashMap::new(),
            threads: HashMap::new(),
            parents: HashMap::new(),
        }
    }

    /// Add a message to the store
    pub fn add_message(&mut self, session_id: SessionId, message: Message) {
        let session_messages = self.messages.entry(session_id).or_insert_with(HashMap::new);
        
        // Handle threading relationships
        if let Some(parent_id) = message.parent_id {
            self.parents.insert(message.id, parent_id);
            self.threads.entry(parent_id).or_insert_with(Vec::new).push(message.id);
        }
        
        session_messages.insert(message.id, message);
    }

    /// Get a message by ID
    pub fn get_message(&self, session_id: SessionId, message_id: MessageId) -> Option<&Message> {
        self.messages.get(&session_id)?.get(&message_id)
    }

    /// Get a mutable reference to a message
    pub fn get_message_mut(&mut self, session_id: SessionId, message_id: MessageId) -> Option<&mut Message> {
        self.messages.get_mut(&session_id)?.get_mut(&message_id)
    }

    /// Remove a message from the store
    pub fn remove_message(&mut self, session_id: SessionId, message_id: MessageId) -> Option<Message> {
        let session_messages = self.messages.get_mut(&session_id)?;
        let message = session_messages.remove(&message_id)?;

        // Clean up threading relationships
        if let Some(parent_id) = message.parent_id {
            if let Some(siblings) = self.threads.get_mut(&parent_id) {
                siblings.retain(|&id| id != message_id);
                if siblings.is_empty() {
                    self.threads.remove(&parent_id);
                }
            }
            self.parents.remove(&message_id);
        }

        // Remove any children relationships
        if let Some(children) = self.threads.remove(&message_id) {
            for child_id in children {
                self.parents.remove(&child_id);
                // Update child messages to remove parent reference
                if let Some(child) = self.get_message_mut(session_id, child_id) {
                    child.parent_id = None;
                }
            }
        }

        Some(message)
    }

    /// Get all messages for a session
    pub fn get_session_messages(&self, session_id: SessionId) -> Vec<&Message> {
        self.messages.get(&session_id)
            .map(|messages| messages.values().collect())
            .unwrap_or_default()
    }

    /// Get children of a message
    pub fn get_children(&self, message_id: MessageId) -> Vec<MessageId> {
        self.threads.get(&message_id).cloned().unwrap_or_default()
    }

    /// Get parent of a message
    pub fn get_parent(&self, message_id: MessageId) -> Option<MessageId> {
        self.parents.get(&message_id).copied()
    }

    /// Check if a message exists
    pub fn message_exists(&self, session_id: SessionId, message_id: MessageId) -> bool {
        self.messages.get(&session_id)
            .map(|messages| messages.contains_key(&message_id))
            .unwrap_or(false)
    }

    /// Get message count for a session
    pub fn get_message_count(&self, session_id: SessionId) -> usize {
        self.messages.get(&session_id)
            .map(|messages| messages.len())
            .unwrap_or(0)
    }

    /// Load messages from a session
    pub fn load_session_messages(&mut self, session_id: SessionId, messages: Vec<Message>) {
        let session_messages = self.messages.entry(session_id).or_insert_with(HashMap::new);
        
        for message in messages {
            // Handle threading relationships
            if let Some(parent_id) = message.parent_id {
                self.parents.insert(message.id, parent_id);
                self.threads.entry(parent_id).or_insert_with(Vec::new).push(message.id);
            }
            
            session_messages.insert(message.id, message);
        }
    }

    /// Clear all messages for a session
    pub fn clear_session(&mut self, session_id: SessionId) {
        if let Some(session_messages) = self.messages.remove(&session_id) {
            // Clean up threading relationships
            for message_id in session_messages.keys() {
                self.parents.remove(message_id);
                self.threads.remove(message_id);
            }
        }
    }
}

impl Default for MessageStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Message manager for handling message operations
pub struct MessageManager {
    message_store: MessageStore,
    event_bus: EventBus,
    clipboard: Option<Clipboard>,
}

impl MessageManager {
    pub fn new(event_bus: EventBus) -> Self {
        let clipboard = Clipboard::new().ok();
        if clipboard.is_none() {
            eprintln!("Warning: Failed to initialize clipboard support");
        }

        Self {
            message_store: MessageStore::new(),
            event_bus,
            clipboard,
        }
    }

    /// Add a message to a session
    pub async fn add_message(&mut self, session_id: SessionId, mut message: Message) -> Result<MessageId, RuffError> {
        // Ensure the message has a unique ID
        if message.id == MessageId::nil() {
            message.id = Uuid::new_v4();
        }

        // Set timestamp if not already set
        if message.timestamp.timestamp() == 0 {
            message.timestamp = Local::now();
        }

        let message_id = message.id;
        self.message_store.add_message(session_id, message);

        // Publish event
        self.event_bus.publish(AppEvent::MessageAdded { session_id, message_id }).await
            .map_err(|e| RuffError::App(e.to_string()))?;

        Ok(message_id)
    }

    /// Edit a message's content
    pub async fn edit_message(&mut self, session_id: SessionId, message_id: MessageId, content: String) -> Result<(), RuffError> {
        let message = self.message_store.get_message_mut(session_id, message_id)
            .ok_or_else(|| RuffError::App(format!("Message {} not found in session {}", message_id, session_id)))?;

        // Update content and edit timestamp
        message.content = content;
        message.edited_at = Some(Local::now());

        // Publish event
        self.event_bus.publish(AppEvent::MessageEdited { session_id, message_id }).await
            .map_err(|e| RuffError::App(e.to_string()))?;

        Ok(())
    }

    /// Delete a message and optionally its children
    pub async fn delete_message(&mut self, session_id: SessionId, message_id: MessageId, delete_subsequent: bool) -> Result<(), RuffError> {
        if !self.message_store.message_exists(session_id, message_id) {
            return Err(RuffError::App(format!("Message {} not found in session {}", message_id, session_id)));
        }

        let mut messages_to_delete = vec![message_id];

        if delete_subsequent {
            // Collect all children recursively using iterative approach
            let mut stack = vec![message_id];
            while let Some(current_id) = stack.pop() {
                let children = self.message_store.get_children(current_id);
                for child_id in children {
                    if !messages_to_delete.contains(&child_id) {
                        messages_to_delete.push(child_id);
                        stack.push(child_id);
                    }
                }
            }
        }

        // Remove all messages and publish events
        for msg_id in messages_to_delete {
            self.message_store.remove_message(session_id, msg_id);
            
            // Publish event for each deleted message
            self.event_bus.publish(AppEvent::MessageDeleted { session_id, message_id: msg_id }).await
                .map_err(|e| RuffError::App(e.to_string()))?;
        }

        Ok(())
    }

    /// Copy a message to clipboard
    pub fn copy_message_to_clipboard(&mut self, session_id: SessionId, message_id: MessageId) -> Result<(), RuffError> {
        let message = self.message_store.get_message(session_id, message_id)
            .ok_or_else(|| RuffError::App(format!("Message {} not found in session {}", message_id, session_id)))?;

        if let Some(ref mut clipboard) = self.clipboard {
            clipboard.set_text(&message.content)
                .map_err(|e| RuffError::App(format!("Failed to copy to clipboard: {}", e)))?;
        } else {
            return Err(RuffError::App("Clipboard not available".to_string()));
        }

        Ok(())
    }

    /// Get a message by ID
    pub fn get_message(&self, session_id: SessionId, message_id: MessageId) -> Option<&Message> {
        self.message_store.get_message(session_id, message_id)
    }

    /// Get all messages for a session
    pub fn get_session_messages(&self, session_id: SessionId) -> Vec<&Message> {
        self.message_store.get_session_messages(session_id)
    }

    /// Get children of a message (for threading)
    pub fn get_message_children(&self, message_id: MessageId) -> Vec<MessageId> {
        self.message_store.get_children(message_id)
    }

    /// Get parent of a message (for threading)
    pub fn get_message_parent(&self, message_id: MessageId) -> Option<MessageId> {
        self.message_store.get_parent(message_id)
    }

    /// Create a threaded reply to a message
    pub async fn create_threaded_reply(&mut self, session_id: SessionId, parent_id: MessageId, role: MessageRole, content: String) -> Result<MessageId, RuffError> {
        // Verify parent message exists
        if !self.message_store.message_exists(session_id, parent_id) {
            return Err(RuffError::App(format!("Parent message {} not found in session {}", parent_id, session_id)));
        }

        let message = Message {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: Some(parent_id),
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "unknown".to_string(),
                temperature: 0.7,
                response_time_ms: 0,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };

        self.add_message(session_id, message).await
    }

    /// Load messages from a session (used during initialization)
    pub fn load_session_messages(&mut self, session_id: SessionId, messages: Vec<Message>) {
        self.message_store.load_session_messages(session_id, messages);
    }

    /// Get message count for a session
    pub fn get_message_count(&self, session_id: SessionId) -> usize {
        self.message_store.get_message_count(session_id)
    }

    /// Check if a message exists
    pub fn message_exists(&self, session_id: SessionId, message_id: MessageId) -> bool {
        self.message_store.message_exists(session_id, message_id)
    }

    /// Clear all messages for a session
    pub fn clear_session_messages(&mut self, session_id: SessionId) {
        self.message_store.clear_session(session_id);
    }

    /// Get message thread (all messages in a conversation branch)
    pub fn get_message_thread(&self, session_id: SessionId, message_id: MessageId) -> Vec<&Message> {
        // Collect all messages in thread from root
        fn collect_thread_messages(
            store: &MessageStore,
            session_id: SessionId,
            message_id: MessageId,
            thread: &mut Vec<MessageId>,
        ) {
            thread.push(message_id);
            for child_id in store.get_children(message_id) {
                collect_thread_messages(store, session_id, child_id, thread);
            }
        }

        let mut thread_ids = Vec::new();
        if let Some(root_id) = self.find_thread_root(message_id) {
            collect_thread_messages(&self.message_store, session_id, root_id, &mut thread_ids);
        }

        // Convert IDs to message references
        thread_ids.into_iter()
            .filter_map(|id| self.message_store.get_message(session_id, id))
            .collect()
    }

    /// Find the root message of a thread
    fn find_thread_root(&self, message_id: MessageId) -> Option<MessageId> {
        let mut current_id = message_id;
        
        while let Some(parent_id) = self.message_store.get_parent(current_id) {
            current_id = parent_id;
        }
        
        Some(current_id)
    }

    /// Update message metadata
    pub fn update_message_metadata(&mut self, session_id: SessionId, message_id: MessageId, metadata: MessageMetadata) -> Result<(), RuffError> {
        let message = self.message_store.get_message_mut(session_id, message_id)
            .ok_or_else(|| RuffError::App(format!("Message {} not found in session {}", message_id, session_id)))?;

        message.metadata = metadata;
        Ok(())
    }

    /// Update message token usage
    pub fn update_message_token_usage(&mut self, session_id: SessionId, message_id: MessageId, token_usage: TokenUsage) -> Result<(), RuffError> {
        let message = self.message_store.get_message_mut(session_id, message_id)
            .ok_or_else(|| RuffError::App(format!("Message {} not found in session {}", message_id, session_id)))?;

        message.token_usage = Some(token_usage);
        Ok(())
    }

    /// Regenerate a response message while preserving conversation context
    pub async fn regenerate_response(&mut self, session_id: SessionId, message_id: MessageId, new_content: String, model_used: String, temperature: f32) -> Result<MessageId, RuffError> {
        // Verify the message exists and is an assistant message
        let original_message = self.message_store.get_message(session_id, message_id)
            .ok_or_else(|| RuffError::App(format!("Message {} not found in session {}", message_id, session_id)))?;

        if !matches!(original_message.role, MessageRole::Assistant) {
            return Err(RuffError::App("Can only regenerate assistant messages".to_string()));
        }

        // Create a new regenerated message
        let new_message_id = Uuid::new_v4();
        let parent_id = original_message.parent_id;
        
        let new_message = Message {
            id: new_message_id,
            role: MessageRole::Assistant,
            content: new_content,
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used,
                temperature,
                response_time_ms: 0,
                is_regenerated: true,
                regeneration_count: original_message.metadata.regeneration_count + 1,
            },
        };

        // If the original message had children, we need to handle them
        let original_children = self.message_store.get_children(message_id);
        
        // Remove the original message (this will also handle children relationships)
        self.message_store.remove_message(session_id, message_id);

        // Add the new regenerated message
        self.message_store.add_message(session_id, new_message);

        // Reattach children to the new message if they existed
        for child_id in original_children {
            if let Some(child_message) = self.message_store.get_message_mut(session_id, child_id) {
                child_message.parent_id = Some(new_message_id);
                // Update the threading relationships
                self.message_store.parents.insert(child_id, new_message_id);
                self.message_store.threads.entry(new_message_id).or_insert_with(Vec::new).push(child_id);
            }
        }

        // Publish regeneration event
        self.event_bus.publish(AppEvent::MessageRegenerated { 
            session_id, 
            old_message_id: message_id, 
            new_message_id 
        }).await.map_err(|e| RuffError::App(e.to_string()))?;

        Ok(new_message_id)
    }

    /// Create a new version of a message (for tracking regenerations)
    pub fn create_message_version(&self, message: &Message) -> MessageVersion {
        MessageVersion {
            id: message.id,
            version: message.metadata.regeneration_count + 1,
            content: message.content.clone(),
            timestamp: message.timestamp,
            metadata: message.metadata.clone(),
            token_usage: message.token_usage.clone(),
        }
    }

    /// Check if a message has been regenerated
    pub fn is_message_regenerated(&self, session_id: SessionId, message_id: MessageId) -> bool {
        self.message_store.get_message(session_id, message_id)
            .map(|msg| msg.metadata.is_regenerated)
            .unwrap_or(false)
    }

    /// Get regeneration count for a message
    pub fn get_regeneration_count(&self, session_id: SessionId, message_id: MessageId) -> u32 {
        self.message_store.get_message(session_id, message_id)
            .map(|msg| msg.metadata.regeneration_count)
            .unwrap_or(0)
    }

    /// Mark a message as regenerated (used when loading from storage)
    pub fn mark_message_as_regenerated(&mut self, session_id: SessionId, message_id: MessageId, regeneration_count: u32) -> Result<(), RuffError> {
        let message = self.message_store.get_message_mut(session_id, message_id)
            .ok_or_else(|| RuffError::App(format!("Message {} not found in session {}", message_id, session_id)))?;

        message.metadata.is_regenerated = true;
        message.metadata.regeneration_count = regeneration_count;
        Ok(())
    }

    /// Get all regenerated messages in a session
    pub fn get_regenerated_messages(&self, session_id: SessionId) -> Vec<&Message> {
        self.message_store.get_session_messages(session_id)
            .into_iter()
            .filter(|msg| msg.metadata.is_regenerated)
            .collect()
    }

    /// Get conversation context for regeneration (messages leading up to the target message)
    pub fn get_conversation_context(&self, session_id: SessionId, target_message_id: MessageId, context_limit: usize) -> Vec<&Message> {
        let all_messages = self.message_store.get_session_messages(session_id);
        
        // Sort messages by timestamp to get chronological order
        let mut sorted_messages = all_messages;
        sorted_messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // Find the target message position
        let target_position = sorted_messages.iter()
            .position(|msg| msg.id == target_message_id);

        if let Some(pos) = target_position {
            // Get messages before the target message, limited by context_limit
            let start_pos = if pos >= context_limit { pos - context_limit } else { 0 };
            sorted_messages[start_pos..pos].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Prepare conversation context for API call (format for regeneration)
    pub fn format_conversation_context(&self, context_messages: Vec<&Message>) -> Vec<ConversationMessage> {
        context_messages.into_iter()
            .map(|msg| ConversationMessage {
                role: match msg.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::System => "system".to_string(),
                },
                content: msg.content.clone(),
                timestamp: msg.timestamp,
            })
            .collect()
    }
}
#[cfg(test
)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{sleep, Duration};

    async fn create_test_message_manager() -> MessageManager {
        let event_bus = EventBus::new();
        MessageManager::new(event_bus)
    }

    fn create_test_message(role: MessageRole, content: &str) -> Message {
        Message {
            id: Uuid::new_v4(),
            role,
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
    async fn test_message_manager_creation() {
        let manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        assert_eq!(manager.get_message_count(session_id), 0);
        assert!(manager.get_session_messages(session_id).is_empty());
    }

    #[tokio::test]
    async fn test_add_message() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let message = create_test_message(MessageRole::User, "Hello, world!");

        let message_id = manager.add_message(session_id, message.clone()).await.unwrap();

        assert_eq!(manager.get_message_count(session_id), 1);
        assert!(manager.message_exists(session_id, message_id));
        
        let retrieved_message = manager.get_message(session_id, message_id).unwrap();
        assert_eq!(retrieved_message.content, "Hello, world!");
        assert_eq!(retrieved_message.role, MessageRole::User);
    }

    #[tokio::test]
    async fn test_edit_message() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let message = create_test_message(MessageRole::User, "Original content");

        let message_id = manager.add_message(session_id, message).await.unwrap();
        
        // Edit the message
        manager.edit_message(session_id, message_id, "Edited content".to_string()).await.unwrap();

        let edited_message = manager.get_message(session_id, message_id).unwrap();
        assert_eq!(edited_message.content, "Edited content");
        assert!(edited_message.edited_at.is_some());
    }

    #[tokio::test]
    async fn test_edit_nonexistent_message() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let nonexistent_id = Uuid::new_v4();

        let result = manager.edit_message(session_id, nonexistent_id, "New content".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_delete_message() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let message = create_test_message(MessageRole::User, "To be deleted");

        let message_id = manager.add_message(session_id, message).await.unwrap();
        assert!(manager.message_exists(session_id, message_id));

        // Delete the message
        manager.delete_message(session_id, message_id, false).await.unwrap();

        assert!(!manager.message_exists(session_id, message_id));
        assert_eq!(manager.get_message_count(session_id), 0);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_message() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let nonexistent_id = Uuid::new_v4();

        let result = manager.delete_message(session_id, nonexistent_id, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_message_threading() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Create parent message
        let parent_message = create_test_message(MessageRole::User, "Parent message");
        let parent_id = manager.add_message(session_id, parent_message).await.unwrap();

        // Create threaded reply
        let child_id = manager.create_threaded_reply(
            session_id,
            parent_id,
            MessageRole::Assistant,
            "Child reply".to_string()
        ).await.unwrap();

        // Verify threading relationships
        let children = manager.get_message_children(parent_id);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], child_id);

        let parent = manager.get_message_parent(child_id);
        assert_eq!(parent, Some(parent_id));

        // Verify child message content
        let child_message = manager.get_message(session_id, child_id).unwrap();
        assert_eq!(child_message.content, "Child reply");
        assert_eq!(child_message.parent_id, Some(parent_id));
    }

    #[tokio::test]
    async fn test_create_threaded_reply_nonexistent_parent() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let nonexistent_parent = Uuid::new_v4();

        let result = manager.create_threaded_reply(
            session_id,
            nonexistent_parent,
            MessageRole::Assistant,
            "Reply".to_string()
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_delete_message_with_children() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Create parent message
        let parent_message = create_test_message(MessageRole::User, "Parent");
        let parent_id = manager.add_message(session_id, parent_message).await.unwrap();

        // Create child messages
        let child1_id = manager.create_threaded_reply(
            session_id, parent_id, MessageRole::Assistant, "Child 1".to_string()
        ).await.unwrap();
        
        let child2_id = manager.create_threaded_reply(
            session_id, parent_id, MessageRole::Assistant, "Child 2".to_string()
        ).await.unwrap();

        assert_eq!(manager.get_message_count(session_id), 3);

        // Delete parent with children
        manager.delete_message(session_id, parent_id, true).await.unwrap();

        // All messages should be deleted
        assert_eq!(manager.get_message_count(session_id), 0);
        assert!(!manager.message_exists(session_id, parent_id));
        assert!(!manager.message_exists(session_id, child1_id));
        assert!(!manager.message_exists(session_id, child2_id));
    }

    #[tokio::test]
    async fn test_delete_message_without_children() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Create parent message
        let parent_message = create_test_message(MessageRole::User, "Parent");
        let parent_id = manager.add_message(session_id, parent_message).await.unwrap();

        // Create child message
        let child_id = manager.create_threaded_reply(
            session_id, parent_id, MessageRole::Assistant, "Child".to_string()
        ).await.unwrap();

        assert_eq!(manager.get_message_count(session_id), 2);

        // Delete parent without children
        manager.delete_message(session_id, parent_id, false).await.unwrap();

        // Only parent should be deleted, child should remain but lose parent reference
        assert_eq!(manager.get_message_count(session_id), 1);
        assert!(!manager.message_exists(session_id, parent_id));
        assert!(manager.message_exists(session_id, child_id));
        
        let child_message = manager.get_message(session_id, child_id).unwrap();
        assert_eq!(child_message.parent_id, None);
    }

    #[tokio::test]
    async fn test_get_session_messages() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Add multiple messages
        let message1 = create_test_message(MessageRole::User, "Message 1");
        let message2 = create_test_message(MessageRole::Assistant, "Message 2");
        let message3 = create_test_message(MessageRole::User, "Message 3");

        manager.add_message(session_id, message1).await.unwrap();
        manager.add_message(session_id, message2).await.unwrap();
        manager.add_message(session_id, message3).await.unwrap();

        let messages = manager.get_session_messages(session_id);
        assert_eq!(messages.len(), 3);
        
        // Check that all messages are present
        let contents: Vec<&str> = messages.iter().map(|m| m.content.as_str()).collect();
        assert!(contents.contains(&"Message 1"));
        assert!(contents.contains(&"Message 2"));
        assert!(contents.contains(&"Message 3"));
    }

    #[tokio::test]
    async fn test_load_session_messages() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Create messages to load
        let messages = vec![
            create_test_message(MessageRole::User, "Loaded message 1"),
            create_test_message(MessageRole::Assistant, "Loaded message 2"),
        ];

        manager.load_session_messages(session_id, messages);

        assert_eq!(manager.get_message_count(session_id), 2);
        let loaded_messages = manager.get_session_messages(session_id);
        assert_eq!(loaded_messages.len(), 2);
    }

    #[tokio::test]
    async fn test_clear_session_messages() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Add messages
        let message1 = create_test_message(MessageRole::User, "Message 1");
        let message2 = create_test_message(MessageRole::Assistant, "Message 2");
        
        manager.add_message(session_id, message1).await.unwrap();
        manager.add_message(session_id, message2).await.unwrap();
        
        assert_eq!(manager.get_message_count(session_id), 2);

        // Clear messages
        manager.clear_session_messages(session_id);

        assert_eq!(manager.get_message_count(session_id), 0);
        assert!(manager.get_session_messages(session_id).is_empty());
    }

    #[tokio::test]
    async fn test_update_message_metadata() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let message = create_test_message(MessageRole::User, "Test message");

        let message_id = manager.add_message(session_id, message).await.unwrap();

        let new_metadata = MessageMetadata {
            model_used: "updated-model".to_string(),
            temperature: 0.9,
            response_time_ms: 200,
            is_regenerated: true,
            regeneration_count: 1,
        };

        manager.update_message_metadata(session_id, message_id, new_metadata.clone()).unwrap();

        let updated_message = manager.get_message(session_id, message_id).unwrap();
        assert_eq!(updated_message.metadata.model_used, "updated-model");
        assert_eq!(updated_message.metadata.temperature, 0.9);
        assert_eq!(updated_message.metadata.response_time_ms, 200);
        assert!(updated_message.metadata.is_regenerated);
        assert_eq!(updated_message.metadata.regeneration_count, 1);
    }

    #[tokio::test]
    async fn test_update_message_token_usage() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let message = create_test_message(MessageRole::User, "Test message");

        let message_id = manager.add_message(session_id, message).await.unwrap();

        let new_token_usage = TokenUsage {
            input_tokens: 50,
            output_tokens: 100,
            total_tokens: 150,
        };

        manager.update_message_token_usage(session_id, message_id, new_token_usage.clone()).unwrap();

        let updated_message = manager.get_message(session_id, message_id).unwrap();
        let token_usage = updated_message.token_usage.as_ref().unwrap();
        assert_eq!(token_usage.input_tokens, 50);
        assert_eq!(token_usage.output_tokens, 100);
        assert_eq!(token_usage.total_tokens, 150);
    }

    #[tokio::test]
    async fn test_get_message_thread() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Create a thread: root -> child1 -> grandchild
        let root_message = create_test_message(MessageRole::User, "Root");
        let root_id = manager.add_message(session_id, root_message).await.unwrap();

        let child1_id = manager.create_threaded_reply(
            session_id, root_id, MessageRole::Assistant, "Child 1".to_string()
        ).await.unwrap();

        let grandchild_id = manager.create_threaded_reply(
            session_id, child1_id, MessageRole::User, "Grandchild".to_string()
        ).await.unwrap();

        // Get thread from any message in the thread
        let thread_from_root = manager.get_message_thread(session_id, root_id);
        let thread_from_child = manager.get_message_thread(session_id, child1_id);
        let thread_from_grandchild = manager.get_message_thread(session_id, grandchild_id);

        // All should return the same thread
        assert_eq!(thread_from_root.len(), 3);
        assert_eq!(thread_from_child.len(), 3);
        assert_eq!(thread_from_grandchild.len(), 3);
    }

    #[tokio::test]
    async fn test_event_publishing() {
        let event_bus = EventBus::new();
        let event_counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&event_counter);

        // Subscribe to events
        let _handle = event_bus.subscribe(move |event| {
            match event {
                AppEvent::MessageAdded { .. } |
                AppEvent::MessageEdited { .. } |
                AppEvent::MessageDeleted { .. } => {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
            Ok(())
        }).await;

        let mut manager = MessageManager::new(event_bus);
        let session_id = Uuid::new_v4();
        let message = create_test_message(MessageRole::User, "Test message");

        // Add message (should trigger event)
        let message_id = manager.add_message(session_id, message).await.unwrap();
        
        // Edit message (should trigger event)
        manager.edit_message(session_id, message_id, "Edited".to_string()).await.unwrap();
        
        // Delete message (should trigger event)
        manager.delete_message(session_id, message_id, false).await.unwrap();

        // Give time for event processing
        sleep(Duration::from_millis(10)).await;

        // Should have received 3 events
        assert_eq!(event_counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_copy_message_to_clipboard() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let message = create_test_message(MessageRole::User, "Copy this text");

        let message_id = manager.add_message(session_id, message).await.unwrap();

        // Note: This test might fail in headless environments without clipboard support
        // The actual clipboard functionality is tested by the presence of the clipboard field
        let result = manager.copy_message_to_clipboard(session_id, message_id);
        
        // The result depends on whether clipboard is available in the test environment
        // We just verify that the method handles the case appropriately
        match result {
            Ok(()) => {
                // Clipboard was available and copy succeeded
            }
            Err(e) => {
                // Clipboard not available or copy failed
                assert!(e.to_string().contains("clipboard") || e.to_string().contains("Clipboard"));
            }
        }
    }

    #[tokio::test]
    async fn test_copy_nonexistent_message() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let nonexistent_id = Uuid::new_v4();

        let result = manager.copy_message_to_clipboard(session_id, nonexistent_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_regenerate_response() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Create a user message and an assistant response
        let user_message = create_test_message(MessageRole::User, "What is AI?");
        let user_id = manager.add_message(session_id, user_message).await.unwrap();

        let mut assistant_message = create_test_message(MessageRole::Assistant, "AI is artificial intelligence");
        assistant_message.parent_id = Some(user_id);
        let assistant_id = manager.add_message(session_id, assistant_message).await.unwrap();

        // Regenerate the assistant response
        let new_id = manager.regenerate_response(
            session_id,
            assistant_id,
            "AI stands for Artificial Intelligence, a field of computer science".to_string(),
            "gpt-4".to_string(),
            0.8
        ).await.unwrap();

        // Verify the new message
        let new_message = manager.get_message(session_id, new_id).unwrap();
        assert_eq!(new_message.content, "AI stands for Artificial Intelligence, a field of computer science");
        assert_eq!(new_message.parent_id, Some(user_id));
        assert!(new_message.metadata.is_regenerated);
        assert_eq!(new_message.metadata.regeneration_count, 1);
        assert_eq!(new_message.metadata.model_used, "gpt-4");
        assert_eq!(new_message.metadata.temperature, 0.8);

        // Verify the original message is gone
        assert!(!manager.message_exists(session_id, assistant_id));

        // Verify message count is still correct
        assert_eq!(manager.get_message_count(session_id), 2);
    }

    #[tokio::test]
    async fn test_regenerate_user_message_fails() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        let user_message = create_test_message(MessageRole::User, "Hello");
        let user_id = manager.add_message(session_id, user_message).await.unwrap();

        // Try to regenerate a user message (should fail)
        let result = manager.regenerate_response(
            session_id,
            user_id,
            "Hi there".to_string(),
            "gpt-4".to_string(),
            0.7
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Can only regenerate assistant messages"));
    }

    #[tokio::test]
    async fn test_regenerate_nonexistent_message() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        let nonexistent_id = Uuid::new_v4();

        let result = manager.regenerate_response(
            session_id,
            nonexistent_id,
            "New content".to_string(),
            "gpt-4".to_string(),
            0.7
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_regenerate_with_children() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Create a conversation chain
        let user_message = create_test_message(MessageRole::User, "Tell me about cats");
        let user_id = manager.add_message(session_id, user_message).await.unwrap();

        let mut assistant_message = create_test_message(MessageRole::Assistant, "Cats are pets");
        assistant_message.parent_id = Some(user_id);
        let assistant_id = manager.add_message(session_id, assistant_message).await.unwrap();

        let follow_up_id = manager.create_threaded_reply(
            session_id,
            assistant_id,
            MessageRole::User,
            "Tell me more".to_string()
        ).await.unwrap();

        // Regenerate the assistant message
        let new_assistant_id = manager.regenerate_response(
            session_id,
            assistant_id,
            "Cats are fascinating feline creatures".to_string(),
            "gpt-4".to_string(),
            0.7
        ).await.unwrap();

        // Verify the follow-up message is now a child of the new assistant message
        let follow_up_message = manager.get_message(session_id, follow_up_id).unwrap();
        assert_eq!(follow_up_message.parent_id, Some(new_assistant_id));

        // Verify threading relationships
        let children = manager.get_message_children(new_assistant_id);
        assert_eq!(children, vec![follow_up_id]);
    }

    #[tokio::test]
    async fn test_is_message_regenerated() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        let mut assistant_message = create_test_message(MessageRole::Assistant, "Original response");
        assistant_message.metadata.is_regenerated = true;
        assistant_message.metadata.regeneration_count = 2;
        let assistant_id = manager.add_message(session_id, assistant_message).await.unwrap();

        assert!(manager.is_message_regenerated(session_id, assistant_id));
        assert_eq!(manager.get_regeneration_count(session_id, assistant_id), 2);
    }

    #[tokio::test]
    async fn test_mark_message_as_regenerated() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        let assistant_message = create_test_message(MessageRole::Assistant, "Response");
        let assistant_id = manager.add_message(session_id, assistant_message).await.unwrap();

        // Initially not regenerated
        assert!(!manager.is_message_regenerated(session_id, assistant_id));
        assert_eq!(manager.get_regeneration_count(session_id, assistant_id), 0);

        // Mark as regenerated
        manager.mark_message_as_regenerated(session_id, assistant_id, 3).unwrap();

        // Verify it's now marked as regenerated
        assert!(manager.is_message_regenerated(session_id, assistant_id));
        assert_eq!(manager.get_regeneration_count(session_id, assistant_id), 3);
    }

    #[tokio::test]
    async fn test_get_regenerated_messages() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Add regular message
        let user_message = create_test_message(MessageRole::User, "Question");
        manager.add_message(session_id, user_message).await.unwrap();

        // Add regenerated message
        let mut regenerated_message = create_test_message(MessageRole::Assistant, "Regenerated response");
        regenerated_message.metadata.is_regenerated = true;
        regenerated_message.metadata.regeneration_count = 1;
        manager.add_message(session_id, regenerated_message).await.unwrap();

        // Add another regenerated message
        let mut another_regenerated = create_test_message(MessageRole::Assistant, "Another regenerated");
        another_regenerated.metadata.is_regenerated = true;
        another_regenerated.metadata.regeneration_count = 2;
        manager.add_message(session_id, another_regenerated).await.unwrap();

        let regenerated_messages = manager.get_regenerated_messages(session_id);
        assert_eq!(regenerated_messages.len(), 2);
        
        for msg in regenerated_messages {
            assert!(msg.metadata.is_regenerated);
            assert!(msg.metadata.regeneration_count > 0);
        }
    }

    #[tokio::test]
    async fn test_get_conversation_context() {
        let mut manager = create_test_message_manager().await;
        let session_id = Uuid::new_v4();
        
        // Create a conversation with multiple messages
        let mut message_ids = Vec::new();
        for i in 0..5 {
            let role = if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant };
            let message = create_test_message(role, &format!("Message {}", i));
            let id = manager.add_message(session_id, message).await.unwrap();
            message_ids.push(id);
        }

        // Get context for the last message with limit of 3
        let context = manager.get_conversation_context(session_id, message_ids[4], 3);
        assert_eq!(context.len(), 3);
        
        // Should get messages 1, 2, 3 (before message 4)
        let context_contents: Vec<&str> = context.iter().map(|m| m.content.as_str()).collect();
        assert!(context_contents.contains(&"Message 1"));
        assert!(context_contents.contains(&"Message 2"));
        assert!(context_contents.contains(&"Message 3"));
    }

    #[tokio::test]
    async fn test_format_conversation_context() {
        let manager = create_test_message_manager().await;
        
        let user_message = create_test_message(MessageRole::User, "Hello");
        let assistant_message = create_test_message(MessageRole::Assistant, "Hi there");
        let system_message = create_test_message(MessageRole::System, "You are helpful");

        let context_messages = vec![&system_message, &user_message, &assistant_message];
        let formatted = manager.format_conversation_context(context_messages);

        assert_eq!(formatted.len(), 3);
        assert_eq!(formatted[0].role, "system");
        assert_eq!(formatted[0].content, "You are helpful");
        assert_eq!(formatted[1].role, "user");
        assert_eq!(formatted[1].content, "Hello");
        assert_eq!(formatted[2].role, "assistant");
        assert_eq!(formatted[2].content, "Hi there");
    }

    #[tokio::test]
    async fn test_create_message_version() {
        let manager = create_test_message_manager().await;
        
        let mut message = create_test_message(MessageRole::Assistant, "Test content");
        message.metadata.regeneration_count = 2;

        let version = manager.create_message_version(&message);
        assert_eq!(version.id, message.id);
        assert_eq!(version.version, 3); // regeneration_count + 1
        assert_eq!(version.content, "Test content");
        assert_eq!(version.metadata.regeneration_count, 2);
    }

    #[tokio::test]
    async fn test_regeneration_event_publishing() {
        let event_bus = EventBus::new();
        let event_counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&event_counter);

        // Subscribe to regeneration events
        let _handle = event_bus.subscribe(move |event| {
            if let AppEvent::MessageRegenerated { .. } = event {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }).await;

        let mut manager = MessageManager::new(event_bus);
        let session_id = Uuid::new_v4();
        
        // Create and regenerate a message
        let mut assistant_message = create_test_message(MessageRole::Assistant, "Original");
        let assistant_id = manager.add_message(session_id, assistant_message).await.unwrap();

        manager.regenerate_response(
            session_id,
            assistant_id,
            "Regenerated".to_string(),
            "gpt-4".to_string(),
            0.7
        ).await.unwrap();

        // Give time for event processing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Should have received 1 regeneration event
        assert_eq!(event_counter.load(Ordering::SeqCst), 1);
    }
}