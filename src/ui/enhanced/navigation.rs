//! Navigation functionality for message jumping and session navigation

use crate::events::{SessionId, MessageId};
use std::collections::HashMap;

/// Navigation manager for handling message jumping and session navigation
pub struct NavigationManager {
    current_session: Option<SessionId>,
    back_history: Vec<SessionId>,    // Sessions we can go back to
    forward_history: Vec<SessionId>, // Sessions we can go forward to
    message_positions: HashMap<SessionId, MessagePosition>,
    smooth_scroll_enabled: bool,
}

/// Message position tracking within a session
#[derive(Debug, Clone)]
pub struct MessagePosition {
    pub current_message: Option<MessageId>,
    pub message_index: usize,
    pub total_messages: usize,
    pub scroll_offset: usize,
}

/// Navigation result
#[derive(Debug, Clone)]
pub enum NavigationResult {
    Success(NavigationAction),
    SessionNotFound(SessionId),
    MessageNotFound(MessageId),
    InvalidMessageNumber(usize),
    NoMessages,
    AlreadyAtPosition,
}

/// Navigation actions
#[derive(Debug, Clone)]
pub enum NavigationAction {
    SessionChanged(SessionId),
    MessageJumped(MessageId, usize),
    ScrollChanged(usize),
    FirstMessage(MessageId),
    LastMessage(MessageId),
    NextSession,
    PreviousSession,
    GoToMessage(usize),
    ScrollToTop,
    ScrollToBottom,
}

/// Navigation direction
#[derive(Debug, Clone, Copy)]
pub enum NavigationDirection {
    Previous,
    Next,
    First,
    Last,
}

impl NavigationManager {
    /// Create a new navigation manager
    pub fn new() -> Self {
        Self {
            current_session: None,
            back_history: Vec::new(),
            forward_history: Vec::new(),
            message_positions: HashMap::new(),
            smooth_scroll_enabled: true,
        }
    }
    
    /// Handle key events for navigation
    pub fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Option<NavigationAction> {
        use crossterm::event::{KeyCode, KeyModifiers};
        
        match (key.code, key.modifiers) {
            // Alt+Left/Right for session navigation
            (KeyCode::Left, KeyModifiers::ALT) => Some(NavigationAction::PreviousSession),
            (KeyCode::Right, KeyModifiers::ALT) => Some(NavigationAction::NextSession),
            
            // Ctrl+G for "Go to Message"
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => Some(NavigationAction::GoToMessage(1)), // Default to message 1
            
            // Ctrl+Home/End for first/last message
            (KeyCode::Home, KeyModifiers::CONTROL) => Some(NavigationAction::ScrollToTop),
            (KeyCode::End, KeyModifiers::CONTROL) => Some(NavigationAction::ScrollToBottom),
            
            // Page Up/Down for scrolling
            (KeyCode::PageUp, KeyModifiers::NONE) => Some(NavigationAction::ScrollToTop),
            (KeyCode::PageDown, KeyModifiers::NONE) => Some(NavigationAction::ScrollToBottom),
            
            _ => None,
        }
    }

    /// Set the current session
    pub fn set_current_session(&mut self, session_id: SessionId) {
        if self.current_session != Some(session_id) {
            // Add current session to back history if it exists
            if let Some(current) = self.current_session {
                self.back_history.push(current);
            }
            
            // Clear forward history when navigating to a new session
            self.forward_history.clear();
            
            self.current_session = Some(session_id);
            
            // Initialize message position if not exists
            if !self.message_positions.contains_key(&session_id) {
                self.message_positions.insert(session_id, MessagePosition {
                    current_message: None,
                    message_index: 0,
                    total_messages: 0,
                    scroll_offset: 0,
                });
            }
        }
    }

    /// Navigate to previous session in history
    pub fn navigate_to_previous_session(&mut self) -> Result<NavigationResult, String> {
        if let Some(previous_session) = self.back_history.pop() {
            // Add current session to forward history
            if let Some(current) = self.current_session {
                self.forward_history.push(current);
            }
            
            self.current_session = Some(previous_session);
            Ok(NavigationResult::Success(NavigationAction::SessionChanged(previous_session)))
        } else {
            Err("No previous session in history".to_string())
        }
    }

    /// Navigate to next session in history
    pub fn navigate_to_next_session(&mut self) -> Result<NavigationResult, String> {
        if let Some(next_session) = self.forward_history.pop() {
            // Add current session to back history
            if let Some(current) = self.current_session {
                self.back_history.push(current);
            }
            
            self.current_session = Some(next_session);
            Ok(NavigationResult::Success(NavigationAction::SessionChanged(next_session)))
        } else {
            Err("No next session in history".to_string())
        }
    }

    /// Jump to a specific message by number (1-based indexing)
    pub fn goto_message(&mut self, message_number: usize) -> NavigationResult {
        let session_id = match self.current_session {
            Some(id) => id,
            None => return NavigationResult::NoMessages,
        };

        let position = match self.message_positions.get_mut(&session_id) {
            Some(pos) => pos,
            None => return NavigationResult::SessionNotFound(session_id),
        };

        if message_number == 0 || message_number > position.total_messages {
            return NavigationResult::InvalidMessageNumber(message_number);
        }

        let message_index = message_number - 1; // Convert to 0-based
        
        if position.message_index == message_index {
            return NavigationResult::AlreadyAtPosition;
        }

        position.message_index = message_index;
        
        // Generate a mock message ID for now (in real implementation, this would come from the session)
        let message_id = uuid::Uuid::new_v4();
        position.current_message = Some(message_id);

        NavigationResult::Success(NavigationAction::MessageJumped(message_id, message_number))
    }

    /// Jump to first message in current session
    pub fn goto_first_message(&mut self) -> NavigationResult {
        let session_id = match self.current_session {
            Some(id) => id,
            None => return NavigationResult::NoMessages,
        };

        let position = match self.message_positions.get_mut(&session_id) {
            Some(pos) => pos,
            None => return NavigationResult::SessionNotFound(session_id),
        };

        if position.total_messages == 0 {
            return NavigationResult::NoMessages;
        }

        if position.message_index == 0 {
            return NavigationResult::AlreadyAtPosition;
        }

        position.message_index = 0;
        position.scroll_offset = 0;
        
        let message_id = uuid::Uuid::new_v4();
        position.current_message = Some(message_id);

        NavigationResult::Success(NavigationAction::FirstMessage(message_id))
    }

    /// Jump to last message in current session
    pub fn goto_last_message(&mut self) -> NavigationResult {
        let session_id = match self.current_session {
            Some(id) => id,
            None => return NavigationResult::NoMessages,
        };

        let position = match self.message_positions.get_mut(&session_id) {
            Some(pos) => pos,
            None => return NavigationResult::SessionNotFound(session_id),
        };

        if position.total_messages == 0 {
            return NavigationResult::NoMessages;
        }

        let last_index = position.total_messages - 1;
        
        if position.message_index == last_index {
            return NavigationResult::AlreadyAtPosition;
        }

        position.message_index = last_index;
        
        let message_id = uuid::Uuid::new_v4();
        position.current_message = Some(message_id);

        NavigationResult::Success(NavigationAction::LastMessage(message_id))
    }

    /// Navigate in a specific direction
    pub fn navigate_message(&mut self, direction: NavigationDirection) -> NavigationResult {
        match direction {
            NavigationDirection::First => self.goto_first_message(),
            NavigationDirection::Last => self.goto_last_message(),
            NavigationDirection::Previous => self.navigate_to_previous_message(),
            NavigationDirection::Next => self.navigate_to_next_message(),
        }
    }

    /// Navigate to previous message
    pub fn navigate_to_previous_message(&mut self) -> NavigationResult {
        let session_id = match self.current_session {
            Some(id) => id,
            None => return NavigationResult::NoMessages,
        };

        let position = match self.message_positions.get_mut(&session_id) {
            Some(pos) => pos,
            None => return NavigationResult::SessionNotFound(session_id),
        };

        if position.total_messages == 0 {
            return NavigationResult::NoMessages;
        }

        if position.message_index == 0 {
            return NavigationResult::AlreadyAtPosition;
        }

        position.message_index -= 1;
        
        let message_id = uuid::Uuid::new_v4();
        position.current_message = Some(message_id);

        NavigationResult::Success(NavigationAction::MessageJumped(message_id, position.message_index + 1))
    }

    /// Navigate to next message
    pub fn navigate_to_next_message(&mut self) -> NavigationResult {
        let session_id = match self.current_session {
            Some(id) => id,
            None => return NavigationResult::NoMessages,
        };

        let position = match self.message_positions.get_mut(&session_id) {
            Some(pos) => pos,
            None => return NavigationResult::SessionNotFound(session_id),
        };

        if position.total_messages == 0 {
            return NavigationResult::NoMessages;
        }

        if position.message_index >= position.total_messages - 1 {
            return NavigationResult::AlreadyAtPosition;
        }

        position.message_index += 1;
        
        let message_id = uuid::Uuid::new_v4();
        position.current_message = Some(message_id);

        NavigationResult::Success(NavigationAction::MessageJumped(message_id, position.message_index + 1))
    }

    /// Scroll up by a page
    pub fn scroll_page_up(&mut self, page_size: usize) -> NavigationResult {
        let session_id = match self.current_session {
            Some(id) => id,
            None => return NavigationResult::NoMessages,
        };

        let position = match self.message_positions.get_mut(&session_id) {
            Some(pos) => pos,
            None => return NavigationResult::SessionNotFound(session_id),
        };

        let new_offset = position.scroll_offset.saturating_sub(page_size);
        
        if new_offset == position.scroll_offset {
            return NavigationResult::AlreadyAtPosition;
        }

        position.scroll_offset = new_offset;
        NavigationResult::Success(NavigationAction::ScrollChanged(new_offset))
    }

    /// Scroll down by a page
    pub fn scroll_page_down(&mut self, page_size: usize) -> NavigationResult {
        let session_id = match self.current_session {
            Some(id) => id,
            None => return NavigationResult::NoMessages,
        };

        let position = match self.message_positions.get_mut(&session_id) {
            Some(pos) => pos,
            None => return NavigationResult::SessionNotFound(session_id),
        };

        let max_scroll = position.total_messages.saturating_sub(page_size);
        let new_offset = (position.scroll_offset + page_size).min(max_scroll);
        
        if new_offset == position.scroll_offset {
            return NavigationResult::AlreadyAtPosition;
        }

        position.scroll_offset = new_offset;
        NavigationResult::Success(NavigationAction::ScrollChanged(new_offset))
    }

    /// Update message count for current session
    pub fn update_message_count(&mut self, count: usize) {
        if let Some(session_id) = self.current_session {
            if let Some(position) = self.message_positions.get_mut(&session_id) {
                position.total_messages = count;
                
                // Ensure current index is valid
                if position.message_index >= count && count > 0 {
                    position.message_index = count - 1;
                }
            }
        }
    }

    /// Get current message position
    pub fn get_current_position(&self) -> Option<&MessagePosition> {
        self.current_session
            .and_then(|id| self.message_positions.get(&id))
    }

    /// Get current session
    pub fn get_current_session(&self) -> Option<SessionId> {
        self.current_session
    }

    /// Check if smooth scrolling is enabled
    pub fn is_smooth_scroll_enabled(&self) -> bool {
        self.smooth_scroll_enabled
    }

    /// Enable or disable smooth scrolling
    pub fn set_smooth_scroll(&mut self, enabled: bool) {
        self.smooth_scroll_enabled = enabled;
    }

    /// Get session history (back history for compatibility)
    pub fn get_session_history(&self) -> &[SessionId] {
        &self.back_history
    }

    /// Get current history index (for compatibility)
    pub fn get_history_index(&self) -> usize {
        self.back_history.len()
    }

    /// Clear navigation history
    pub fn clear_history(&mut self) {
        self.back_history.clear();
        self.forward_history.clear();
    }

    /// Remove a session from tracking
    pub fn remove_session(&mut self, session_id: SessionId) {
        self.message_positions.remove(&session_id);
        self.back_history.retain(|&id| id != session_id);
        self.forward_history.retain(|&id| id != session_id);
        
        if self.current_session == Some(session_id) {
            self.current_session = None;
        }
    }
}

impl Default for NavigationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session_id() -> SessionId {
        uuid::Uuid::new_v4()
    }

    #[test]
    fn test_navigation_manager_creation() {
        let nav = NavigationManager::new();
        assert!(nav.get_current_session().is_none());
        assert_eq!(nav.get_history_index(), 0);
        assert!(nav.get_session_history().is_empty());
        assert!(nav.is_smooth_scroll_enabled());
        assert!(nav.get_current_position().is_none());
    }

    #[test]
    fn test_set_current_session() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        
        nav.set_current_session(session1);
        assert_eq!(nav.get_current_session(), Some(session1));
        assert!(nav.get_current_position().is_some());
        
        let position = nav.get_current_position().unwrap();
        assert_eq!(position.message_index, 0);
        assert_eq!(position.total_messages, 0);
        assert_eq!(position.scroll_offset, 0);
        assert!(position.current_message.is_none());
    }

    #[test]
    fn test_session_history() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        let session2 = create_test_session_id();
        let session3 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.set_current_session(session2);
        nav.set_current_session(session3);
        
        // Should have session1 and session2 in history
        let history = nav.get_session_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], session1);
        assert_eq!(history[1], session2);
        assert_eq!(nav.get_current_session(), Some(session3));
    }

    #[test]
    fn test_navigate_previous_session() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        let session2 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.set_current_session(session2);
        
        // Navigate back to session1
        let result = nav.navigate_to_previous_session();
        assert!(result.is_ok());
        
        match result.unwrap() {
            NavigationResult::Success(NavigationAction::SessionChanged(id)) => {
                assert_eq!(id, session1);
            }
            _ => panic!("Expected SessionChanged action"),
        }
        
        assert_eq!(nav.get_current_session(), Some(session1));
        
        // Try to navigate back when at beginning
        let result = nav.navigate_to_previous_session();
        assert!(result.is_err());
    }

    #[test]
    fn test_navigate_next_session() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        let session2 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.set_current_session(session2);
        
        // Check history state
        let history = nav.get_session_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], session1);
        assert_eq!(nav.get_current_session(), Some(session2));
        
        // Navigate back first
        nav.navigate_to_previous_session().unwrap();
        assert_eq!(nav.get_current_session(), Some(session1));
        
        // Navigate forward - should go back to session2
        let result = nav.navigate_to_next_session();
        assert!(result.is_ok());
        
        // The current session should now be session2
        assert_eq!(nav.get_current_session(), Some(session2));
        
        // Try to navigate forward when at end
        let result = nav.navigate_to_next_session();
        assert!(result.is_err());
    }

    #[test]
    fn test_goto_message() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.update_message_count(5);
        
        // Jump to message 3
        let result = nav.goto_message(3);
        match result {
            NavigationResult::Success(NavigationAction::MessageJumped(_, number)) => {
                assert_eq!(number, 3);
            }
            _ => panic!("Expected MessageJumped action"),
        }
        
        let position = nav.get_current_position().unwrap();
        assert_eq!(position.message_index, 2); // 0-based index
        
        // Try invalid message number
        let result = nav.goto_message(10);
        match result {
            NavigationResult::InvalidMessageNumber(10) => {},
            _ => panic!("Expected InvalidMessageNumber"),
        }
        
        // Try message number 0
        let result = nav.goto_message(0);
        match result {
            NavigationResult::InvalidMessageNumber(0) => {},
            _ => panic!("Expected InvalidMessageNumber"),
        }
    }

    #[test]
    fn test_goto_first_message() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.update_message_count(5);
        
        // Jump to message 3 first
        nav.goto_message(3);
        
        // Jump to first message
        let result = nav.goto_first_message();
        match result {
            NavigationResult::Success(NavigationAction::FirstMessage(_)) => {},
            _ => panic!("Expected FirstMessage action"),
        }
        
        let position = nav.get_current_position().unwrap();
        assert_eq!(position.message_index, 0);
        assert_eq!(position.scroll_offset, 0);
        
        // Try again when already at first
        let result = nav.goto_first_message();
        match result {
            NavigationResult::AlreadyAtPosition => {},
            _ => panic!("Expected AlreadyAtPosition"),
        }
    }

    #[test]
    fn test_goto_last_message() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.update_message_count(5);
        
        // Jump to last message
        let result = nav.goto_last_message();
        match result {
            NavigationResult::Success(NavigationAction::LastMessage(_)) => {},
            _ => panic!("Expected LastMessage action"),
        }
        
        let position = nav.get_current_position().unwrap();
        assert_eq!(position.message_index, 4); // Last index for 5 messages
        
        // Try again when already at last
        let result = nav.goto_last_message();
        match result {
            NavigationResult::AlreadyAtPosition => {},
            _ => panic!("Expected AlreadyAtPosition"),
        }
    }

    #[test]
    fn test_navigate_message_direction() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.update_message_count(5);
        
        // Start by jumping to message 3 to have a non-zero position
        nav.goto_message(3);
        
        // Navigate to first
        let result = nav.navigate_message(NavigationDirection::First);
        match result {
            NavigationResult::Success(NavigationAction::FirstMessage(_)) => {},
            _ => panic!("Expected FirstMessage action, got: {:?}", result),
        }
        
        // Navigate to last
        let result = nav.navigate_message(NavigationDirection::Last);
        match result {
            NavigationResult::Success(NavigationAction::LastMessage(_)) => {},
            _ => panic!("Expected LastMessage action, got: {:?}", result),
        }
        
        // Navigate to previous
        let result = nav.navigate_message(NavigationDirection::Previous);
        match result {
            NavigationResult::Success(NavigationAction::MessageJumped(_, number)) => {
                assert_eq!(number, 4); // Second to last message
            }
            _ => panic!("Expected MessageJumped action, got: {:?}", result),
        }
        
        // Navigate to next
        let result = nav.navigate_message(NavigationDirection::Next);
        match result {
            NavigationResult::Success(NavigationAction::MessageJumped(_, number)) => {
                assert_eq!(number, 5); // Back to last message
            }
            _ => panic!("Expected MessageJumped action, got: {:?}", result),
        }
    }

    #[test]
    fn test_scroll_page_up_down() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.update_message_count(20);
        
        // Scroll down first
        let result = nav.scroll_page_down(5);
        match result {
            NavigationResult::Success(NavigationAction::ScrollChanged(offset)) => {
                assert_eq!(offset, 5);
            }
            _ => panic!("Expected ScrollChanged action"),
        }
        
        // Scroll up
        let result = nav.scroll_page_up(3);
        match result {
            NavigationResult::Success(NavigationAction::ScrollChanged(offset)) => {
                assert_eq!(offset, 2);
            }
            _ => panic!("Expected ScrollChanged action"),
        }
        
        // Scroll up beyond beginning
        let result = nav.scroll_page_up(10);
        match result {
            NavigationResult::Success(NavigationAction::ScrollChanged(offset)) => {
                assert_eq!(offset, 0);
            }
            _ => panic!("Expected ScrollChanged action"),
        }
        
        // Try to scroll up when already at top
        let result = nav.scroll_page_up(5);
        match result {
            NavigationResult::AlreadyAtPosition => {},
            _ => panic!("Expected AlreadyAtPosition"),
        }
    }

    #[test]
    fn test_update_message_count() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.update_message_count(10);
        
        let position = nav.get_current_position().unwrap();
        assert_eq!(position.total_messages, 10);
        
        // Jump to last message
        nav.goto_last_message();
        assert_eq!(nav.get_current_position().unwrap().message_index, 9);
        
        // Reduce message count
        nav.update_message_count(5);
        assert_eq!(nav.get_current_position().unwrap().total_messages, 5);
        assert_eq!(nav.get_current_position().unwrap().message_index, 4); // Adjusted to valid index
    }

    #[test]
    fn test_smooth_scroll_setting() {
        let mut nav = NavigationManager::new();
        
        assert!(nav.is_smooth_scroll_enabled());
        
        nav.set_smooth_scroll(false);
        assert!(!nav.is_smooth_scroll_enabled());
        
        nav.set_smooth_scroll(true);
        assert!(nav.is_smooth_scroll_enabled());
    }

    #[test]
    fn test_clear_history() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        let session2 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.set_current_session(session2);
        
        assert!(!nav.get_session_history().is_empty());
        
        nav.clear_history();
        assert!(nav.get_session_history().is_empty());
        assert_eq!(nav.get_history_index(), 0);
    }

    #[test]
    fn test_remove_session() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        let session2 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.set_current_session(session2);
        
        // Remove session1 from tracking
        nav.remove_session(session1);
        
        let history = nav.get_session_history();
        assert!(!history.contains(&session1));
        
        // Remove current session
        nav.remove_session(session2);
        assert!(nav.get_current_session().is_none());
    }

    #[test]
    fn test_no_messages_scenarios() {
        let mut nav = NavigationManager::new();
        
        // Test without any session
        let result = nav.goto_message(1);
        match result {
            NavigationResult::NoMessages => {},
            _ => panic!("Expected NoMessages"),
        }
        
        let result = nav.goto_first_message();
        match result {
            NavigationResult::NoMessages => {},
            _ => panic!("Expected NoMessages"),
        }
        
        // Test with session but no messages
        let session1 = create_test_session_id();
        nav.set_current_session(session1);
        
        let result = nav.goto_first_message();
        match result {
            NavigationResult::NoMessages => {},
            _ => panic!("Expected NoMessages"),
        }
        
        let result = nav.goto_last_message();
        match result {
            NavigationResult::NoMessages => {},
            _ => panic!("Expected NoMessages"),
        }
    }

    #[test]
    fn test_already_at_position_scenarios() {
        let mut nav = NavigationManager::new();
        let session1 = create_test_session_id();
        
        nav.set_current_session(session1);
        nav.update_message_count(5);
        
        // Jump to message 3
        nav.goto_message(3);
        
        // Try to jump to same message
        let result = nav.goto_message(3);
        match result {
            NavigationResult::AlreadyAtPosition => {},
            _ => panic!("Expected AlreadyAtPosition"),
        }
        
        // Navigate to first message
        nav.goto_first_message();
        
        // Try to navigate to previous when at first
        let result = nav.navigate_to_previous_message();
        match result {
            NavigationResult::AlreadyAtPosition => {},
            _ => panic!("Expected AlreadyAtPosition"),
        }
        
        // Navigate to last message
        nav.goto_last_message();
        
        // Try to navigate to next when at last
        let result = nav.navigate_to_next_message();
        match result {
            NavigationResult::AlreadyAtPosition => {},
            _ => panic!("Expected AlreadyAtPosition"),
        }
    }
}