use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruff::{
    events::EventBus,
    message::manager::MessageManager,
    session::manager::{Message, MessageRole, SessionManager},
};
use tempfile::TempDir;

fn create_managers(temp_dir: &TempDir) -> (SessionManager, MessageManager) {
    let event_bus = EventBus::new();
    let session_manager = SessionManager::new(event_bus.clone(), temp_dir.path().join("sessions"));
    let message_manager = MessageManager::new(event_bus);

    (session_manager, message_manager)
}

fn bench_add_message(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("add_message", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let (mut session_manager, mut message_manager) = create_managers(&temp_dir);
            let session_id = session_manager
                .create_session(Some("Bench".to_string()))
                .await
                .unwrap();
            let message = Message::new(MessageRole::User, "Test message");

            black_box(
                message_manager
                    .add_message(session_id, message)
                    .await
                    .unwrap(),
            );
        });
    });
}

fn bench_get_messages(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("get_messages");

    for size in [10, 100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let (mut session_manager, mut message_manager) = create_managers(&temp_dir);
                let session_id = session_manager
                    .create_session(Some("Bench".to_string()))
                    .await
                    .unwrap();

                for i in 0..size {
                    let msg = Message::new(MessageRole::User, format!("Message {}", i));
                    message_manager.add_message(session_id, msg).await.unwrap();
                }

                black_box(message_manager.get_session_messages_owned(session_id));
            });
        });
    }

    group.finish();
}

fn bench_edit_message(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("edit_message", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let (mut session_manager, mut message_manager) = create_managers(&temp_dir);
            let session_id = session_manager
                .create_session(Some("Bench".to_string()))
                .await
                .unwrap();
            let message = Message::new(MessageRole::User, "Original content");
            let message_id = message_manager
                .add_message(session_id, message)
                .await
                .unwrap();

            black_box(
                message_manager
                    .edit_message(session_id, message_id, "Updated content".to_string())
                    .await
                    .unwrap(),
            );
        });
    });
}

criterion_group!(
    benches,
    bench_add_message,
    bench_get_messages,
    bench_edit_message
);
criterion_main!(benches);
