use ruff::{
    events::EventBus,
    message::manager::{Message, MessageManager, MessageRole},
    session::manager::SessionManager,
    EnhancedError,
};
use tempfile::TempDir;

fn create_managers() -> (TempDir, SessionManager, MessageManager) {
    let temp_dir = TempDir::new().expect("create temporary test directory");
    let event_bus = EventBus::new();
    let session_manager = SessionManager::new(event_bus.clone(), temp_dir.path().join("sessions"));
    let message_manager = MessageManager::new(event_bus);

    (temp_dir, session_manager, message_manager)
}

fn create_test_message(content: impl Into<String>) -> Message {
    Message::new(MessageRole::User, content)
}

#[tokio::test]
async fn test_session_switching_no_message_mixing() -> Result<(), EnhancedError> {
    let (_temp_dir, mut session_manager, mut message_manager) = create_managers();
    let session_a = session_manager
        .create_session(Some("Session A".to_string()))
        .await?;
    let session_b = session_manager
        .create_session(Some("Session B".to_string()))
        .await?;

    message_manager
        .add_message(session_a, create_test_message("Message A1"))
        .await?;
    message_manager
        .add_message(session_a, create_test_message("Message A2"))
        .await?;
    message_manager
        .add_message(session_b, create_test_message("Message B1"))
        .await?;
    message_manager
        .add_message(session_b, create_test_message("Message B2"))
        .await?;

    session_manager.switch_session(session_a).await?;
    let messages_a = message_manager.get_session_messages_owned(session_a);
    assert_eq!(messages_a.len(), 2);
    assert!(messages_a
        .iter()
        .all(|m| m.content.starts_with("Message A")));

    session_manager.switch_session(session_b).await?;
    let messages_b = message_manager.get_session_messages_owned(session_b);
    assert_eq!(messages_b.len(), 2);
    assert!(messages_b
        .iter()
        .all(|m| m.content.starts_with("Message B")));

    assert_eq!(
        message_manager.get_session_messages_owned(session_a).len(),
        2
    );

    Ok(())
}

#[tokio::test]
async fn test_session_isolation() -> Result<(), EnhancedError> {
    let (_temp_dir, mut session_manager, mut message_manager) = create_managers();
    let session1 = session_manager
        .create_session(Some("Session 1".to_string()))
        .await?;
    let session2 = session_manager
        .create_session(Some("Session 2".to_string()))
        .await?;
    let session3 = session_manager
        .create_session(Some("Session 3".to_string()))
        .await?;

    for i in 1..=3 {
        message_manager
            .add_message(session1, create_test_message(format!("S1-M{}", i)))
            .await?;
    }
    for i in 1..=5 {
        message_manager
            .add_message(session2, create_test_message(format!("S2-M{}", i)))
            .await?;
    }
    for i in 1..=2 {
        message_manager
            .add_message(session3, create_test_message(format!("S3-M{}", i)))
            .await?;
    }

    let s1_messages = message_manager.get_session_messages_owned(session1);
    let s2_messages = message_manager.get_session_messages_owned(session2);
    let s3_messages = message_manager.get_session_messages_owned(session3);

    assert_eq!(s1_messages.len(), 3);
    assert_eq!(s2_messages.len(), 5);
    assert_eq!(s3_messages.len(), 2);
    assert!(s1_messages.iter().all(|m| m.content.starts_with("S1")));
    assert!(s2_messages.iter().all(|m| m.content.starts_with("S2")));
    assert!(s3_messages.iter().all(|m| m.content.starts_with("S3")));

    Ok(())
}

#[tokio::test]
async fn test_session_deletion_and_message_cleanup_are_explicit() -> Result<(), EnhancedError> {
    let (_temp_dir, mut session_manager, mut message_manager) = create_managers();
    let session_id = session_manager
        .create_session(Some("To Delete".to_string()))
        .await?;

    for i in 1..=5 {
        message_manager
            .add_message(session_id, create_test_message(format!("Message {}", i)))
            .await?;
    }

    assert_eq!(
        message_manager.get_session_messages_owned(session_id).len(),
        5
    );

    session_manager.delete_session(session_id).await?;
    assert!(session_manager.get_session(session_id).is_none());

    assert_eq!(
        message_manager.get_session_messages_owned(session_id).len(),
        5
    );
    message_manager.clear_session_messages(session_id);
    assert!(message_manager
        .get_session_messages_owned(session_id)
        .is_empty());

    Ok(())
}

#[tokio::test]
async fn test_concurrent_session_access() -> Result<(), EnhancedError> {
    let (_temp_dir, mut session_manager, message_manager) = create_managers();
    let message_manager = std::sync::Arc::new(tokio::sync::Mutex::new(message_manager));

    let session_a = session_manager
        .create_session(Some("Concurrent A".to_string()))
        .await?;
    let session_b = session_manager
        .create_session(Some("Concurrent B".to_string()))
        .await?;

    let msg_mgr_a = message_manager.clone();
    let task_a = tokio::spawn(async move {
        let mut mgr = msg_mgr_a.lock().await;
        for i in 1..=10 {
            mgr.add_message(session_a, create_test_message(format!("A{}", i)))
                .await
                .unwrap();
        }
    });

    let msg_mgr_b = message_manager.clone();
    let task_b = tokio::spawn(async move {
        let mut mgr = msg_mgr_b.lock().await;
        for i in 1..=10 {
            mgr.add_message(session_b, create_test_message(format!("B{}", i)))
                .await
                .unwrap();
        }
    });

    task_a.await.unwrap();
    task_b.await.unwrap();

    let mgr = message_manager.lock().await;
    let messages_a = mgr.get_session_messages_owned(session_a);
    let messages_b = mgr.get_session_messages_owned(session_b);

    assert_eq!(messages_a.len(), 10);
    assert_eq!(messages_b.len(), 10);
    assert!(messages_a.iter().all(|m| m.content.starts_with('A')));
    assert!(messages_b.iter().all(|m| m.content.starts_with('B')));

    Ok(())
}

#[tokio::test]
async fn test_session_list_and_search() -> Result<(), EnhancedError> {
    let (_temp_dir, mut session_manager, _message_manager) = create_managers();
    let s1 = session_manager
        .create_session(Some("Alpha Session".to_string()))
        .await?;
    session_manager
        .create_session(Some("Beta Session".to_string()))
        .await?;
    session_manager
        .create_session(Some("Gamma Session".to_string()))
        .await?;

    let sessions = session_manager
        .list_sessions(None, false, "created")
        .await?;
    assert_eq!(sessions.len(), 3);

    let alpha_results = session_manager
        .list_sessions(Some("Alpha"), false, "created")
        .await?;
    assert_eq!(alpha_results.len(), 1);
    assert_eq!(alpha_results[0].id, s1);

    Ok(())
}

#[tokio::test]
async fn test_session_archive_restore() -> Result<(), EnhancedError> {
    let (_temp_dir, mut session_manager, _message_manager) = create_managers();
    let session_id = session_manager
        .create_session(Some("Archive Test".to_string()))
        .await?;

    assert!(!session_manager.get_session(session_id).unwrap().is_archived);

    session_manager.archive_session(session_id).await?;
    assert!(session_manager.get_session(session_id).unwrap().is_archived);

    session_manager.restore_session(session_id).await?;
    assert!(!session_manager.get_session(session_id).unwrap().is_archived);

    Ok(())
}
