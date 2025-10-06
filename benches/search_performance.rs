// Search performance benchmarks
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ruff::{
    message::manager::{MessageManager, Message, MessageRole},
    session::manager::SessionManager,
    search::tantivy_backend::TantivyMessageSearchIndex,
    message::search::MessageSearchQuery,
    events::EventBus,
};
use std::sync::Arc;
use tempfile::TempDir;

fn bench_index_message(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("index_message", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let search_dir = temp_dir.path().join("search");
            std::fs::create_dir_all(&search_dir).unwrap();

            let search_index = TantivyMessageSearchIndex::new(search_dir).unwrap();
            let event_bus = Arc::new(EventBus::new());
            let mut session_manager = SessionManager::new(event_bus);
            let session_id = session_manager.create_session(Some("Bench".to_string())).await.unwrap();

            let message = Message::new(session_id, MessageRole::User, "Test message for indexing".to_string());

            black_box(search_index.index_message(session_id, &message).await.unwrap());
        });
    });
}

fn bench_search_messages(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("search_messages");

    for corpus_size in [100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(corpus_size), corpus_size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let search_dir = temp_dir.path().join("search");
                std::fs::create_dir_all(&search_dir).unwrap();

                let search_index = TantivyMessageSearchIndex::new(search_dir).unwrap();
                let event_bus = Arc::new(EventBus::new());
                let mut session_manager = SessionManager::new(event_bus);
                let session_id = session_manager.create_session(Some("Bench".to_string())).await.unwrap();

                // Index corpus
                for i in 0..size {
                    let content = if i % 10 == 0 {
                        format!("Important message number {} about rust programming", i)
                    } else {
                        format!("Random message number {}", i)
                    };
                    let message = Message::new(session_id, MessageRole::User, content);
                    search_index.index_message(session_id, &message).await.unwrap();
                }

                search_index.commit().await.unwrap();

                // Benchmark search
                let query = MessageSearchQuery {
                    text: "rust programming".to_string(),
                    session_ids: Some(vec![session_id]),
                    roles: None,
                    date_range: None,
                    limit: 10,
                    include_metadata: true,
                };

                black_box(search_index.search(&query).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_bulk_index(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("bulk_index");

    for size in [50, 100, 200].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let search_dir = temp_dir.path().join("search");
                std::fs::create_dir_all(&search_dir).unwrap();

                let search_index = TantivyMessageSearchIndex::new(search_dir).unwrap();
                let event_bus = Arc::new(EventBus::new());
                let mut session_manager = SessionManager::new(event_bus);
                let session_id = session_manager.create_session(Some("Bench".to_string())).await.unwrap();

                for i in 0..size {
                    let message = Message::new(session_id, MessageRole::User, format!("Message {}", i));
                    search_index.index_message(session_id, &message).await.unwrap();
                }

                black_box(search_index.commit().await.unwrap());
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_index_message, bench_search_messages, bench_bulk_index);
criterion_main!(benches);
