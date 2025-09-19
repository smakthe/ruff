use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use crate::models::TokenUsage;

pub type SessionId = Uuid;
pub type MessageId = Uuid;
pub type PluginId = String;
pub type ThemeId = String;

/// Events that can occur throughout the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    // Session events
    SessionCreated(SessionId),
    SessionSwitched(SessionId),
    SessionUpdated(SessionId),
    SessionRenamed { id: SessionId, old_title: String, new_title: String },
    SessionDeleted(SessionId),
    SessionArchived(SessionId),
    SessionRestored(SessionId),
    
    // Message events
    MessageAdded { session_id: SessionId, message_id: MessageId },
    MessageEdited { session_id: SessionId, message_id: MessageId },
    MessageDeleted { session_id: SessionId, message_id: MessageId },
    MessageRegenerated { session_id: SessionId, old_message_id: MessageId, new_message_id: MessageId },
    
    // UI events
    ThemeChanged(ThemeId),
    FontSizeChanged(u16),
    LayoutChanged,
    PaneResized { pane_id: String, new_size: (u16, u16) },
    
    // Search events
    SearchRequested(SearchQuery),
    SearchCompleted { query: SearchQuery, result_count: usize },
    
    // Export/Import events
    ExportRequested { session_id: Option<SessionId>, format: ExportFormat },
    ExportCompleted { session_id: Option<SessionId>, format: ExportFormat, file_path: String },
    ImportRequested { file_path: String, format: ImportFormat },
    ImportCompleted { imported_sessions: Vec<SessionId> },
    SessionImported { session_id: SessionId, source_format: String },
    
    // Plugin events
    PluginLoaded(PluginId),
    PluginUnloaded(PluginId),
    PluginError { plugin_id: PluginId, error: String },
    UIExtensionRegistered { plugin_id: PluginId, extension_id: String },
    UIExtensionUnregistered { plugin_id: PluginId, extension_id: String },
    
    // Configuration events
    ConfigurationChanged,
    ModelConfigChanged(String),
    
    // API events
    ApiRequestStarted { model: String, tokens: u32 },
    ApiRequestCompleted { model: String, token_usage: TokenUsage, response_time_ms: u64 },
    ApiRequestFailed { model: String, error: String },
    
    // Streaming events
    StreamingStarted { session_id: SessionId, message_id: MessageId },
    StreamingChunk { session_id: SessionId, message_id: MessageId, content: String },
    StreamingCompleted { session_id: SessionId, message_id: MessageId, token_usage: Option<TokenUsage> },
    StreamingCancelled { session_id: SessionId, message_id: MessageId },
    StreamingError { session_id: SessionId, message_id: MessageId, error: String },
    
    // Application lifecycle events
    ApplicationStarted,
    ApplicationShutdown,
    
    // Performance and indexing events
    SessionIndexed(SessionId),
    MessageIndexed { session_id: SessionId, message_id: MessageId },
    IndexRebuilt,
    IndexOptimized,
    
    // Error events
    ErrorOccurred { error: String, context: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub session_ids: Option<Vec<SessionId>>,
    pub date_range: Option<(DateTime<Local>, DateTime<Local>)>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Markdown,
    PlainText,
    Json,
    Html,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportFormat {
    Json,
    ChatGptExport,
    ClaudeExport,
}

/// Event handler function type
pub type EventHandler = Arc<dyn Fn(&AppEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync>;

/// Async event handler function type
pub type AsyncEventHandler = Arc<dyn Fn(AppEvent) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync>;

/// Event subscription handle for unsubscribing
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionHandle(Uuid);

/// Event bus for application-wide event communication
pub struct EventBus {
    sync_handlers: Arc<RwLock<HashMap<SubscriptionHandle, EventHandler>>>,
    async_handlers: Arc<RwLock<HashMap<SubscriptionHandle, AsyncEventHandler>>>,
    event_sender: mpsc::UnboundedSender<AppEvent>,
    _event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<AppEvent>>>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            sync_handlers: Arc::new(RwLock::new(HashMap::new())),
            async_handlers: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            _event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
        }
    }
    
    /// Subscribe to events with a synchronous handler
    pub async fn subscribe<F>(&self, handler: F) -> SubscriptionHandle
    where
        F: Fn(&AppEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync + 'static,
    {
        let handle = SubscriptionHandle(Uuid::new_v4());
        let mut handlers = self.sync_handlers.write().await;
        handlers.insert(handle.clone(), Arc::new(handler));
        handle
    }
    
    /// Subscribe to events with an asynchronous handler
    pub async fn subscribe_async<F, Fut>(&self, handler: F) -> SubscriptionHandle
    where
        F: Fn(AppEvent) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    {
        let handle = SubscriptionHandle(Uuid::new_v4());
        let async_handler: AsyncEventHandler = Arc::new(move |event| {
            Box::pin(handler(event))
        });
        
        let mut handlers = self.async_handlers.write().await;
        handlers.insert(handle.clone(), async_handler);
        handle
    }
    
    /// Unsubscribe from events
    pub async fn unsubscribe(&self, handle: SubscriptionHandle) -> bool {
        let mut sync_handlers = self.sync_handlers.write().await;
        let mut async_handlers = self.async_handlers.write().await;
        
        sync_handlers.remove(&handle).is_some() || async_handlers.remove(&handle).is_some()
    }
    
    /// Publish an event to all subscribers
    pub async fn publish(&self, event: AppEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Send to async processing queue
        self.event_sender.send(event.clone()).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        
        // Process synchronous handlers immediately
        let sync_handlers = self.sync_handlers.read().await;
        for handler in sync_handlers.values() {
            if let Err(e) = handler(&event) {
                eprintln!("Error in sync event handler: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// Start the async event processing loop
    pub async fn start_processing(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut receiver = {
            let mut receiver_guard = self._event_receiver.write().await;
            receiver_guard.take().ok_or("Event processing already started")?
        };
        
        let async_handlers = Arc::clone(&self.async_handlers);
        
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                let handlers = async_handlers.read().await;
                
                // Process all async handlers concurrently
                let futures: Vec<_> = handlers.values().map(|handler| {
                    let event_clone = event.clone();
                    let handler_clone = Arc::clone(handler);
                    tokio::spawn(async move {
                        if let Err(e) = handler_clone(event_clone).await {
                            eprintln!("Error in async event handler: {}", e);
                        }
                    })
                }).collect();
                
                // Wait for all handlers to complete
                for future in futures {
                    let _ = future.await;
                }
            }
        });
        
        Ok(())
    }
    
    /// Get the number of active subscriptions
    pub async fn subscription_count(&self) -> usize {
        let sync_count = self.sync_handlers.read().await.len();
        let async_count = self.async_handlers.read().await.len();
        sync_count + async_count
    }
    
    /// Clear all subscriptions
    pub async fn clear_subscriptions(&self) {
        self.sync_handlers.write().await.clear();
        self.async_handlers.write().await.clear();
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_event_bus_creation() {
        let event_bus = EventBus::new();
        assert_eq!(event_bus.subscription_count().await, 0);
    }

    #[tokio::test]
    async fn test_sync_event_subscription() {
        let event_bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        
        let handle = event_bus.subscribe(move |_event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }).await;
        
        assert_eq!(event_bus.subscription_count().await, 1);
        
        // Publish an event
        let session_id = Uuid::new_v4();
        event_bus.publish(AppEvent::SessionCreated(session_id)).await.unwrap();
        
        // Give a moment for processing
        sleep(Duration::from_millis(10)).await;
        
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        
        // Unsubscribe
        assert!(event_bus.unsubscribe(handle).await);
        assert_eq!(event_bus.subscription_count().await, 0);
    }

    #[tokio::test]
    async fn test_async_event_subscription() {
        let event_bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        
        let handle = event_bus.subscribe_async(move |_event| {
            let counter = Arc::clone(&counter_clone);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }).await;
        
        assert_eq!(event_bus.subscription_count().await, 1);
        
        // Start event processing
        event_bus.start_processing().await.unwrap();
        
        // Publish an event
        let session_id = Uuid::new_v4();
        event_bus.publish(AppEvent::SessionCreated(session_id)).await.unwrap();
        
        // Give time for async processing
        sleep(Duration::from_millis(50)).await;
        
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        
        // Unsubscribe
        assert!(event_bus.unsubscribe(handle).await);
        assert_eq!(event_bus.subscription_count().await, 0);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let event_bus = EventBus::new();
        let counter1 = Arc::new(AtomicUsize::new(0));
        let counter2 = Arc::new(AtomicUsize::new(0));
        
        let counter1_clone = Arc::clone(&counter1);
        let counter2_clone = Arc::clone(&counter2);
        
        let handle1 = event_bus.subscribe(move |_event| {
            counter1_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }).await;
        
        let handle2 = event_bus.subscribe_async(move |_event| {
            let counter = Arc::clone(&counter2_clone);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }).await;
        
        assert_eq!(event_bus.subscription_count().await, 2);
        
        // Start event processing
        event_bus.start_processing().await.unwrap();
        
        // Publish an event
        let session_id = Uuid::new_v4();
        event_bus.publish(AppEvent::SessionCreated(session_id)).await.unwrap();
        
        // Give time for processing
        sleep(Duration::from_millis(50)).await;
        
        assert_eq!(counter1.load(Ordering::SeqCst), 1);
        assert_eq!(counter2.load(Ordering::SeqCst), 1);
        
        // Clean up
        event_bus.unsubscribe(handle1).await;
        event_bus.unsubscribe(handle2).await;
    }

    #[tokio::test]
    async fn test_event_types() {
        let event_bus = EventBus::new();
        let received_events = Arc::new(RwLock::new(Vec::new()));
        let events_clone = Arc::clone(&received_events);
        
        let _handle = event_bus.subscribe(move |event| {
            let events = Arc::clone(&events_clone);
            let event_clone = event.clone();
            tokio::spawn(async move {
                events.write().await.push(event_clone);
            });
            Ok(())
        }).await;
        
        // Test various event types
        let session_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        
        let test_events = vec![
            AppEvent::SessionCreated(session_id),
            AppEvent::MessageAdded { session_id, message_id },
            AppEvent::ThemeChanged("dark".to_string()),
            AppEvent::SearchRequested(SearchQuery {
                text: "test".to_string(),
                session_ids: None,
                date_range: None,
                limit: 10,
            }),
        ];
        
        for event in &test_events {
            event_bus.publish(event.clone()).await.unwrap();
        }
        
        // Give time for processing
        sleep(Duration::from_millis(50)).await;
        
        let received = received_events.read().await;
        assert_eq!(received.len(), test_events.len());
    }

    #[tokio::test]
    async fn test_clear_subscriptions() {
        let event_bus = EventBus::new();
        
        let _handle1 = event_bus.subscribe(|_| Ok(())).await;
        let _handle2 = event_bus.subscribe_async(|_| async { Ok(()) }).await;
        
        assert_eq!(event_bus.subscription_count().await, 2);
        
        event_bus.clear_subscriptions().await;
        assert_eq!(event_bus.subscription_count().await, 0);
    }
}