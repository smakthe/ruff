use ruff::{
    session::manager::SessionManager,
    message::manager::{MessageManager, Message, MessageRole},
    events::{EventBus, SessionId},
    export::{ExportService, ExportFormat, SessionExportRequest, ExportOptions},
    EnhancedError,
};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Local;

#[tokio::test]
async fn test_messages_not_stored_in_session() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let event_bus_clone = (*event_bus).clone();
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus_clone);

    // Create session
    let session_id = session_manager.create_session(Some("Test Session".to_string())).await?;

    // Add messages via MessageManager
    let message1 = create_test_message(session_id, "Hello");
    let message2 = create_test_message(session_id, "World");

    message_manager.add_message(session_id, message1).await?;
    message_manager.add_message(session_id, message2).await?;

    // Get session
    let session = session_manager.get_session(session_id)
        .ok_or_else(|| EnhancedError::session("Session not found"))?;

    // VERIFY: MessageManager has the messages
    let messages = message_manager.get_session_messages_owned(session_id);
    assert_eq!(messages.len(), 2, "Should have 2 messages");
    assert_eq!(messages[0].content, "Hello");
    assert_eq!(messages[1].content, "World");

    // VERIFY: Session metadata is correct
    assert_eq!(session.message_count, 2, "Session message count should be 2");

    Ok(())
}

#[tokio::test]
async fn test_export_uses_message_manager() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let event_bus_clone = (*event_bus).clone();
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus_clone);

    let export_dir = std::env::temp_dir().join("ruff_test_exports");
    std::fs::create_dir_all(&export_dir).ok();
    let export_service = ExportService::new(export_dir.clone());

    // Create session with messages
    let session_id = session_manager.create_session(Some("Export Test".to_string())).await?;

    let message = create_test_message(session_id, "Test content");
    let message_id = message.id;
    message_manager.add_message(session_id, message.clone()).await?;

    // Modify message in MessageManager (edit_message takes just content string)
    message_manager.edit_message(session_id, message_id, "Updated content".to_string()).await?;

    // Export session
    let session = session_manager.get_session(session_id).unwrap();
    let request = SessionExportRequest {
        session_id,
        format: ExportFormat::Markdown,
        options: ExportOptions::default(),
        output_path: None,
    };

    let result = export_service.export_session(session, request, &message_manager)?;

    // Read exported file
    let exported_content = std::fs::read_to_string(&result.file_path)?;

    // VERIFY: Exported content has UPDATED message, not original
    assert!(exported_content.contains("Updated content"), "Export should contain updated content");

    // Cleanup
    std::fs::remove_file(&result.file_path).ok();
    std::fs::remove_dir_all(&export_dir).ok();

    Ok(())
}

#[tokio::test]
async fn test_session_file_has_no_embedded_messages() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let event_bus_clone = (*event_bus).clone();
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus_clone);

    // Create session with messages
    let session_id = session_manager.create_session(Some("Persistence Test".to_string())).await?;

    let message = create_test_message(session_id, "Test message");
    message_manager.add_message(session_id, message).await?;

    // Save session
    session_manager.save_session(session_id).await?;

    // Read session file directly
    let session_file = dirs::data_dir().unwrap()
        .join("ruff/sessions")
        .join(format!("{}.json", session_id));

    if let Ok(file_content) = std::fs::read_to_string(session_file) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&file_content) {
            // VERIFY: No "messages" field in JSON
            assert!(json.get("messages").is_none(), "Session file should NOT contain 'messages' field");

            // VERIFY: Has message_count field
            assert!(json.get("message_count").is_some(), "Session should have message_count");
            assert_eq!(json["message_count"], 1);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_message_operations_independent() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let event_bus_clone = (*event_bus).clone();
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus_clone);

    // Create session
    let session_id = session_manager.create_session(Some("Independence Test".to_string())).await?;

    // Add message
    let message = create_test_message(session_id, "Original");
    let message_id = message.id;
    message_manager.add_message(session_id, message).await?;

    // Get session before edit
    let session_before = session_manager.get_session(session_id).unwrap();
    assert_eq!(session_before.message_count, 1);

    // Edit message (takes content string, not full message)
    message_manager.edit_message(session_id, message_id, "Edited".to_string()).await?;

    // Get session after edit
    let session_after = session_manager.get_session(session_id).unwrap();
    assert_eq!(session_after.message_count, 1, "Count should not change after edit");

    // Get message to verify edit
    let messages = message_manager.get_session_messages_owned(session_id);
    assert_eq!(messages[0].content, "Edited", "Message should be edited");

    // Delete message (takes delete_subsequent boolean)
    message_manager.delete_message(session_id, message_id, false).await?;

    // Get session after delete
    let session_final = session_manager.get_session(session_id).unwrap();
    assert_eq!(session_final.message_count, 0, "Count should be 0 after delete");

    // Verify message deleted
    let messages_final = message_manager.get_session_messages_owned(session_id);
    assert_eq!(messages_final.len(), 0, "Should have no messages after delete");

    Ok(())
}

// Helper function
fn create_test_message(session_id: SessionId, content: &str) -> Message {
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
