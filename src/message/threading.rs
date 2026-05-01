//! Message threading support

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::events::MessageId;
use crate::session::manager::Message;
use crate::EnhancedError;

/// Message thread representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageThread {
    pub root_message_id: MessageId,
    pub messages: Vec<MessageId>,
    pub depth: usize,
    pub branch_count: usize,
}

/// Thread relationship information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRelationship {
    pub parent: Option<MessageId>,
    pub children: Vec<MessageId>,
    pub siblings: Vec<MessageId>,
    pub depth: usize,
}

/// Thread statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadStatistics {
    pub total_threads: usize,
    pub max_depth: usize,
    pub average_depth: f64,
    pub total_messages: usize,
    pub branching_factor: f64,
}

/// Thread manager for handling message threading operations
pub struct ThreadManager {
    /// Thread relationships: message_id -> thread info
    threads: HashMap<MessageId, MessageThread>,
    /// Parent-child relationships: parent_id -> children_ids
    children: HashMap<MessageId, Vec<MessageId>>,
    /// Child-parent relationships: child_id -> parent_id
    parents: HashMap<MessageId, MessageId>,
}

impl ThreadManager {
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
            children: HashMap::new(),
            parents: HashMap::new(),
        }
    }

    /// Build thread relationships from a collection of messages
    pub fn build_threads(&mut self, messages: &[Message]) {
        self.clear();

        // First pass: build parent-child relationships
        for message in messages {
            if let Some(parent_id) = message.parent_id {
                self.parents.insert(message.id, parent_id);
                self.children
                    .entry(parent_id)
                    .or_insert_with(Vec::new)
                    .push(message.id);
            }
        }

        // Second pass: build thread structures
        let mut processed = HashSet::new();
        for message in messages {
            if !processed.contains(&message.id) && message.parent_id.is_none() {
                // This is a root message, build the thread
                let thread = self.build_thread_from_root(message.id, messages);
                for &msg_id in &thread.messages {
                    processed.insert(msg_id);
                }
                self.threads.insert(message.id, thread);
            }
        }
    }

    /// Build a thread starting from a root message
    fn build_thread_from_root(&self, root_id: MessageId, _messages: &[Message]) -> MessageThread {
        let mut thread_messages = Vec::new();
        let mut max_depth = 0;
        let mut branch_count = 0;

        self.collect_thread_messages(
            root_id,
            &mut thread_messages,
            &mut max_depth,
            &mut branch_count,
            0,
        );

        MessageThread {
            root_message_id: root_id,
            messages: thread_messages,
            depth: max_depth,
            branch_count,
        }
    }

    /// Recursively collect all messages in a thread
    fn collect_thread_messages(
        &self,
        message_id: MessageId,
        thread_messages: &mut Vec<MessageId>,
        max_depth: &mut usize,
        branch_count: &mut usize,
        current_depth: usize,
    ) {
        thread_messages.push(message_id);
        *max_depth = (*max_depth).max(current_depth);

        if let Some(children) = self.children.get(&message_id) {
            if children.len() > 1 {
                *branch_count += 1;
            }

            for &child_id in children {
                self.collect_thread_messages(
                    child_id,
                    thread_messages,
                    max_depth,
                    branch_count,
                    current_depth + 1,
                );
            }
        }
    }

    /// Get the thread containing a specific message
    pub fn get_thread_for_message(&self, message_id: MessageId) -> Option<&MessageThread> {
        // Find the root of this message's thread
        let root_id = self.find_thread_root(message_id)?;
        self.threads.get(&root_id)
    }

    /// Find the root message of a thread
    pub fn find_thread_root(&self, message_id: MessageId) -> Option<MessageId> {
        let mut current_id = message_id;

        // Traverse up to find the root
        while let Some(&parent_id) = self.parents.get(&current_id) {
            current_id = parent_id;
        }

        Some(current_id)
    }

    /// Get all children of a message
    pub fn get_children(&self, message_id: MessageId) -> Vec<MessageId> {
        self.children.get(&message_id).cloned().unwrap_or_default()
    }

    /// Get the parent of a message
    pub fn get_parent(&self, message_id: MessageId) -> Option<MessageId> {
        self.parents.get(&message_id).copied()
    }

    /// Get all siblings of a message (messages with the same parent)
    pub fn get_siblings(&self, message_id: MessageId) -> Vec<MessageId> {
        if let Some(parent_id) = self.get_parent(message_id) {
            self.get_children(parent_id)
                .into_iter()
                .filter(|&id| id != message_id)
                .collect()
        } else {
            // Root messages - find other root messages in the same session
            Vec::new()
        }
    }

    /// Get the depth of a message in its thread (0 for root)
    pub fn get_message_depth(&self, message_id: MessageId) -> usize {
        let mut depth = 0;
        let mut current_id = message_id;

        while let Some(parent_id) = self.get_parent(current_id) {
            depth += 1;
            current_id = parent_id;
        }

        depth
    }

    /// Get thread relationship information for a message
    pub fn get_thread_relationship(&self, message_id: MessageId) -> ThreadRelationship {
        ThreadRelationship {
            parent: self.get_parent(message_id),
            children: self.get_children(message_id),
            siblings: self.get_siblings(message_id),
            depth: self.get_message_depth(message_id),
        }
    }

    /// Get all thread roots
    pub fn get_thread_roots(&self) -> Vec<MessageId> {
        self.threads.keys().cloned().collect()
    }

    /// Get all threads
    pub fn get_all_threads(&self) -> Vec<&MessageThread> {
        self.threads.values().collect()
    }

    /// Check if a message is a root message
    pub fn is_root_message(&self, message_id: MessageId) -> bool {
        !self.parents.contains_key(&message_id)
    }

    /// Check if a message is a leaf message (has no children)
    pub fn is_leaf_message(&self, message_id: MessageId) -> bool {
        self.children
            .get(&message_id)
            .map_or(true, |children| children.is_empty())
    }

    /// Get the path from root to a specific message
    pub fn get_path_to_message(&self, message_id: MessageId) -> Vec<MessageId> {
        let mut path = Vec::new();
        let mut current_id = message_id;

        // Build path from message to root
        path.push(current_id);
        while let Some(parent_id) = self.get_parent(current_id) {
            path.push(parent_id);
            current_id = parent_id;
        }

        // Reverse to get path from root to message
        path.reverse();
        path
    }

    /// Add a new message to the threading system
    pub fn add_message(&mut self, message: &Message) -> Result<(), EnhancedError> {
        if let Some(parent_id) = message.parent_id {
            // Verify parent exists
            if !self.parents.contains_key(&parent_id) && !self.is_root_message(parent_id) {
                return Err(EnhancedError::unknown(format!(
                    "Parent message {} not found",
                    parent_id
                )));
            }

            self.parents.insert(message.id, parent_id);
            self.children
                .entry(parent_id)
                .or_insert_with(Vec::new)
                .push(message.id);

            // Update the thread that contains this parent
            if let Some(root_id) = self.find_thread_root(parent_id) {
                // Calculate values before borrowing mutably
                let depth = self.get_message_depth(message.id);
                let children_count = self.get_children(parent_id).len();

                if let Some(thread) = self.threads.get_mut(&root_id) {
                    thread.messages.push(message.id);
                    thread.depth = thread.depth.max(depth);

                    // Check if parent now has multiple children (new branch)
                    if children_count > 1 {
                        thread.branch_count += 1;
                    }
                }
            }
        } else {
            // This is a new root message, create a new thread
            let thread = MessageThread {
                root_message_id: message.id,
                messages: vec![message.id],
                depth: 0,
                branch_count: 0,
            };
            self.threads.insert(message.id, thread);
        }

        Ok(())
    }

    /// Remove a message from the threading system
    pub fn remove_message(
        &mut self,
        message_id: MessageId,
        remove_children: bool,
    ) -> Result<Vec<MessageId>, EnhancedError> {
        let mut removed_messages = Vec::new();

        if remove_children {
            // Remove all children recursively
            let children = self.get_children(message_id);
            for child_id in children {
                let child_removed = self.remove_message(child_id, true)?;
                removed_messages.extend(child_removed);
            }
        } else {
            // Reparent children to this message's parent
            let children = self.get_children(message_id);
            let parent_id = self.get_parent(message_id);

            for child_id in children {
                if let Some(parent) = parent_id {
                    self.parents.insert(child_id, parent);
                    self.children
                        .entry(parent)
                        .or_insert_with(Vec::new)
                        .push(child_id);
                } else {
                    // Child becomes a new root
                    self.parents.remove(&child_id);
                    let thread = MessageThread {
                        root_message_id: child_id,
                        messages: vec![child_id], // Will be rebuilt if needed
                        depth: 0,
                        branch_count: 0,
                    };
                    self.threads.insert(child_id, thread);
                }
            }
        }

        // Remove the message itself
        if let Some(parent_id) = self.parents.remove(&message_id) {
            if let Some(siblings) = self.children.get_mut(&parent_id) {
                siblings.retain(|&id| id != message_id);
                if siblings.is_empty() {
                    self.children.remove(&parent_id);
                }
            }
        }

        self.children.remove(&message_id);

        // If this was a root message, remove its thread
        if self.threads.contains_key(&message_id) {
            self.threads.remove(&message_id);
        } else {
            // Update the thread that contained this message
            if let Some(root_id) = self.find_thread_root(message_id) {
                if let Some(thread) = self.threads.get_mut(&root_id) {
                    thread.messages.retain(|&id| id != message_id);
                }
            }
        }

        removed_messages.push(message_id);
        Ok(removed_messages)
    }

    /// Calculate threading statistics
    pub fn calculate_statistics(&self) -> ThreadStatistics {
        let total_threads = self.threads.len();
        let total_messages: usize = self.threads.values().map(|t| t.messages.len()).sum();
        let max_depth = self.threads.values().map(|t| t.depth).max().unwrap_or(0);
        let average_depth = if total_threads > 0 {
            self.threads.values().map(|t| t.depth as f64).sum::<f64>() / total_threads as f64
        } else {
            0.0
        };

        let total_branches: usize = self.threads.values().map(|t| t.branch_count).sum();
        let branching_factor = if total_threads > 0 {
            total_branches as f64 / total_threads as f64
        } else {
            0.0
        };

        ThreadStatistics {
            total_threads,
            max_depth,
            average_depth,
            total_messages,
            branching_factor,
        }
    }

    /// Clear all threading data
    pub fn clear(&mut self) {
        self.threads.clear();
        self.children.clear();
        self.parents.clear();
    }

    /// Get messages at a specific depth in a thread
    pub fn get_messages_at_depth(&self, root_id: MessageId, depth: usize) -> Vec<MessageId> {
        let mut messages_at_depth = Vec::new();
        self.collect_messages_at_depth(root_id, depth, 0, &mut messages_at_depth);
        messages_at_depth
    }

    /// Recursively collect messages at a specific depth
    fn collect_messages_at_depth(
        &self,
        message_id: MessageId,
        target_depth: usize,
        current_depth: usize,
        result: &mut Vec<MessageId>,
    ) {
        if current_depth == target_depth {
            result.push(message_id);
            return;
        }

        if let Some(children) = self.children.get(&message_id) {
            for &child_id in children {
                self.collect_messages_at_depth(child_id, target_depth, current_depth + 1, result);
            }
        }
    }

    /// Get the conversation flow (linear path through the thread)
    pub fn get_conversation_flow(&self, root_id: MessageId) -> Vec<MessageId> {
        let mut flow = Vec::new();
        let mut current_id = root_id;

        loop {
            flow.push(current_id);

            // Follow the first child (main conversation path)
            if let Some(children) = self.children.get(&current_id) {
                if let Some(&first_child) = children.first() {
                    current_id = first_child;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        flow
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::manager::{MessageMetadata, MessageRole};

    use chrono::Local;
    use uuid::Uuid;

    fn create_test_message(id: MessageId, parent_id: Option<MessageId>, content: &str) -> Message {
        Message {
            id,
            role: MessageRole::User,
            content: content.to_string(),
            timestamp: Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        }
    }

    #[test]
    fn test_thread_manager_creation() {
        let manager = ThreadManager::new();
        assert_eq!(manager.get_all_threads().len(), 0);
        assert_eq!(manager.get_thread_roots().len(), 0);
    }

    #[test]
    fn test_build_simple_thread() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root message"),
            create_test_message(child_id, Some(root_id), "Child message"),
        ];

        manager.build_threads(&messages);

        assert_eq!(manager.get_thread_roots().len(), 1);
        assert!(manager.get_thread_roots().contains(&root_id));

        let thread = manager.get_thread_for_message(root_id).unwrap();
        assert_eq!(thread.root_message_id, root_id);
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.depth, 1);

        assert_eq!(manager.get_parent(child_id), Some(root_id));
        assert_eq!(manager.get_children(root_id), vec![child_id]);
    }

    #[test]
    fn test_build_branching_thread() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child1_id = Uuid::new_v4();
        let child2_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root"),
            create_test_message(child1_id, Some(root_id), "Child 1"),
            create_test_message(child2_id, Some(root_id), "Child 2"),
            create_test_message(grandchild_id, Some(child1_id), "Grandchild"),
        ];

        manager.build_threads(&messages);

        let thread = manager.get_thread_for_message(root_id).unwrap();
        assert_eq!(thread.messages.len(), 4);
        assert_eq!(thread.depth, 2);
        assert_eq!(thread.branch_count, 1); // One branch at root level

        let children = manager.get_children(root_id);
        assert_eq!(children.len(), 2);
        assert!(children.contains(&child1_id));
        assert!(children.contains(&child2_id));
    }

    #[test]
    fn test_get_message_depth() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root"),
            create_test_message(child_id, Some(root_id), "Child"),
            create_test_message(grandchild_id, Some(child_id), "Grandchild"),
        ];

        manager.build_threads(&messages);

        assert_eq!(manager.get_message_depth(root_id), 0);
        assert_eq!(manager.get_message_depth(child_id), 1);
        assert_eq!(manager.get_message_depth(grandchild_id), 2);
    }

    #[test]
    fn test_get_siblings() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child1_id = Uuid::new_v4();
        let child2_id = Uuid::new_v4();
        let child3_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root"),
            create_test_message(child1_id, Some(root_id), "Child 1"),
            create_test_message(child2_id, Some(root_id), "Child 2"),
            create_test_message(child3_id, Some(root_id), "Child 3"),
        ];

        manager.build_threads(&messages);

        let siblings = manager.get_siblings(child1_id);
        assert_eq!(siblings.len(), 2);
        assert!(siblings.contains(&child2_id));
        assert!(siblings.contains(&child3_id));
        assert!(!siblings.contains(&child1_id));
    }

    #[test]
    fn test_get_path_to_message() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root"),
            create_test_message(child_id, Some(root_id), "Child"),
            create_test_message(grandchild_id, Some(child_id), "Grandchild"),
        ];

        manager.build_threads(&messages);

        let path = manager.get_path_to_message(grandchild_id);
        assert_eq!(path, vec![root_id, child_id, grandchild_id]);

        let path = manager.get_path_to_message(root_id);
        assert_eq!(path, vec![root_id]);
    }

    #[test]
    fn test_add_message() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let root_message = create_test_message(root_id, None, "Root");

        manager.add_message(&root_message).unwrap();

        assert_eq!(manager.get_thread_roots().len(), 1);
        assert!(manager.is_root_message(root_id));

        let child_id = Uuid::new_v4();
        let child_message = create_test_message(child_id, Some(root_id), "Child");

        manager.add_message(&child_message).unwrap();

        assert_eq!(manager.get_children(root_id), vec![child_id]);
        assert_eq!(manager.get_parent(child_id), Some(root_id));
    }

    #[test]
    fn test_remove_message_with_children() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root"),
            create_test_message(child_id, Some(root_id), "Child"),
            create_test_message(grandchild_id, Some(child_id), "Grandchild"),
        ];

        manager.build_threads(&messages);

        let removed = manager.remove_message(child_id, true).unwrap();
        assert_eq!(removed.len(), 2); // child and grandchild
        assert!(removed.contains(&child_id));
        assert!(removed.contains(&grandchild_id));

        assert!(manager.get_children(root_id).is_empty());
    }

    #[test]
    fn test_remove_message_reparent_children() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root"),
            create_test_message(child_id, Some(root_id), "Child"),
            create_test_message(grandchild_id, Some(child_id), "Grandchild"),
        ];

        manager.build_threads(&messages);

        let removed = manager.remove_message(child_id, false).unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], child_id);

        // Grandchild should now be a direct child of root
        assert_eq!(manager.get_parent(grandchild_id), Some(root_id));
        assert_eq!(manager.get_children(root_id), vec![grandchild_id]);
    }

    #[test]
    fn test_calculate_statistics() {
        let mut manager = ThreadManager::new();

        let root1_id = Uuid::new_v4();
        let root2_id = Uuid::new_v4();
        let child1_id = Uuid::new_v4();
        let child2_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root1_id, None, "Root 1"),
            create_test_message(child1_id, Some(root1_id), "Child 1"),
            create_test_message(grandchild_id, Some(child1_id), "Grandchild"),
            create_test_message(root2_id, None, "Root 2"),
            create_test_message(child2_id, Some(root2_id), "Child 2"),
        ];

        manager.build_threads(&messages);

        let stats = manager.calculate_statistics();
        assert_eq!(stats.total_threads, 2);
        assert_eq!(stats.total_messages, 5);
        assert_eq!(stats.max_depth, 2);
    }

    #[test]
    fn test_get_conversation_flow() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child1_id = Uuid::new_v4();
        let child2_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root"),
            create_test_message(child1_id, Some(root_id), "Child 1"),
            create_test_message(child2_id, Some(root_id), "Child 2"),
            create_test_message(grandchild_id, Some(child1_id), "Grandchild"),
        ];

        manager.build_threads(&messages);

        let flow = manager.get_conversation_flow(root_id);
        // Should follow the first child path
        assert_eq!(flow, vec![root_id, child1_id, grandchild_id]);
    }

    #[test]
    fn test_get_messages_at_depth() {
        let mut manager = ThreadManager::new();

        let root_id = Uuid::new_v4();
        let child1_id = Uuid::new_v4();
        let child2_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        let messages = vec![
            create_test_message(root_id, None, "Root"),
            create_test_message(child1_id, Some(root_id), "Child 1"),
            create_test_message(child2_id, Some(root_id), "Child 2"),
            create_test_message(grandchild_id, Some(child1_id), "Grandchild"),
        ];

        manager.build_threads(&messages);

        let depth_0 = manager.get_messages_at_depth(root_id, 0);
        assert_eq!(depth_0, vec![root_id]);

        let depth_1 = manager.get_messages_at_depth(root_id, 1);
        assert_eq!(depth_1.len(), 2);
        assert!(depth_1.contains(&child1_id));
        assert!(depth_1.contains(&child2_id));

        let depth_2 = manager.get_messages_at_depth(root_id, 2);
        assert_eq!(depth_2, vec![grandchild_id]);
    }
}
