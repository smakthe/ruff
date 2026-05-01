//! Search indexing functionality

use std::collections::HashMap;

use crate::events::{MessageId, SessionId};
use crate::session::manager::{ChatSession, Message};
use crate::EnhancedError;

/// Search index for full-text search
pub struct SearchIndex {
    sessions: HashMap<SessionId, ChatSession>,
    messages: HashMap<SessionId, HashMap<MessageId, Message>>,
}

/// Index manager
pub struct IndexManager {}

impl SearchIndex {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            messages: HashMap::new(),
        }
    }

    pub fn index_session(
        &mut self,
        session_id: SessionId,
        session: ChatSession,
    ) -> Result<(), EnhancedError> {
        self.sessions.insert(session_id, session);
        Ok(())
    }

    pub fn remove_session(&mut self, session_id: SessionId) -> Result<(), EnhancedError> {
        self.sessions.remove(&session_id);
        self.messages.remove(&session_id);
        Ok(())
    }

    pub fn index_message(
        &mut self,
        session_id: SessionId,
        message_id: MessageId,
        message: Message,
    ) -> Result<(), EnhancedError> {
        self.messages
            .entry(session_id)
            .or_default()
            .insert(message_id, message);
        Ok(())
    }

    pub fn remove_message(
        &mut self,
        session_id: SessionId,
        message_id: MessageId,
    ) -> Result<(), EnhancedError> {
        if let Some(messages) = self.messages.get_mut(&session_id) {
            messages.remove(&message_id);
            if messages.is_empty() {
                self.messages.remove(&session_id);
            }
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.sessions.clear();
        self.messages.clear();
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn message_count(&self) -> usize {
        self.messages.values().map(HashMap::len).sum()
    }
}

impl IndexManager {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for IndexManager {
    fn default() -> Self {
        Self::new()
    }
}
