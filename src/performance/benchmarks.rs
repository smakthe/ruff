//! Performance benchmarks and optimization tests
//! 
//! This module provides comprehensive benchmarking capabilities to measure
//! and optimize performance across different components of the application.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::events::SessionId;
use crate::session::manager::{ChatSession, Message, MessageRole, MessageMetadata, ModelConfig};
// Lazy loading and virtual scrolling benchmarks would be implemented when modules are restructured
use crate::models::TokenUsage;
use crate::RuffError;

/// Performance benchmark suite
pub struct PerformanceBenchmarks {
    /// Benchmark results
    results: Arc<RwLock<HashMap<String, BenchmarkResult>>>,
    /// Configuration
    config: BenchmarkConfig,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of iterations for each benchmark
    pub iterations: usize,
    /// Warmup iterations (not counted in results)
    pub warmup_iterations: usize,
    /// Maximum time to spend on a single benchmark
    pub max_benchmark_time: Duration,
    /// Whether to run memory usage tests
    pub measure_memory: bool,
    /// Whether to run concurrent tests
    pub test_concurrency: bool,
}

/// Benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub total_time: Duration,
    pub average_time: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
    pub median_time: Duration,
    pub percentile_95: Duration,
    pub percentile_99: Duration,
    pub memory_usage: Option<MemoryUsage>,
    pub throughput: Option<f64>, // Operations per second
    pub error_count: usize,
    pub timestamp: DateTime<Local>,
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub peak_memory_bytes: usize,
    pub average_memory_bytes: usize,
    pub memory_allocations: usize,
    pub memory_deallocations: usize,
}

/// Benchmark test data
pub struct BenchmarkTestData {
    pub sessions: Vec<(SessionId, ChatSession)>,
    pub messages: HashMap<SessionId, Vec<Message>>,
    pub large_session_id: SessionId,
    pub small_session_id: SessionId,
}

impl PerformanceBenchmarks {
    /// Create a new performance benchmark suite
    pub fn new(config: BenchmarkConfig) -> Self {
        Self {
            results: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Run all benchmarks
    pub async fn run_all_benchmarks(&self) -> Result<HashMap<String, BenchmarkResult>, RuffError> {
        println!("Starting performance benchmarks...");
        
        // Generate test data
        let test_data = self.generate_test_data().await?;
        
        // Session management benchmarks
        self.benchmark_session_creation(&test_data).await?;
        self.benchmark_session_loading(&test_data).await?;
        self.benchmark_session_switching(&test_data).await?;
        
        // Message management benchmarks
        self.benchmark_message_operations(&test_data).await?;
        self.benchmark_message_search(&test_data).await?;
        
        // Lazy loading benchmarks
        self.benchmark_lazy_loading(&test_data).await?;
        
        // Virtual scrolling benchmarks
        self.benchmark_virtual_scrolling(&test_data).await?;
        
        // Search indexing benchmarks
        self.benchmark_search_indexing(&test_data).await?;
        
        // Memory usage benchmarks
        if self.config.measure_memory {
            self.benchmark_memory_usage(&test_data).await?;
        }
        
        // Concurrency benchmarks
        if self.config.test_concurrency {
            self.benchmark_concurrency(&test_data).await?;
        }
        
        println!("Benchmarks completed!");
        
        let results = self.results.read().unwrap();
        Ok(results.clone())
    }

    /// Generate test data for benchmarks
    async fn generate_test_data(&self) -> Result<BenchmarkTestData, RuffError> {
        println!("Generating test data...");
        
        let mut sessions = Vec::new();
        let mut messages = HashMap::new();
        
        // Create small session (10 messages)
        let small_session_id = Uuid::new_v4();
        let small_session = self.create_test_session(small_session_id, "Small Session", 10);
        let small_messages = self.create_test_messages(10);
        sessions.push((small_session_id, small_session));
        messages.insert(small_session_id, small_messages);
        
        // Create large session (10,000 messages)
        let large_session_id = Uuid::new_v4();
        let large_session = self.create_test_session(large_session_id, "Large Session", 10000);
        let large_messages = self.create_test_messages(10000);
        sessions.push((large_session_id, large_session));
        messages.insert(large_session_id, large_messages);
        
        // Create medium sessions (100 sessions with 100 messages each)
        for i in 0..100 {
            let session_id = Uuid::new_v4();
            let session = self.create_test_session(session_id, &format!("Session {}", i), 100);
            let session_messages = self.create_test_messages(100);
            sessions.push((session_id, session));
            messages.insert(session_id, session_messages);
        }
        
        Ok(BenchmarkTestData {
            sessions,
            messages,
            large_session_id,
            small_session_id,
        })
    }

    /// Create a test session
    fn create_test_session(&self, id: SessionId, title: &str, message_count: u32) -> ChatSession {
        ChatSession {
            id,
            title: title.to_string(),
            created_at: Local::now(),
            updated_at: Local::now(),
            messages: Vec::new(), // Messages stored separately for benchmarking
            model: "test-model".to_string(),
            system_prompt: None,
            model_config: ModelConfig::default(),
            total_tokens_used: TokenUsage {
                input_tokens: message_count * 10,
                output_tokens: message_count * 20,
                total_tokens: message_count * 30,
            },
            tags: vec!["benchmark".to_string(), "test".to_string()],
            is_archived: false,
            export_count: 0,
            message_count,
            last_activity: Local::now(),
        }
    }

    /// Create test messages
    fn create_test_messages(&self, count: usize) -> Vec<Message> {
        (0..count)
            .map(|i| Message {
                id: Uuid::new_v4(),
                role: if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant },
                content: format!("This is test message number {} with some content to make it realistic for benchmarking purposes.", i),
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
            })
            .collect()
    }

    /// Benchmark session creation
    async fn benchmark_session_creation(&self, _test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        self.run_benchmark("session_creation", |_| async {
            // Simulate session creation
            let _session_id = Uuid::new_v4();
            let _session = ChatSession {
                id: _session_id,
                title: "Benchmark Session".to_string(),
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
            };
            Ok(())
        }).await
    }

    /// Benchmark session loading
    async fn benchmark_session_loading(&self, test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        let sessions = test_data.sessions.clone();
        
        self.run_benchmark("session_loading", move |_| {
            let sessions = sessions.clone();
            async move {
                // Simulate loading sessions from storage
                for (_session_id, session) in &sessions {
                    let _serialized = serde_json::to_string(session)
                        .map_err(|e| RuffError::App(e.to_string()))?;
                    let _deserialized: ChatSession = serde_json::from_str(&_serialized)
                        .map_err(|e| RuffError::App(e.to_string()))?;
                }
                Ok(())
            }
        }).await
    }

    /// Benchmark session switching
    async fn benchmark_session_switching(&self, test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        let session_ids: Vec<SessionId> = test_data.sessions.iter().map(|(id, _)| *id).collect();
        
        self.run_benchmark("session_switching", move |iteration| {
            let session_ids = session_ids.clone();
            async move {
                // Simulate switching between sessions
                let _session_id = session_ids[iteration % session_ids.len()];
                // Simulate session switch overhead
                tokio::task::yield_now().await;
                Ok(())
            }
        }).await
    }

    /// Benchmark message operations
    async fn benchmark_message_operations(&self, test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        let messages = test_data.messages.get(&test_data.small_session_id).unwrap().clone();
        
        self.run_benchmark("message_operations", move |iteration| {
            let messages = messages.clone();
            async move {
                // Simulate message operations
                let message = &messages[iteration % messages.len()];
                
                // Simulate message editing
                let _edited_content = format!("{} (edited)", message.content);
                
                // Simulate message serialization
                let _serialized = serde_json::to_string(message)
                    .map_err(|e| RuffError::App(e.to_string()))?;
                
                Ok(())
            }
        }).await
    }

    /// Benchmark message search
    async fn benchmark_message_search(&self, test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        let all_messages: Vec<Message> = test_data.messages.values().flatten().cloned().collect();
        
        self.run_benchmark("message_search", move |iteration| {
            let all_messages = all_messages.clone();
            async move {
                // Simulate message search
                let search_term = format!("message {}", iteration % 100);
                let _results: Vec<&Message> = all_messages
                    .iter()
                    .filter(|msg| msg.content.contains(&search_term))
                    .collect();
                Ok(())
            }
        }).await
    }

    /// Benchmark lazy loading
    async fn benchmark_lazy_loading(&self, test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        let large_messages = test_data.messages.get(&test_data.large_session_id).unwrap().clone();
        
        self.run_benchmark("lazy_loading", move |iteration| {
            let large_messages = large_messages.clone();
            async move {
                // Simulate lazy loading of message chunks
                let chunk_size = 100;
                let start_index = (iteration * chunk_size) % large_messages.len();
                let end_index = (start_index + chunk_size).min(large_messages.len());
                
                let _chunk = &large_messages[start_index..end_index];
                
                // Simulate chunk processing
                tokio::task::yield_now().await;
                Ok(())
            }
        }).await
    }

    /// Benchmark virtual scrolling
    async fn benchmark_virtual_scrolling(&self, _test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        self.run_benchmark("virtual_scrolling", |iteration| async move {
            // Simulate virtual scrolling calculations
            let viewport_height = 20;
            let total_messages = 10000;
            let scroll_position = iteration % (total_messages - viewport_height);
            
            // Simulate visibility calculations
            let first_visible = scroll_position;
            let last_visible = (scroll_position + viewport_height).min(total_messages);
            
            // Simulate rendering calculations
            for i in first_visible..last_visible {
                let _message_y = (i - first_visible) * 3; // 3 lines per message
            }
            
            Ok(())
        }).await
    }

    /// Benchmark search indexing
    async fn benchmark_search_indexing(&self, test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        let sessions = test_data.sessions.clone();
        
        self.run_benchmark("search_indexing", move |_| {
            let sessions = sessions.clone();
            async move {
                // Simulate search indexing
                for (session_id, session) in &sessions {
                    // Simulate indexing session metadata
                    let _index_key = format!("session:{}", session_id);
                    let _index_content = format!("{} {}", session.title, session.model);
                    
                    // Simulate tokenization and indexing
                    let _tokens: Vec<&str> = _index_content.split_whitespace().collect();
                }
                Ok(())
            }
        }).await
    }

    /// Benchmark memory usage
    async fn benchmark_memory_usage(&self, test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        let test_data_clone = test_data.clone();
        self.run_benchmark("memory_usage", move |_| {
            let test_data = test_data_clone.clone();
            async move {
                // Simulate memory-intensive operations
                let mut large_data = Vec::new();
                
                // Allocate memory for sessions
                for (_, session) in &test_data.sessions {
                    large_data.push(session.clone());
                }
                
                // Simulate memory usage for messages (convert to string for uniform type)
                let mut message_data = Vec::new();
                for messages in test_data.messages.values() {
                    for message in messages {
                        message_data.push(message.content.clone());
                    }
                }
                
                // Simulate processing
                let _processed_count = large_data.len() + message_data.len();
                
                // Clear memory
                large_data.clear();
                message_data.clear();
                
                Ok(())
            }
        }).await
    }

    /// Benchmark concurrency
    async fn benchmark_concurrency(&self, test_data: &BenchmarkTestData) -> Result<(), RuffError> {
        let sessions = test_data.sessions.clone();
        
        self.run_benchmark("concurrency", move |_| {
            let sessions = sessions.clone();
            async move {
                // Simulate concurrent operations
                let tasks: Vec<_> = sessions
                    .iter()
                    .take(10) // Limit to 10 concurrent tasks
                    .map(|(session_id, session)| {
                        let session_id = *session_id;
                        let session = session.clone();
                        tokio::spawn(async move {
                            // Simulate concurrent session processing
                            let _serialized = serde_json::to_string(&session).unwrap();
                            tokio::task::yield_now().await;
                            session_id
                        })
                    })
                    .collect();
                
                // Wait for all tasks to complete
                for task in tasks {
                    let _ = task.await;
                }
                
                Ok(())
            }
        }).await
    }

    /// Run a benchmark with the given function
    async fn run_benchmark<F, Fut>(&self, name: &str, benchmark_fn: F) -> Result<(), RuffError>
    where
        F: Fn(usize) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), RuffError>> + Send,
    {
        println!("Running benchmark: {}", name);
        
        let mut times = Vec::new();
        let mut error_count = 0;
        let start_time = Instant::now();
        
        // Warmup iterations
        for i in 0..self.config.warmup_iterations {
            if let Err(_) = benchmark_fn(i).await {
                error_count += 1;
            }
        }
        
        // Actual benchmark iterations
        for i in 0..self.config.iterations {
            let iteration_start = Instant::now();
            
            match benchmark_fn(i).await {
                Ok(()) => {
                    let duration = iteration_start.elapsed();
                    times.push(duration);
                }
                Err(_) => {
                    error_count += 1;
                }
            }
            
            // Check if we've exceeded the maximum benchmark time
            if start_time.elapsed() > self.config.max_benchmark_time {
                println!("Benchmark {} exceeded maximum time, stopping early", name);
                break;
            }
        }
        
        if times.is_empty() {
            return Err(RuffError::App(format!("Benchmark {} produced no valid results", name)));
        }
        
        // Calculate statistics
        times.sort();
        let total_time: Duration = times.iter().sum();
        let average_time = total_time / times.len() as u32;
        let min_time = *times.first().unwrap();
        let max_time = *times.last().unwrap();
        let median_time = times[times.len() / 2];
        let percentile_95 = times[(times.len() as f64 * 0.95) as usize];
        let percentile_99 = times[(times.len() as f64 * 0.99) as usize];
        
        let throughput = if average_time.as_nanos() > 0 {
            Some(1_000_000_000.0 / average_time.as_nanos() as f64)
        } else {
            None
        };
        
        let result = BenchmarkResult {
            name: name.to_string(),
            iterations: times.len(),
            total_time,
            average_time,
            min_time,
            max_time,
            median_time,
            percentile_95,
            percentile_99,
            memory_usage: None, // Would need actual memory profiling
            throughput,
            error_count,
            timestamp: Local::now(),
        };
        
        // Store result
        {
            let mut results = self.results.write().unwrap();
            results.insert(name.to_string(), result.clone());
        }
        
        println!(
            "Benchmark {} completed: avg={:?}, min={:?}, max={:?}, errors={}",
            name, average_time, min_time, max_time, error_count
        );
        
        Ok(())
    }

    /// Get benchmark results
    pub fn get_results(&self) -> HashMap<String, BenchmarkResult> {
        let results = self.results.read().unwrap();
        results.clone()
    }

    /// Generate performance report
    pub fn generate_report(&self) -> String {
        let results = self.results.read().unwrap();
        let mut report = String::new();
        
        report.push_str("# Performance Benchmark Report\n\n");
        report.push_str(&format!("Generated at: {}\n\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
        
        for (name, result) in results.iter() {
            report.push_str(&format!("## {}\n", name));
            report.push_str(&format!("- Iterations: {}\n", result.iterations));
            report.push_str(&format!("- Average time: {:?}\n", result.average_time));
            report.push_str(&format!("- Min time: {:?}\n", result.min_time));
            report.push_str(&format!("- Max time: {:?}\n", result.max_time));
            report.push_str(&format!("- Median time: {:?}\n", result.median_time));
            report.push_str(&format!("- 95th percentile: {:?}\n", result.percentile_95));
            report.push_str(&format!("- 99th percentile: {:?}\n", result.percentile_99));
            
            if let Some(throughput) = result.throughput {
                report.push_str(&format!("- Throughput: {:.2} ops/sec\n", throughput));
            }
            
            if result.error_count > 0 {
                report.push_str(&format!("- Errors: {}\n", result.error_count));
            }
            
            report.push_str("\n");
        }
        
        report
    }

    /// Save benchmark results to file
    pub async fn save_results(&self, file_path: &str) -> Result<(), RuffError> {
        let results = self.get_results();
        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| RuffError::App(format!("Failed to serialize results: {}", e)))?;
        
        tokio::fs::write(file_path, json).await
            .map_err(|e| RuffError::App(format!("Failed to write results file: {}", e)))?;
        
        Ok(())
    }
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            warmup_iterations: 10,
            max_benchmark_time: Duration::from_secs(60),
            measure_memory: false,
            test_concurrency: true,
        }
    }
}

impl Clone for BenchmarkTestData {
    fn clone(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
            messages: self.messages.clone(),
            large_session_id: self.large_session_id,
            small_session_id: self.small_session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_benchmark_creation() {
        let config = BenchmarkConfig::default();
        let benchmarks = PerformanceBenchmarks::new(config);
        
        let results = benchmarks.get_results();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_simple_benchmark() {
        let config = BenchmarkConfig {
            iterations: 5,
            warmup_iterations: 1,
            max_benchmark_time: Duration::from_secs(10),
            measure_memory: false,
            test_concurrency: false,
        };
        
        let benchmarks = PerformanceBenchmarks::new(config);
        
        // Run a simple benchmark
        benchmarks.run_benchmark("test_benchmark", |_| async {
            sleep(Duration::from_millis(1)).await;
            Ok(())
        }).await.unwrap();
        
        let results = benchmarks.get_results();
        assert_eq!(results.len(), 1);
        assert!(results.contains_key("test_benchmark"));
        
        let result = &results["test_benchmark"];
        assert_eq!(result.iterations, 5);
        assert!(result.average_time >= Duration::from_millis(1));
    }

    #[tokio::test]
    async fn test_benchmark_with_errors() {
        let config = BenchmarkConfig {
            iterations: 5,
            warmup_iterations: 0,
            max_benchmark_time: Duration::from_secs(10),
            measure_memory: false,
            test_concurrency: false,
        };
        
        let benchmarks = PerformanceBenchmarks::new(config);
        
        // Run a benchmark that sometimes fails
        benchmarks.run_benchmark("error_benchmark", |iteration| async move {
            if iteration % 2 == 0 {
                Err(RuffError::App("Test error".to_string()))
            } else {
                Ok(())
            }
        }).await.unwrap();
        
        let results = benchmarks.get_results();
        let result = &results["error_benchmark"];
        assert!(result.error_count > 0);
        assert!(result.iterations < 5); // Some iterations failed
    }

    #[tokio::test]
    async fn test_generate_report() {
        let config = BenchmarkConfig {
            iterations: 3,
            warmup_iterations: 0,
            max_benchmark_time: Duration::from_secs(10),
            measure_memory: false,
            test_concurrency: false,
        };
        
        let benchmarks = PerformanceBenchmarks::new(config);
        
        benchmarks.run_benchmark("test1", |_| async { Ok(()) }).await.unwrap();
        benchmarks.run_benchmark("test2", |_| async { Ok(()) }).await.unwrap();
        
        let report = benchmarks.generate_report();
        assert!(report.contains("Performance Benchmark Report"));
        assert!(report.contains("test1"));
        assert!(report.contains("test2"));
        assert!(report.contains("Average time"));
    }
}