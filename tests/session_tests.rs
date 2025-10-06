use ruff::{
    session::manager::SessionManager,
    message::manager::{MessageManager, Message, MessageRole},
    events::EventBus,
    EnhancedError,
};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Local;

#[tokio::test]
async fn test_session_switching_no_message_mixing() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus.clone());

    // Create two sessions
    let session_a = session_manager.create_session(Some("Session A".to_string())).await?;
    let session_b = session_manager.create_session(Some("Session B".to_string())).await?;

    // Add distinct messages to each
    let msg_a1 = create_test_message(session_a, "Message A1");
    let msg_a2 = create_test_message(session_a, "Message A2");
    let msg_b1 = create_test_message(session_b, "Message B1");
    let msg_b2 = create_test_message(session_b, "Message B2");

    message_manager.add_message(session_a, msg_a1).await?;
    message_manager.add_message(session_a, msg_a2).await?;
    message_manager.add_message(session_b, msg_b1).await?;
    message_manager.add_message(session_b, msg_b2).await?;

    // Switch to session A
    session_manager.switch_session(session_a).await?;
    let messages_a = message_manager.get_session_messages_owned(session_a);

    // VERIFY: Only session A messages
    assert_eq!(messages_a.len(), 2);
    assert!(messages_a.iter().all(|m| m.content.starts_with("Message A")));

    // Switch to session B
    session_manager.switch_session(session_b).await?;
    let messages_b = message_manager.get_session_messages_owned(session_b);

    // VERIFY: Only session B messages
    assert_eq!(messages_b.len(), 2);
    assert!(messages_b.iter().all(|m| m.content.starts_with("Message B")));

    // VERIFY: Session A messages unchanged
    let messages_a_again = message_manager.get_session_messages_owned(session_a);
    assert_eq!(messages_a_again.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_session_isolation() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus.clone());

    // Create multiple sessions
    let session1 = session_manager.create_session(Some("Session 1".to_string())).await?;
    let session2 = session_manager.create_session(Some("Session 2".to_string())).await?;
    let session3 = session_manager.create_session(Some("Session 3".to_string())).await?;

    // Add messages to each
    for i in 1..=3 {
        let msg = create_test_message(session1, &format!("S1-M{}", i));
        message_manager.add_message(session1, msg).await?;
    }

    for i in 1..=5 {
        let msg = create_test_message(session2, &format!("S2-M{}", i));
        message_manager.add_message(session2, msg).await?;
    }

    for i in 1..=2 {
        let msg = create_test_message(session3, &format!("S3-M{}", i));
        message_manager.add_message(session3, msg).await?;
    }

    // VERIFY: Each session has correct count
    let s1_messages = message_manager.get_session_messages_owned(session1);
    let s2_messages = message_manager.get_session_messages_owned(session2);
    let s3_messages = message_manager.get_session_messages_owned(session3);

    assert_eq!(s1_messages.len(), 3);
    assert_eq!(s2_messages.len(), 5);
    assert_eq!(s3_messages.len(), 2);

    // VERIFY: No message cross-contamination
    assert!(s1_messages.iter().all(|m| m.content.starts_with("S1")));
    assert!(s2_messages.iter().all(|m| m.content.starts_with("S2")));
    assert!(s3_messages.iter().all(|m| m.content.starts_with("S3")));

    Ok(())
}

#[tokio::test]
async fn test_session_deletion_removes_messages() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus.clone());

    // Create session with messages
    let session_id = session_manager.create_session(Some("To Delete".to_string())).await?;

    for i in 1..=5 {
        let msg = create_test_message(session_id, &format!("Message {}", i));
        message_manager.add_message(session_id, msg).await?;
    }

    // VERIFY: Messages exist
    let messages_before = message_manager.get_session_messages_owned(session_id);
    assert_eq!(messages_before.len(), 5);

    // Delete session
    session_manager.delete_session(session_id).await?;

    // VERIFY: Session is gone
    assert!(session_manager.get_session(session_id).is_none());

    // VERIFY: Messages are gone
    let messages_after = message_manager.get_session_messages_owned(session_id);
    assert_eq!(messages_after.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_concurrent_session_access() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let message_manager = Arc::new(tokio::sync::Mutex::new(
        MessageManager::new(event_bus.clone())
    ));

    // Create sessions
    let session_a = session_manager.create_session(Some("Concurrent A".to_string())).await?;
    let session_b = session_manager.create_session(Some("Concurrent B".to_string())).await?;

    // Spawn concurrent tasks to add messages
    let msg_mgr_a = message_manager.clone();
    let task_a = tokio::spawn(async move {
        let mut mgr = msg_mgr_a.lock().await;
        for i in 1..=10 {
            let msg = create_test_message(session_a, &format!("A{}", i));
            mgr.add_message(session_a, msg).await.unwrap();
        }
    });

    let msg_mgr_b = message_manager.clone();
    let task_b = tokio::spawn(async move {
        let mut mgr = msg_mgr_b.lock().await;
        for i in 1..=10 {
            let msg = create_test_message(session_b, &format!("B{}", i));
            mgr.add_message(session_b, msg).await.unwrap();
        }
    });

    // Wait for both to complete
    task_a.await.unwrap();
    task_b.await.unwrap();

    // VERIFY: Both sessions have correct counts
    let mgr = message_manager.lock().await;
    let messages_a = mgr.get_session_messages_owned(session_a);
    let messages_b = mgr.get_session_messages_owned(session_b);

    assert_eq!(messages_a.len(), 10);
    assert_eq!(messages_b.len(), 10);

    // VERIFY: No mixing
    assert!(messages_a.iter().all(|m| m.content.starts_with("A")));
    assert!(messages_b.iter().all(|m| m.content.starts_with("B")));

    Ok(())
}

#[tokio::test]
async fn test_session_list_and_search() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;

    // Create multiple sessions
    let s1 = session_manager.create_session(Some("Alpha Session".to_string())).await?;
    let s2 = session_manager.create_session(Some("Beta Session".to_string())).await?;
    let s3 = session_manager.create_session(Some("Gamma Session".to_string())).await?;

    // VERIFY: All sessions listed
    let sessions = session_manager.list_sessions(None, false, "created").await?;
    assert_eq!(sessions.len(), 3);

    // VERIFY: Search by title works
    let alpha_results = session_manager.list_sessions(Some("Alpha"), false, "created").await?;
    assert_eq!(alpha_results.len(), 1);
    assert_eq!(alpha_results[0].id, s1);

    Ok(())
}

#[tokio::test]
async fn test_session_archive_unarchive() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;

    // Create session
    let session_id = session_manager.create_session(Some("Archive Test".to_string())).await?;

    // VERIFY: Initially not archived
    let session = session_manager.get_session(session_id).unwrap();
    assert!(!session.is_archived);

    // Archive
    session_manager.archive_session(session_id, true).await?;

    // VERIFY: Now archived
    let session = session_manager.get_session(session_id).unwrap();
    assert!(session.is_archived);

    // Unarchive
    session_manager.archive_session(session_id, false).await?;

    // VERIFY: Not archived again
    let session = session_manager.get_session(session_id).unwrap();
    assert!(!session.is_archived);

    Ok(())
}

// Helper function
fn create_test_message(session_id: Uuid, content: &str) -> Message {
    Message {
        id: Uuid::new_v4(),
        session_id,
        role: MessageRole::User,
        content: content.to_string(),
        timestamp: Local::now(),
        edited_at: None,
        token_usage: None,
        parent_id: None,
        children: vec![],
        metadata: Default::default(),
    }
}
