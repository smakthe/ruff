// Message operation benchmarks
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ruff::{message::manager::MessageManager, session::manager::{SessionManager, Message, MessageRole}, events::EventBus};
use std::sync::Arc;
use tempfile::TempDir;

fn bench_add_message(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("add_message", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let event_bus = Arc::new(EventBus::new());
            let mut message_manager = MessageManager::new(event_bus.clone(), temp_dir.path().to_path_buf());
            let mut session_manager = SessionManager::new(event_bus);

            let session_id = session_manager.create_session(Some("Bench".to_string())).await.unwrap();
            let message = Message::new(session_id, MessageRole::User, "Test message".to_string());

            black_box(message_manager.add_message(session_id, message).await.unwrap());
        });
    });
}

fn bench_get_messages(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("get_messages");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let event_bus = Arc::new(EventBus::new());
                let mut message_manager = MessageManager::new(event_bus.clone(), temp_dir.path().to_path_buf());
                let mut session_manager = SessionManager::new(event_bus);

                let session_id = session_manager.create_session(Some("Bench".to_string())).await.unwrap();

                for i in 0..size {
                    let msg = Message::new(session_id, MessageRole::User, format!("Message {}", i));
                    message_manager.add_message(session_id, msg.clone()).await.unwrap();
                }

                // Benchmark: get all messages
                black_box(message_manager.get_session_messages_owned(session_id));
            });
        });
    }

    group.finish();
}

fn bench_update_message(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("update_message", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let event_bus = Arc::new(EventBus::new());
            let mut message_manager = MessageManager::new(event_bus.clone(), temp_dir.path().to_path_buf());
            let mut session_manager = SessionManager::new(event_bus);

            let session_id = session_manager.create_session(Some("Bench".to_string())).await.unwrap();
            let message = Message::new(session_id, MessageRole::User, "Original content".to_string());
            let message_id = message_manager.add_message(session_id, message).await.unwrap();

            let mut updated_message = message_manager.get_message(session_id, message_id).unwrap().clone();
            updated_message.content = "Updated content".to_string();

            black_box(message_manager.update_message(session_id, updated_message).await.unwrap());
        });
    });
}

criterion_group!(benches, bench_add_message, bench_get_messages, bench_update_message);
criterion_main!(benches);
