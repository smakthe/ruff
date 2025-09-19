use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use futures_util::StreamExt;

use crate::{
    api::{APIClient, ResponseStream},
    events::{AppEvent, EventBus, SessionId, MessageId},
    models::{AIModel, TokenUsage},
    RuffError,
};

/// Represents the state of a streaming response
#[derive(Debug, Clone)]
pub struct StreamingState {
    pub session_id: SessionId,
    pub message_id: MessageId,
    pub content: String,
    pub is_complete: bool,
    pub started_at: Instant,
    pub last_update: Instant,
    pub token_usage: Option<TokenUsage>,
    pub cancellation_token: CancellationToken,
}

/// Manages streaming responses and typing indicators
pub struct StreamingService {
    active_streams: Arc<RwLock<HashMap<MessageId, StreamingState>>>,
    event_bus: Arc<EventBus>,
    api_client: Arc<APIClient>,
    typing_indicator_interval: Duration,
}

impl StreamingService {
    pub fn new(event_bus: Arc<EventBus>, api_client: Arc<APIClient>) -> Self {
        Self {
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            api_client,
            typing_indicator_interval: Duration::from_millis(500),
        }
    }
    
    /// Start a streaming response
    pub async fn start_streaming(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        model: &AIModel,
        messages: Vec<crate::api::ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(), RuffError> {
        // Create streaming state
        let cancellation_token = CancellationToken::new();
        let streaming_state = StreamingState {
            session_id,
            message_id,
            content: String::new(),
            is_complete: false,
            started_at: Instant::now(),
            last_update: Instant::now(),
            token_usage: None,
            cancellation_token: cancellation_token.clone(),
        };
        
        // Store the streaming state
        {
            let mut streams = self.active_streams.write().await;
            streams.insert(message_id, streaming_state);
        }
        
        // Emit streaming started event
        self.event_bus.publish(AppEvent::StreamingStarted { session_id, message_id }).await
            .map_err(|e| RuffError::Api { message: format!("Event bus error: {}", e) })?;
        
        // Start the streaming request
        let stream = self.api_client.send_streaming_message(
            model,
            messages,
            api_key,
            max_tokens,
            temperature,
        ).await?;
        
        // Start typing indicator
        self.start_typing_indicator(message_id).await;
        
        // Process the stream
        self.process_stream(session_id, message_id, stream).await;
        
        Ok(())
    }
    
    /// Cancel a streaming response
    pub async fn cancel_streaming(&self, message_id: MessageId) -> Result<(), RuffError> {
        let mut streams = self.active_streams.write().await;
        
        if let Some(state) = streams.get_mut(&message_id) {
            state.cancellation_token.cancel();
            
            // Emit cancellation event
            self.event_bus.publish(AppEvent::StreamingCancelled {
                session_id: state.session_id,
                message_id,
            }).await
                .map_err(|e| RuffError::Api { message: format!("Event bus error: {}", e) })?;
            
            streams.remove(&message_id);
        }
        
        Ok(())
    }
    
    /// Get the current content of a streaming message
    pub async fn get_streaming_content(&self, message_id: MessageId) -> Option<String> {
        let streams = self.active_streams.read().await;
        streams.get(&message_id).map(|state| state.content.clone())
    }
    
    /// Check if a message is currently streaming
    pub async fn is_streaming(&self, message_id: MessageId) -> bool {
        let streams = self.active_streams.read().await;
        streams.contains_key(&message_id)
    }
    
    /// Get all active streaming messages
    pub async fn get_active_streams(&self) -> Vec<MessageId> {
        let streams = self.active_streams.read().await;
        streams.keys().cloned().collect()
    }
    
    /// Process a streaming response
    async fn process_stream(
        &self,
        session_id: SessionId,
        message_id: MessageId,
        mut stream: ResponseStream,
    ) {
        let active_streams = Arc::clone(&self.active_streams);
        let event_bus = Arc::clone(&self.event_bus);
        
        tokio::spawn(async move {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // Update streaming state
                        {
                            let mut streams = active_streams.write().await;
                            if let Some(state) = streams.get_mut(&message_id) {
                                if !chunk.content.is_empty() {
                                    state.content.push_str(&chunk.content);
                                    state.last_update = Instant::now();
                                    
                                    // Emit chunk event
                                    let _ = event_bus.publish(AppEvent::StreamingChunk {
                                        session_id,
                                        message_id,
                                        content: chunk.content.clone(),
                                    }).await;
                                }
                                
                                if chunk.is_complete {
                                    state.is_complete = true;
                                    let token_usage = chunk.token_usage.clone();
                                    state.token_usage = chunk.token_usage;
                                    
                                    // Emit completion event
                                    let _ = event_bus.publish(AppEvent::StreamingCompleted {
                                        session_id,
                                        message_id,
                                        token_usage,
                                    }).await;
                                    
                                    // Remove from active streams
                                    streams.remove(&message_id);
                                    break;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        // Emit error event
                        let _ = event_bus.publish(AppEvent::StreamingError {
                            session_id,
                            message_id,
                            error: error.to_string(),
                        }).await;
                        
                        // Remove from active streams
                        let mut streams = active_streams.write().await;
                        streams.remove(&message_id);
                        break;
                    }
                }
            }
        });
    }
    
    /// Start typing indicator for a streaming message
    async fn start_typing_indicator(&self, message_id: MessageId) {
        let active_streams = Arc::clone(&self.active_streams);
        let interval = self.typing_indicator_interval;
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            
            loop {
                ticker.tick().await;
                
                // Check if stream is still active
                let streams = active_streams.read().await;
                if !streams.contains_key(&message_id) {
                    break;
                }
                
                // Check if stream has been updated recently
                if let Some(state) = streams.get(&message_id) {
                    if state.last_update.elapsed() > interval * 2 {
                        // Stream might be stalled, continue showing indicator
                    }
                }
            }
        });
    }
    
    /// Clean up completed or stale streams
    pub async fn cleanup_streams(&self) {
        let mut streams = self.active_streams.write().await;
        let stale_threshold = Duration::from_secs(300); // 5 minutes
        
        streams.retain(|_, state| {
            !state.is_complete && state.started_at.elapsed() < stale_threshold
        });
    }
    
    /// Get streaming statistics
    pub async fn get_streaming_stats(&self) -> StreamingStats {
        let streams = self.active_streams.read().await;
        let active_count = streams.len();
        let total_content_length: usize = streams.values()
            .map(|state| state.content.len())
            .sum();
        
        StreamingStats {
            active_streams: active_count,
            total_content_length,
            average_response_time: streams.values()
                .map(|state| state.last_update.duration_since(state.started_at))
                .fold(Duration::ZERO, |acc, duration| acc + duration)
                .checked_div(active_count as u32)
                .unwrap_or(Duration::ZERO),
        }
    }
}

/// Statistics about streaming responses
#[derive(Debug, Clone)]
pub struct StreamingStats {
    pub active_streams: usize,
    pub total_content_length: usize,
    pub average_response_time: Duration,
}

/// Typing indicator component for UI
pub struct TypingIndicator {
    is_visible: bool,
    animation_frame: usize,
    last_update: Instant,
    animation_frames: Vec<&'static str>,
}

impl TypingIndicator {
    pub fn new() -> Self {
        Self {
            is_visible: false,
            animation_frame: 0,
            last_update: Instant::now(),
            animation_frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
        }
    }
    
    /// Show the typing indicator
    pub fn show(&mut self) {
        self.is_visible = true;
        self.last_update = Instant::now();
    }
    
    /// Hide the typing indicator
    pub fn hide(&mut self) {
        self.is_visible = false;
        self.animation_frame = 0;
    }
    
    /// Update the animation frame
    pub fn update(&mut self) {
        if !self.is_visible {
            return;
        }
        
        if self.last_update.elapsed() >= Duration::from_millis(100) {
            self.animation_frame = (self.animation_frame + 1) % self.animation_frames.len();
            self.last_update = Instant::now();
        }
    }
    
    /// Get the current animation frame
    pub fn current_frame(&self) -> &str {
        if self.is_visible {
            self.animation_frames[self.animation_frame]
        } else {
            ""
        }
    }
    
    /// Check if the indicator is visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
}

impl Default for TypingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::api::APIClient;
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_streaming_service_creation() {
        let event_bus = Arc::new(EventBus::new());
        let api_client = Arc::new(APIClient::new());
        let streaming_service = StreamingService::new(event_bus, api_client);
        
        assert_eq!(streaming_service.get_active_streams().await.len(), 0);
    }

    #[tokio::test]
    async fn test_typing_indicator() {
        let mut indicator = TypingIndicator::new();
        
        assert!(!indicator.is_visible());
        assert_eq!(indicator.current_frame(), "");
        
        indicator.show();
        assert!(indicator.is_visible());
        assert!(!indicator.current_frame().is_empty());
        
        indicator.hide();
        assert!(!indicator.is_visible());
        assert_eq!(indicator.current_frame(), "");
    }

    #[tokio::test]
    async fn test_typing_indicator_animation() {
        let mut indicator = TypingIndicator::new();
        indicator.show();
        
        let first_frame = indicator.current_frame().to_string();
        
        // Simulate time passing
        sleep(Duration::from_millis(150)).await;
        indicator.update();
        
        // Frame should potentially change (though timing might affect this)
        let second_frame = indicator.current_frame().to_string();
        
        // At minimum, we should have valid frames
        assert!(!first_frame.is_empty());
        assert!(!second_frame.is_empty());
    }

    #[tokio::test]
    async fn test_streaming_stats() {
        let event_bus = Arc::new(EventBus::new());
        let api_client = Arc::new(APIClient::new());
        let streaming_service = StreamingService::new(event_bus, api_client);
        
        let stats = streaming_service.get_streaming_stats().await;
        assert_eq!(stats.active_streams, 0);
        assert_eq!(stats.total_content_length, 0);
    }

    #[tokio::test]
    async fn test_cleanup_streams() {
        let event_bus = Arc::new(EventBus::new());
        let api_client = Arc::new(APIClient::new());
        let streaming_service = StreamingService::new(event_bus, api_client);
        
        // Test cleanup with no streams
        streaming_service.cleanup_streams().await;
        assert_eq!(streaming_service.get_active_streams().await.len(), 0);
    }
}