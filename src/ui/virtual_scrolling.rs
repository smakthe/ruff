//! Virtual scrolling implementation for efficient message display
//! 
//! This module provides virtual scrolling capabilities to handle large
//! message histories without performance degradation.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Local};
use ratatui::layout::Rect;
use uuid::Uuid;

use crate::events::{SessionId, MessageId};
use crate::session::manager::Message;
use crate::session::lazy_loading::LazyMessageLoader;
use crate::EnhancedError;

/// Virtual scrolling viewport for messages
pub struct VirtualScrollView {
    /// Session being displayed
    session_id: SessionId,
    /// Lazy message loader
    message_loader: Arc<LazyMessageLoader>,
    /// Current viewport configuration
    viewport: Viewport,
    /// Cached visible messages
    visible_messages: Arc<RwLock<VecDeque<MessageItem>>>,
    /// Scroll position tracking
    scroll_state: ScrollState,
    /// Rendering configuration
    render_config: RenderConfig,
}

/// Viewport configuration
#[derive(Debug, Clone)]
pub struct Viewport {
    /// Available display area
    pub area: Rect,
    /// Number of lines per message (estimated)
    pub lines_per_message: usize,
    /// Buffer size (messages to keep loaded beyond visible area)
    pub buffer_size: usize,
    /// Total messages in session
    pub total_messages: usize,
}

/// Scroll state tracking
#[derive(Debug, Clone)]
pub struct ScrollState {
    /// Current scroll position (message index)
    pub position: usize,
    /// Offset within the current message (for partial visibility)
    pub offset: usize,
    /// First visible message index
    pub first_visible: usize,
    /// Last visible message index
    pub last_visible: usize,
    /// Whether we're at the top
    pub at_top: bool,
    /// Whether we're at the bottom
    pub at_bottom: bool,
}

/// Message item for virtual scrolling
#[derive(Debug, Clone)]
pub struct MessageItem {
    pub message: Message,
    pub rendered_height: usize,
    pub y_position: usize,
    pub is_visible: bool,
    pub is_partially_visible: bool,
}

/// Rendering configuration
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Maximum width for message content
    pub max_width: usize,
    /// Show timestamps
    pub show_timestamps: bool,
    /// Show message metadata
    pub show_metadata: bool,
    /// Compact mode (fewer lines per message)
    pub compact_mode: bool,
}

/// Scroll direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// Scroll result
#[derive(Debug, Clone)]
pub struct ScrollResult {
    pub scrolled: bool,
    pub new_position: usize,
    pub messages_loaded: usize,
    pub messages_unloaded: usize,
}

impl VirtualScrollView {
    /// Create a new virtual scroll view
    pub fn new(
        session_id: SessionId,
        message_loader: Arc<LazyMessageLoader>,
        viewport: Viewport,
        render_config: RenderConfig,
    ) -> Self {
        let scroll_state = ScrollState {
            position: 0,
            offset: 0,
            first_visible: 0,
            last_visible: 0,
            at_top: true,
            at_bottom: viewport.total_messages == 0,
        };

        Self {
            session_id,
            message_loader,
            viewport,
            visible_messages: Arc::new(RwLock::new(VecDeque::new())),
            scroll_state,
            render_config,
        }
    }

    /// Initialize the virtual scroll view
    pub async fn initialize(&mut self) -> Result<(), EnhancedError> {
        // Load initial messages
        self.load_initial_messages().await?;
        self.update_scroll_state();
        Ok(())
    }

    /// Load initial messages for the viewport
    async fn load_initial_messages(&mut self) -> Result<(), EnhancedError> {
        let visible_count = self.calculate_visible_message_count();
        let buffer_count = self.viewport.buffer_size;
        let total_to_load = visible_count + buffer_count * 2;

        // Start from the end (most recent messages) by default
        let start_index = if self.viewport.total_messages > total_to_load {
            self.viewport.total_messages - total_to_load
        } else {
            0
        };

        let messages = self.message_loader
            .get_message_range(self.session_id, start_index, total_to_load)
            .await?;

        let mut visible_messages = self.visible_messages.write().unwrap();
        visible_messages.clear();

        for (i, message) in messages.into_iter().enumerate() {
            let message_item = MessageItem {
                rendered_height: self.calculate_message_height(&message),
                y_position: 0, // Will be calculated in update_positions
                is_visible: false, // Will be updated in update_visibility
                is_partially_visible: false,
                message,
            };
            visible_messages.push_back(message_item);
        }

        self.update_message_positions();
        self.update_message_visibility();

        Ok(())
    }

    /// Calculate how many messages can fit in the viewport
    fn calculate_visible_message_count(&self) -> usize {
        let available_height = self.viewport.area.height as usize;
        let estimated_messages = available_height / self.viewport.lines_per_message;
        estimated_messages.max(1)
    }

    /// Calculate the rendered height of a message
    fn calculate_message_height(&self, message: &Message) -> usize {
        let base_height = if self.render_config.compact_mode { 1 } else { 2 };
        let content_lines = self.calculate_content_lines(&message.content);
        let metadata_lines = if self.render_config.show_metadata { 1 } else { 0 };
        let timestamp_lines = if self.render_config.show_timestamps { 1 } else { 0 };

        base_height + content_lines + metadata_lines + timestamp_lines
    }

    /// Calculate how many lines the message content will take
    fn calculate_content_lines(&self, content: &str) -> usize {
        let max_width = self.render_config.max_width;
        if max_width == 0 {
            return content.lines().count();
        }

        content
            .lines()
            .map(|line| (line.len() + max_width - 1) / max_width) // Ceiling division
            .sum::<usize>()
            .max(1)
    }

    /// Update message positions based on their heights
    fn update_message_positions(&mut self) {
        let mut visible_messages = self.visible_messages.write().unwrap();
        let mut current_y = 0;

        for message_item in visible_messages.iter_mut() {
            message_item.y_position = current_y;
            current_y += message_item.rendered_height;
        }
    }

    /// Update which messages are visible in the current viewport
    fn update_message_visibility(&mut self) {
        let viewport_height = self.viewport.area.height as usize;
        let scroll_offset = self.scroll_state.position;

        let mut visible_messages = self.visible_messages.write().unwrap();
        let mut first_visible = None;
        let mut last_visible = None;

        for (index, message_item) in visible_messages.iter_mut().enumerate() {
            let message_top = message_item.y_position;
            let message_bottom = message_top + message_item.rendered_height;

            // Check if message is visible
            let is_visible = message_bottom > scroll_offset && message_top < scroll_offset + viewport_height;
            let is_partially_visible = is_visible && (message_top < scroll_offset || message_bottom > scroll_offset + viewport_height);

            message_item.is_visible = is_visible;
            message_item.is_partially_visible = is_partially_visible;

            if is_visible {
                if first_visible.is_none() {
                    first_visible = Some(index);
                }
                last_visible = Some(index);
            }
        }

        // Update scroll state
        self.scroll_state.first_visible = first_visible.unwrap_or(0);
        self.scroll_state.last_visible = last_visible.unwrap_or(0);
    }

    /// Update scroll state flags
    fn update_scroll_state(&mut self) {
        self.scroll_state.at_top = self.scroll_state.position == 0;
        
        let visible_messages = self.visible_messages.read().unwrap();
        if let Some(last_message) = visible_messages.back() {
            let total_content_height = last_message.y_position + last_message.rendered_height;
            let viewport_height = self.viewport.area.height as usize;
            self.scroll_state.at_bottom = self.scroll_state.position + viewport_height >= total_content_height;
        } else {
            self.scroll_state.at_bottom = true;
        }
    }

    /// Scroll in the specified direction
    pub async fn scroll(&mut self, direction: ScrollDirection, amount: Option<usize>) -> Result<ScrollResult, EnhancedError> {
        let old_position = self.scroll_state.position;
        let viewport_height = self.viewport.area.height as usize;

        let new_position = match direction {
            ScrollDirection::Up => {
                let scroll_amount = amount.unwrap_or(1);
                self.scroll_state.position.saturating_sub(scroll_amount)
            }
            ScrollDirection::Down => {
                let scroll_amount = amount.unwrap_or(1);
                self.scroll_state.position + scroll_amount
            }
            ScrollDirection::PageUp => {
                let page_size = viewport_height.saturating_sub(1);
                self.scroll_state.position.saturating_sub(page_size)
            }
            ScrollDirection::PageDown => {
                let page_size = viewport_height.saturating_sub(1);
                self.scroll_state.position + page_size
            }
            ScrollDirection::Home => 0,
            ScrollDirection::End => {
                // Calculate total content height
                let visible_messages = self.visible_messages.read().unwrap();
                if let Some(last_message) = visible_messages.back() {
                    let total_height = last_message.y_position + last_message.rendered_height;
                    total_height.saturating_sub(viewport_height)
                } else {
                    0
                }
            }
        };

        self.scroll_state.position = new_position;

        // Check if we need to load more messages
        let mut messages_loaded = 0;
        let mut messages_unloaded = 0;

        if self.needs_more_messages().await? {
            let load_result = self.load_additional_messages().await?;
            messages_loaded = load_result.loaded;
            messages_unloaded = load_result.unloaded;
        }

        // Update visibility and positions
        self.update_message_visibility();
        self.update_scroll_state();

        Ok(ScrollResult {
            scrolled: old_position != self.scroll_state.position,
            new_position: self.scroll_state.position,
            messages_loaded,
            messages_unloaded,
        })
    }

    /// Check if we need to load more messages based on scroll position
    async fn needs_more_messages(&self) -> Result<bool, EnhancedError> {
        let visible_messages = self.visible_messages.read().unwrap();
        let buffer_size = self.viewport.buffer_size;

        // Check if we're near the top and need earlier messages
        let near_top = self.scroll_state.first_visible < buffer_size;
        
        // Check if we're near the bottom and need later messages
        let near_bottom = visible_messages.len() - self.scroll_state.last_visible < buffer_size;

        Ok(near_top || near_bottom)
    }

    /// Load additional messages when needed
    async fn load_additional_messages(&mut self) -> Result<LoadResult, EnhancedError> {
        let mut loaded = 0;
        let mut unloaded = 0;

        let visible_messages_count = {
            let visible_messages = self.visible_messages.read().unwrap();
            visible_messages.len()
        };

        let buffer_size = self.viewport.buffer_size;

        // Load earlier messages if near the top
        if self.scroll_state.first_visible < buffer_size {
            let current_first_index = self.get_first_message_index();
            if current_first_index > 0 {
                let load_count = buffer_size.min(current_first_index);
                let start_index = current_first_index - load_count;
                
                let earlier_messages = self.message_loader
                    .get_message_range(self.session_id, start_index, load_count)
                    .await?;

                let mut visible_messages = self.visible_messages.write().unwrap();
                for message in earlier_messages.into_iter().rev() {
                    let message_item = MessageItem {
                        rendered_height: self.calculate_message_height(&message),
                        y_position: 0,
                        is_visible: false,
                        is_partially_visible: false,
                        message,
                    };
                    visible_messages.push_front(message_item);
                    loaded += 1;
                }
            }
        }

        // Load later messages if near the bottom
        if visible_messages_count - self.scroll_state.last_visible < buffer_size {
            let current_last_index = self.get_last_message_index();
            if current_last_index < self.viewport.total_messages - 1 {
                let remaining_messages = self.viewport.total_messages - current_last_index - 1;
                let load_count = buffer_size.min(remaining_messages);
                let start_index = current_last_index + 1;
                
                let later_messages = self.message_loader
                    .get_message_range(self.session_id, start_index, load_count)
                    .await?;

                let mut visible_messages = self.visible_messages.write().unwrap();
                for message in later_messages {
                    let message_item = MessageItem {
                        rendered_height: self.calculate_message_height(&message),
                        y_position: 0,
                        is_visible: false,
                        is_partially_visible: false,
                        message,
                    };
                    visible_messages.push_back(message_item);
                    loaded += 1;
                }
            }
        }

        // Unload messages that are too far from the viewport
        let max_cache_size = self.viewport.buffer_size * 4; // Keep 4x buffer size
        {
            let mut visible_messages = self.visible_messages.write().unwrap();
            while visible_messages.len() > max_cache_size {
                // Remove from the end that's furthest from current view
                if self.scroll_state.first_visible < visible_messages.len() / 2 {
                    // Remove from back
                    visible_messages.pop_back();
                } else {
                    // Remove from front
                    visible_messages.pop_front();
                }
                unloaded += 1;
            }
        }

        // Update positions after loading/unloading
        if loaded > 0 || unloaded > 0 {
            self.update_message_positions();
        }

        Ok(LoadResult { loaded, unloaded })
    }

    /// Get the index of the first message in the visible buffer
    fn get_first_message_index(&self) -> usize {
        // This would need to be tracked based on the actual session message indices
        // For now, return 0 as a placeholder
        0
    }

    /// Get the index of the last message in the visible buffer
    fn get_last_message_index(&self) -> usize {
        let visible_messages = self.visible_messages.read().unwrap();
        visible_messages.len().saturating_sub(1)
    }

    /// Get currently visible messages for rendering
    pub fn get_visible_messages(&self) -> Vec<MessageItem> {
        let visible_messages = self.visible_messages.read().unwrap();
        visible_messages
            .iter()
            .filter(|item| item.is_visible)
            .cloned()
            .collect()
    }

    /// Get all loaded messages (for debugging)
    pub fn get_all_loaded_messages(&self) -> Vec<MessageItem> {
        let visible_messages = self.visible_messages.read().unwrap();
        visible_messages.iter().cloned().collect()
    }

    /// Jump to a specific message
    pub async fn jump_to_message(&mut self, message_id: MessageId) -> Result<bool, EnhancedError> {
        // This would require finding the message index and scrolling to it
        // Implementation would depend on having a message index lookup
        Ok(false) // Placeholder
    }

    /// Update viewport size (when terminal is resized)
    pub async fn update_viewport(&mut self, new_area: Rect) -> Result<(), EnhancedError> {
        self.viewport.area = new_area;
        self.render_config.max_width = new_area.width as usize;
        
        // Recalculate message heights and positions
        {
            let mut visible_messages = self.visible_messages.write().unwrap();
            for message_item in visible_messages.iter_mut() {
                message_item.rendered_height = self.calculate_message_height(&message_item.message);
            }
        }
        
        self.update_message_positions();
        self.update_message_visibility();
        self.update_scroll_state();
        
        Ok(())
    }

    /// Get scroll state for UI display
    pub fn get_scroll_state(&self) -> &ScrollState {
        &self.scroll_state
    }

    /// Get viewport information
    pub fn get_viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Update render configuration
    pub fn update_render_config(&mut self, config: RenderConfig) {
        self.render_config = config;
        
        // Recalculate message heights
        {
            let mut visible_messages = self.visible_messages.write().unwrap();
            for message_item in visible_messages.iter_mut() {
                message_item.rendered_height = self.calculate_message_height(&message_item.message);
            }
        }
        
        self.update_message_positions();
        self.update_message_visibility();
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> VirtualScrollStats {
        let visible_messages = self.visible_messages.read().unwrap();
        let visible_count = visible_messages.iter().filter(|item| item.is_visible).count();
        
        VirtualScrollStats {
            total_messages: self.viewport.total_messages,
            loaded_messages: visible_messages.len(),
            visible_messages: visible_count,
            scroll_position: self.scroll_state.position,
            memory_usage_estimate: visible_messages.len() * std::mem::size_of::<MessageItem>(),
        }
    }
}

/// Load result for additional messages
#[derive(Debug, Clone)]
struct LoadResult {
    loaded: usize,
    unloaded: usize,
}

/// Virtual scroll performance statistics
#[derive(Debug, Clone)]
pub struct VirtualScrollStats {
    pub total_messages: usize,
    pub loaded_messages: usize,
    pub visible_messages: usize,
    pub scroll_position: usize,
    pub memory_usage_estimate: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            max_width: 80,
            show_timestamps: true,
            show_metadata: false,
            compact_mode: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;
    use crate::session::lazy_loading::LazyMessageLoader;
    use crate::session::manager::{MessageRole, MessageMetadata};
    use crate::models::TokenUsage;

    fn create_test_message(id: MessageId, content: &str) -> Message {
        Message {
            id,
            role: MessageRole::User,
            content: content.to_string(),
            timestamp: chrono::Local::now(),
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

    async fn create_test_virtual_scroll() -> (VirtualScrollView, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let message_loader = Arc::new(LazyMessageLoader::new(temp_dir.path().to_path_buf(), 10, 5));
        message_loader.initialize().await.unwrap();

        let session_id = Uuid::new_v4();
        let viewport = Viewport {
            area: Rect::new(0, 0, 80, 20),
            lines_per_message: 3,
            buffer_size: 5,
            total_messages: 0,
        };

        let render_config = RenderConfig::default();
        let virtual_scroll = VirtualScrollView::new(session_id, message_loader, viewport, render_config);

        (virtual_scroll, temp_dir)
    }

    #[tokio::test]
    async fn test_virtual_scroll_creation() {
        let (virtual_scroll, _temp_dir) = create_test_virtual_scroll().await;
        
        assert_eq!(virtual_scroll.scroll_state.position, 0);
        assert!(virtual_scroll.scroll_state.at_top);
        assert!(virtual_scroll.scroll_state.at_bottom);
    }

    #[tokio::test]
    async fn test_message_height_calculation() {
        let (virtual_scroll, _temp_dir) = create_test_virtual_scroll().await;
        
        let short_message = create_test_message(Uuid::new_v4(), "Short");
        let long_message = create_test_message(Uuid::new_v4(), "This is a very long message that should wrap across multiple lines when displayed in the terminal interface");

        let short_height = virtual_scroll.calculate_message_height(&short_message);
        let long_height = virtual_scroll.calculate_message_height(&long_message);

        assert!(long_height > short_height);
        assert!(short_height >= 2); // Base height + content
    }

    #[tokio::test]
    async fn test_visible_message_count_calculation() {
        let (virtual_scroll, _temp_dir) = create_test_virtual_scroll().await;
        
        let visible_count = virtual_scroll.calculate_visible_message_count();
        
        // With height 20 and 3 lines per message, should be around 6-7 messages
        assert!(visible_count >= 6);
        assert!(visible_count <= 7);
    }

    #[tokio::test]
    async fn test_scroll_directions() {
        let (mut virtual_scroll, _temp_dir) = create_test_virtual_scroll().await;
        
        // Test scrolling down
        let result = virtual_scroll.scroll(ScrollDirection::Down, Some(5)).await.unwrap();
        assert_eq!(result.new_position, 5);
        
        // Test scrolling up
        let result = virtual_scroll.scroll(ScrollDirection::Up, Some(3)).await.unwrap();
        assert_eq!(result.new_position, 2);
        
        // Test home
        let result = virtual_scroll.scroll(ScrollDirection::Home, None).await.unwrap();
        assert_eq!(result.new_position, 0);
        assert!(virtual_scroll.scroll_state.at_top);
    }

    #[tokio::test]
    async fn test_viewport_update() {
        let (mut virtual_scroll, _temp_dir) = create_test_virtual_scroll().await;
        
        let new_area = Rect::new(0, 0, 120, 30);
        virtual_scroll.update_viewport(new_area).await.unwrap();
        
        assert_eq!(virtual_scroll.viewport.area, new_area);
        assert_eq!(virtual_scroll.render_config.max_width, 120);
    }

    #[tokio::test]
    async fn test_render_config_update() {
        let (mut virtual_scroll, _temp_dir) = create_test_virtual_scroll().await;
        
        let new_config = RenderConfig {
            max_width: 100,
            show_timestamps: false,
            show_metadata: true,
            compact_mode: true,
        };
        
        virtual_scroll.update_render_config(new_config.clone());
        
        assert_eq!(virtual_scroll.render_config.max_width, 100);
        assert!(!virtual_scroll.render_config.show_timestamps);
        assert!(virtual_scroll.render_config.show_metadata);
        assert!(virtual_scroll.render_config.compact_mode);
    }
}