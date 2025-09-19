//! Background indexing system for search functionality
//! 
//! This module provides background indexing capabilities to maintain
//! search indices without blocking the main application thread.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::events::{EventBus, AppEvent, SessionId, MessageId};
use crate::session::manager::{ChatSession, Message};
use crate::search::index::SearchIndex;
use crate::RuffError;

/// Background indexing service
pub struct BackgroundIndexer {
    /// Command channel sender
    command_sender: mpsc::UnboundedSender<IndexCommand>,
    /// Indexing statistics
    stats: Arc<RwLock<IndexingStats>>,
    /// Configuration
    config: IndexingConfig,
}

/// Indexing worker that runs in the background
pub struct IndexingWorker {
    /// Command channel receiver
    command_receiver: mpsc::UnboundedReceiver<IndexCommand>,
    /// Search index
    search_index: Arc<RwLock<SearchIndex>>,
    /// Event bus for notifications
    event_bus: EventBus,
    /// Indexing queue
    indexing_queue: Arc<Mutex<VecDeque<IndexTask>>>,
    /// Statistics
    stats: Arc<RwLock<IndexingStats>>,
    /// Configuration
    config: IndexingConfig,
    /// Current indexing state
    state: IndexingState,
}

/// Indexing command
#[derive(Debug)]
pub enum IndexCommand {
    /// Index a session
    IndexSession(SessionId, ChatSession),
    /// Index a message
    IndexMessage(SessionId, MessageId, Message),
    /// Remove session from index
    RemoveSession(SessionId),
    /// Remove message from index
    RemoveMessage(SessionId, MessageId),
    /// Update session in index
    UpdateSession(SessionId, ChatSession),
    /// Update message in index
    UpdateMessage(SessionId, MessageId, Message),
    /// Rebuild entire index
    RebuildIndex(Vec<(SessionId, ChatSession)>),
    /// Optimize index
    OptimizeIndex,
    /// Get indexing status
    GetStatus(oneshot::Sender<IndexingStatus>),
    /// Pause indexing
    Pause,
    /// Resume indexing
    Resume,
    /// Shutdown indexer
    Shutdown,
}

/// Indexing task
#[derive(Debug, Clone)]
pub struct IndexTask {
    pub id: Uuid,
    pub task_type: IndexTaskType,
    pub priority: IndexPriority,
    pub created_at: DateTime<Local>,
    pub session_id: SessionId,
    pub data: IndexTaskData,
}

/// Type of indexing task
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexTaskType {
    IndexSession,
    IndexMessage,
    RemoveSession,
    RemoveMessage,
    UpdateSession,
    UpdateMessage,
    RebuildIndex,
    OptimizeIndex,
}

/// Task priority
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndexPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Task data
#[derive(Debug, Clone)]
pub enum IndexTaskData {
    Session(ChatSession),
    Message(MessageId, Message),
    SessionList(Vec<(SessionId, ChatSession)>),
    MessageId(MessageId),
    None,
}

/// Indexing configuration
#[derive(Debug, Clone)]
pub struct IndexingConfig {
    /// Maximum number of tasks to process per batch
    pub batch_size: usize,
    /// Delay between batches
    pub batch_delay: Duration,
    /// Maximum queue size
    pub max_queue_size: usize,
    /// Enable incremental indexing
    pub incremental_indexing: bool,
    /// Index optimization interval
    pub optimization_interval: Duration,
    /// Maximum indexing time per task
    pub max_task_time: Duration,
}

/// Indexing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingStats {
    pub total_tasks_processed: u64,
    pub sessions_indexed: u64,
    pub messages_indexed: u64,
    pub indexing_errors: u64,
    pub average_task_time: Duration,
    pub last_optimization: Option<DateTime<Local>>,
    pub queue_size: usize,
    pub is_paused: bool,
    pub is_running: bool,
}

/// Current indexing status
#[derive(Debug, Clone)]
pub struct IndexingStatus {
    pub stats: IndexingStats,
    pub current_task: Option<IndexTask>,
    pub queue_length: usize,
    pub estimated_completion: Option<DateTime<Local>>,
}

/// Indexing state
#[derive(Debug, Clone)]
struct IndexingState {
    is_paused: bool,
    is_running: bool,
    current_task: Option<IndexTask>,
    last_optimization: Option<DateTime<Local>>,
}

impl BackgroundIndexer {
    /// Create a new background indexer
    pub fn new(
        search_index: Arc<RwLock<SearchIndex>>,
        event_bus: EventBus,
        config: IndexingConfig,
    ) -> Self {
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        let stats = Arc::new(RwLock::new(IndexingStats::default()));

        // Spawn the indexing worker
        let worker = IndexingWorker::new(
            command_receiver,
            search_index,
            event_bus,
            Arc::clone(&stats),
            config.clone(),
        );

        tokio::spawn(async move {
            worker.run().await;
        });

        Self {
            command_sender,
            stats,
            config,
        }
    }

    /// Index a session
    pub fn index_session(&self, session_id: SessionId, session: ChatSession) -> Result<(), RuffError> {
        self.send_command(IndexCommand::IndexSession(session_id, session))
    }

    /// Index a message
    pub fn index_message(&self, session_id: SessionId, message_id: MessageId, message: Message) -> Result<(), RuffError> {
        self.send_command(IndexCommand::IndexMessage(session_id, message_id, message))
    }

    /// Remove session from index
    pub fn remove_session(&self, session_id: SessionId) -> Result<(), RuffError> {
        self.send_command(IndexCommand::RemoveSession(session_id))
    }

    /// Remove message from index
    pub fn remove_message(&self, session_id: SessionId, message_id: MessageId) -> Result<(), RuffError> {
        self.send_command(IndexCommand::RemoveMessage(session_id, message_id))
    }

    /// Update session in index
    pub fn update_session(&self, session_id: SessionId, session: ChatSession) -> Result<(), RuffError> {
        self.send_command(IndexCommand::UpdateSession(session_id, session))
    }

    /// Update message in index
    pub fn update_message(&self, session_id: SessionId, message_id: MessageId, message: Message) -> Result<(), RuffError> {
        self.send_command(IndexCommand::UpdateMessage(session_id, message_id, message))
    }

    /// Rebuild entire index
    pub fn rebuild_index(&self, sessions: Vec<(SessionId, ChatSession)>) -> Result<(), RuffError> {
        self.send_command(IndexCommand::RebuildIndex(sessions))
    }

    /// Optimize index
    pub fn optimize_index(&self) -> Result<(), RuffError> {
        self.send_command(IndexCommand::OptimizeIndex)
    }

    /// Get indexing status
    pub async fn get_status(&self) -> Result<IndexingStatus, RuffError> {
        let (sender, receiver) = oneshot::channel();
        self.send_command(IndexCommand::GetStatus(sender))?;
        
        receiver.await
            .map_err(|e| RuffError::App(format!("Failed to get indexing status: {}", e)))
    }

    /// Pause indexing
    pub fn pause(&self) -> Result<(), RuffError> {
        self.send_command(IndexCommand::Pause)
    }

    /// Resume indexing
    pub fn resume(&self) -> Result<(), RuffError> {
        self.send_command(IndexCommand::Resume)
    }

    /// Shutdown indexer
    pub fn shutdown(&self) -> Result<(), RuffError> {
        self.send_command(IndexCommand::Shutdown)
    }

    /// Get current statistics
    pub fn get_stats(&self) -> IndexingStats {
        let stats = self.stats.read().unwrap();
        stats.clone()
    }

    /// Send command to worker
    fn send_command(&self, command: IndexCommand) -> Result<(), RuffError> {
        self.command_sender.send(command)
            .map_err(|e| RuffError::App(format!("Failed to send indexing command: {}", e)))
    }
}

impl IndexingWorker {
    /// Create a new indexing worker
    fn new(
        command_receiver: mpsc::UnboundedReceiver<IndexCommand>,
        search_index: Arc<RwLock<SearchIndex>>,
        event_bus: EventBus,
        stats: Arc<RwLock<IndexingStats>>,
        config: IndexingConfig,
    ) -> Self {
        Self {
            command_receiver,
            search_index,
            event_bus,
            indexing_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats,
            config,
            state: IndexingState {
                is_paused: false,
                is_running: true,
                current_task: None,
                last_optimization: None,
            },
        }
    }

    /// Run the indexing worker
    async fn run(mut self) {
        let mut optimization_timer = tokio::time::interval(self.config.optimization_interval);
        
        loop {
            tokio::select! {
                // Handle commands
                command = self.command_receiver.recv() => {
                    match command {
                        Some(cmd) => {
                            if let Err(e) = self.handle_command(cmd).await {
                                eprintln!("Indexing error: {}", e);
                                self.increment_error_count();
                            }
                        }
                        None => {
                            // Channel closed, shutdown
                            break;
                        }
                    }
                }
                
                // Process indexing queue
                _ = sleep(self.config.batch_delay) => {
                    if !self.state.is_paused && self.state.is_running {
                        if let Err(e) = self.process_queue_batch().await {
                            eprintln!("Queue processing error: {}", e);
                            self.increment_error_count();
                        }
                    }
                }
                
                // Periodic optimization
                _ = optimization_timer.tick() => {
                    if !self.state.is_paused && self.state.is_running {
                        if let Err(e) = self.optimize_index().await {
                            eprintln!("Index optimization error: {}", e);
                            self.increment_error_count();
                        }
                    }
                }
            }
        }
    }

    /// Handle a command
    async fn handle_command(&mut self, command: IndexCommand) -> Result<(), RuffError> {
        match command {
            IndexCommand::IndexSession(session_id, session) => {
                self.queue_task(IndexTask {
                    id: Uuid::new_v4(),
                    task_type: IndexTaskType::IndexSession,
                    priority: IndexPriority::Normal,
                    created_at: Local::now(),
                    session_id,
                    data: IndexTaskData::Session(session),
                }).await?;
            }
            
            IndexCommand::IndexMessage(session_id, message_id, message) => {
                self.queue_task(IndexTask {
                    id: Uuid::new_v4(),
                    task_type: IndexTaskType::IndexMessage,
                    priority: IndexPriority::High, // Messages are high priority
                    created_at: Local::now(),
                    session_id,
                    data: IndexTaskData::Message(message_id, message),
                }).await?;
            }
            
            IndexCommand::RemoveSession(session_id) => {
                self.queue_task(IndexTask {
                    id: Uuid::new_v4(),
                    task_type: IndexTaskType::RemoveSession,
                    priority: IndexPriority::High,
                    created_at: Local::now(),
                    session_id,
                    data: IndexTaskData::None,
                }).await?;
            }
            
            IndexCommand::RemoveMessage(session_id, message_id) => {
                self.queue_task(IndexTask {
                    id: Uuid::new_v4(),
                    task_type: IndexTaskType::RemoveMessage,
                    priority: IndexPriority::High,
                    created_at: Local::now(),
                    session_id,
                    data: IndexTaskData::MessageId(message_id),
                }).await?;
            }
            
            IndexCommand::UpdateSession(session_id, session) => {
                self.queue_task(IndexTask {
                    id: Uuid::new_v4(),
                    task_type: IndexTaskType::UpdateSession,
                    priority: IndexPriority::Normal,
                    created_at: Local::now(),
                    session_id,
                    data: IndexTaskData::Session(session),
                }).await?;
            }
            
            IndexCommand::UpdateMessage(session_id, message_id, message) => {
                self.queue_task(IndexTask {
                    id: Uuid::new_v4(),
                    task_type: IndexTaskType::UpdateMessage,
                    priority: IndexPriority::High,
                    created_at: Local::now(),
                    session_id,
                    data: IndexTaskData::Message(message_id, message),
                }).await?;
            }
            
            IndexCommand::RebuildIndex(sessions) => {
                self.queue_task(IndexTask {
                    id: Uuid::new_v4(),
                    task_type: IndexTaskType::RebuildIndex,
                    priority: IndexPriority::Critical,
                    created_at: Local::now(),
                    session_id: Uuid::nil(), // Not session-specific
                    data: IndexTaskData::SessionList(sessions),
                }).await?;
            }
            
            IndexCommand::OptimizeIndex => {
                self.queue_task(IndexTask {
                    id: Uuid::new_v4(),
                    task_type: IndexTaskType::OptimizeIndex,
                    priority: IndexPriority::Low,
                    created_at: Local::now(),
                    session_id: Uuid::nil(),
                    data: IndexTaskData::None,
                }).await?;
            }
            
            IndexCommand::GetStatus(sender) => {
                let status = self.get_current_status().await;
                let _ = sender.send(status);
            }
            
            IndexCommand::Pause => {
                self.state.is_paused = true;
                self.update_stats_pause_state(true);
            }
            
            IndexCommand::Resume => {
                self.state.is_paused = false;
                self.update_stats_pause_state(false);
            }
            
            IndexCommand::Shutdown => {
                self.state.is_running = false;
                self.update_stats_running_state(false);
            }
        }
        
        Ok(())
    }

    /// Queue a task for processing
    async fn queue_task(&self, task: IndexTask) -> Result<(), RuffError> {
        let mut queue = self.indexing_queue.lock().unwrap();
        
        // Check queue size limit
        if queue.len() >= self.config.max_queue_size {
            return Err(RuffError::App("Indexing queue is full".to_string()));
        }
        
        // Insert task in priority order
        let insert_pos = queue
            .iter()
            .position(|existing_task| existing_task.priority < task.priority)
            .unwrap_or(queue.len());
        
        queue.insert(insert_pos, task);
        
        // Update queue size in stats
        self.update_queue_size(queue.len());
        
        Ok(())
    }

    /// Process a batch of tasks from the queue
    async fn process_queue_batch(&mut self) -> Result<(), RuffError> {
        let tasks_to_process = {
            let mut queue = self.indexing_queue.lock().unwrap();
            let batch_size = self.config.batch_size.min(queue.len());
            
            if batch_size == 0 {
                return Ok(());
            }
            
            let mut tasks = Vec::with_capacity(batch_size);
            for _ in 0..batch_size {
                if let Some(task) = queue.pop_front() {
                    tasks.push(task);
                }
            }
            
            // Update queue size in stats
            self.update_queue_size(queue.len());
            
            tasks
        };

        for task in tasks_to_process {
            let start_time = Instant::now();
            self.state.current_task = Some(task.clone());
            
            let result = tokio::time::timeout(
                self.config.max_task_time,
                self.process_task(task.clone())
            ).await;
            
            match result {
                Ok(Ok(())) => {
                    let duration = start_time.elapsed();
                    self.update_task_completion(duration);
                }
                Ok(Err(e)) => {
                    eprintln!("Task processing error: {}", e);
                    self.increment_error_count();
                }
                Err(_) => {
                    eprintln!("Task timeout: {:?}", task.task_type);
                    self.increment_error_count();
                }
            }
            
            self.state.current_task = None;
        }

        Ok(())
    }

    /// Process a single indexing task
    async fn process_task(&mut self, task: IndexTask) -> Result<(), RuffError> {
        match task.task_type {
            IndexTaskType::IndexSession => {
                if let IndexTaskData::Session(session) = task.data {
                    self.index_session_impl(task.session_id, &session).await?;
                }
            }
            
            IndexTaskType::IndexMessage => {
                if let IndexTaskData::Message(message_id, message) = task.data {
                    self.index_message_impl(task.session_id, message_id, &message).await?;
                }
            }
            
            IndexTaskType::RemoveSession => {
                self.remove_session_impl(task.session_id).await?;
            }
            
            IndexTaskType::RemoveMessage => {
                if let IndexTaskData::MessageId(message_id) = task.data {
                    self.remove_message_impl(task.session_id, message_id).await?;
                }
            }
            
            IndexTaskType::UpdateSession => {
                if let IndexTaskData::Session(session) = task.data {
                    self.update_session_impl(task.session_id, &session).await?;
                }
            }
            
            IndexTaskType::UpdateMessage => {
                if let IndexTaskData::Message(message_id, message) = task.data {
                    self.update_message_impl(task.session_id, message_id, &message).await?;
                }
            }
            
            IndexTaskType::RebuildIndex => {
                if let IndexTaskData::SessionList(sessions) = task.data {
                    self.rebuild_index_impl(sessions).await?;
                }
            }
            
            IndexTaskType::OptimizeIndex => {
                self.optimize_index_impl().await?;
            }
        }
        
        Ok(())
    }

    /// Implementation for indexing a session
    async fn index_session_impl(&self, session_id: SessionId, _session: &ChatSession) -> Result<(), RuffError> {
        // This would integrate with the actual search index implementation
        // For now, just simulate the work
        tokio::task::yield_now().await;
        
        // Publish event
        self.event_bus.publish(AppEvent::SessionIndexed(session_id)).await
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        self.increment_sessions_indexed();
        Ok(())
    }

    /// Implementation for indexing a message
    async fn index_message_impl(&self, session_id: SessionId, message_id: MessageId, _message: &Message) -> Result<(), RuffError> {
        // This would integrate with the actual search index implementation
        tokio::task::yield_now().await;
        
        // Publish event
        self.event_bus.publish(AppEvent::MessageIndexed { session_id, message_id }).await
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        self.increment_messages_indexed();
        Ok(())
    }

    /// Implementation for removing a session from index
    async fn remove_session_impl(&self, _session_id: SessionId) -> Result<(), RuffError> {
        tokio::task::yield_now().await;
        Ok(())
    }

    /// Implementation for removing a message from index
    async fn remove_message_impl(&self, _session_id: SessionId, _message_id: MessageId) -> Result<(), RuffError> {
        tokio::task::yield_now().await;
        Ok(())
    }

    /// Implementation for updating a session in index
    async fn update_session_impl(&self, session_id: SessionId, session: &ChatSession) -> Result<(), RuffError> {
        // Remove old version and add new version
        self.remove_session_impl(session_id).await?;
        self.index_session_impl(session_id, session).await?;
        Ok(())
    }

    /// Implementation for updating a message in index
    async fn update_message_impl(&self, session_id: SessionId, message_id: MessageId, message: &Message) -> Result<(), RuffError> {
        // Remove old version and add new version
        self.remove_message_impl(session_id, message_id).await?;
        self.index_message_impl(session_id, message_id, message).await?;
        Ok(())
    }

    /// Implementation for rebuilding the entire index
    async fn rebuild_index_impl(&self, sessions: Vec<(SessionId, ChatSession)>) -> Result<(), RuffError> {
        // Clear existing index
        {
            let _search_index = self.search_index.write().unwrap();
            // search_index.clear(); // Would call actual clear method
        }
        
        // Re-index all sessions
        for (session_id, session) in sessions {
            self.index_session_impl(session_id, &session).await?;
        }
        
        // Publish event
        self.event_bus.publish(AppEvent::IndexRebuilt).await
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        Ok(())
    }

    /// Implementation for optimizing the index
    async fn optimize_index_impl(&mut self) -> Result<(), RuffError> {
        tokio::task::yield_now().await;
        
        self.state.last_optimization = Some(Local::now());
        self.update_last_optimization(Local::now());
        
        Ok(())
    }

    /// Optimize index (called periodically)
    async fn optimize_index(&mut self) -> Result<(), RuffError> {
        self.optimize_index_impl().await
    }

    /// Get current indexing status
    async fn get_current_status(&self) -> IndexingStatus {
        let stats = {
            let stats = self.stats.read().unwrap();
            stats.clone()
        };
        
        let queue_length = {
            let queue = self.indexing_queue.lock().unwrap();
            queue.len()
        };
        
        IndexingStatus {
            stats,
            current_task: self.state.current_task.clone(),
            queue_length,
            estimated_completion: None, // Would calculate based on queue size and average task time
        }
    }

    /// Update statistics - increment error count
    fn increment_error_count(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.indexing_errors += 1;
    }

    /// Update statistics - increment sessions indexed
    fn increment_sessions_indexed(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.sessions_indexed += 1;
        stats.total_tasks_processed += 1;
    }

    /// Update statistics - increment messages indexed
    fn increment_messages_indexed(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.messages_indexed += 1;
        stats.total_tasks_processed += 1;
    }

    /// Update statistics - task completion
    fn update_task_completion(&self, duration: Duration) {
        let mut stats = self.stats.write().unwrap();
        stats.total_tasks_processed += 1;
        
        // Update average task time (simple moving average)
        let total_time = stats.average_task_time.as_nanos() as f64 * (stats.total_tasks_processed - 1) as f64;
        let new_average = (total_time + duration.as_nanos() as f64) / stats.total_tasks_processed as f64;
        stats.average_task_time = Duration::from_nanos(new_average as u64);
    }

    /// Update statistics - queue size
    fn update_queue_size(&self, size: usize) {
        let mut stats = self.stats.write().unwrap();
        stats.queue_size = size;
    }

    /// Update statistics - pause state
    fn update_stats_pause_state(&self, is_paused: bool) {
        let mut stats = self.stats.write().unwrap();
        stats.is_paused = is_paused;
    }

    /// Update statistics - running state
    fn update_stats_running_state(&self, is_running: bool) {
        let mut stats = self.stats.write().unwrap();
        stats.is_running = is_running;
    }

    /// Update statistics - last optimization
    fn update_last_optimization(&self, timestamp: DateTime<Local>) {
        let mut stats = self.stats.write().unwrap();
        stats.last_optimization = Some(timestamp);
    }
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            batch_delay: Duration::from_millis(100),
            max_queue_size: 1000,
            incremental_indexing: true,
            optimization_interval: Duration::from_secs(300), // 5 minutes
            max_task_time: Duration::from_secs(30),
        }
    }
}

impl Default for IndexingStats {
    fn default() -> Self {
        Self {
            total_tasks_processed: 0,
            sessions_indexed: 0,
            messages_indexed: 0,
            indexing_errors: 0,
            average_task_time: Duration::from_millis(0),
            last_optimization: None,
            queue_size: 0,
            is_paused: false,
            is_running: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::search::index::SearchIndex;
    use crate::session::manager::{MessageRole, MessageMetadata};
    use crate::models::TokenUsage;

    fn create_test_session() -> ChatSession {
        use crate::session::manager::{ChatSession, ModelConfig};
        
        ChatSession {
            id: Uuid::new_v4(),
            title: "Test Session".to_string(),
            created_at: Local::now(),
            updated_at: Local::now(),
            messages: Vec::new(),
            model: "test-model".to_string(),
            system_prompt: None,
            model_config: ModelConfig::default(),
            total_tokens_used: TokenUsage::default(),
            tags: Vec::new(),
            is_archived: false,
            export_count: 0,
            message_count: 0,
            last_activity: Local::now(),
        }
    }

    fn create_test_message() -> Message {
        Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "Test message".to_string(),
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
    async fn test_background_indexer_creation() {
        let search_index = Arc::new(RwLock::new(SearchIndex::new()));
        let event_bus = EventBus::new();
        let config = IndexingConfig::default();
        
        let indexer = BackgroundIndexer::new(search_index, event_bus, config);
        let stats = indexer.get_stats();
        
        assert_eq!(stats.total_tasks_processed, 0);
        assert!(stats.is_running);
        assert!(!stats.is_paused);
    }

    #[tokio::test]
    async fn test_index_session_command() {
        let search_index = Arc::new(RwLock::new(SearchIndex::new()));
        let event_bus = EventBus::new();
        let config = IndexingConfig::default();
        
        let indexer = BackgroundIndexer::new(search_index, event_bus, config);
        let session_id = Uuid::new_v4();
        let session = create_test_session();
        
        let result = indexer.index_session(session_id, session);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_index_message_command() {
        let search_index = Arc::new(RwLock::new(SearchIndex::new()));
        let event_bus = EventBus::new();
        let config = IndexingConfig::default();
        
        let indexer = BackgroundIndexer::new(search_index, event_bus, config);
        let session_id = Uuid::new_v4();
        let message = create_test_message();
        let message_id = message.id;
        
        let result = indexer.index_message(session_id, message_id, message);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pause_resume_commands() {
        let search_index = Arc::new(RwLock::new(SearchIndex::new()));
        let event_bus = EventBus::new();
        let config = IndexingConfig::default();
        
        let indexer = BackgroundIndexer::new(search_index, event_bus, config);
        
        // Test pause
        let result = indexer.pause();
        assert!(result.is_ok());
        
        // Test resume
        let result = indexer.resume();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_status() {
        let search_index = Arc::new(RwLock::new(SearchIndex::new()));
        let event_bus = EventBus::new();
        let config = IndexingConfig::default();
        
        let indexer = BackgroundIndexer::new(search_index, event_bus, config);
        
        // Give the worker a moment to start
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let status = indexer.get_status().await;
        assert!(status.is_ok());
        
        let status = status.unwrap();
        assert!(status.stats.is_running);
        assert!(!status.stats.is_paused);
    }

    #[tokio::test]
    async fn test_task_priority_ordering() {
        let task1 = IndexTask {
            id: Uuid::new_v4(),
            task_type: IndexTaskType::IndexSession,
            priority: IndexPriority::Low,
            created_at: Local::now(),
            session_id: Uuid::new_v4(),
            data: IndexTaskData::None,
        };
        
        let task2 = IndexTask {
            id: Uuid::new_v4(),
            task_type: IndexTaskType::IndexMessage,
            priority: IndexPriority::High,
            created_at: Local::now(),
            session_id: Uuid::new_v4(),
            data: IndexTaskData::None,
        };
        
        let task3 = IndexTask {
            id: Uuid::new_v4(),
            task_type: IndexTaskType::RebuildIndex,
            priority: IndexPriority::Critical,
            created_at: Local::now(),
            session_id: Uuid::new_v4(),
            data: IndexTaskData::None,
        };
        
        // Test priority ordering
        assert!(task3.priority > task2.priority);
        assert!(task2.priority > task1.priority);
    }
}