use ruff::{
    events::EventBus,
    export::{ExportFormat, ExportOptions, ExportService, SessionExportRequest},
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

fn create_test_message(content: &str) -> Message {
    Message::new(MessageRole::User, content)
}

#[tokio::test]
async fn test_message_manager_persists_messages_by_session() -> Result<(), EnhancedError> {
    let temp_dir = TempDir::new().expect("create temporary test directory");
    let session_id = uuid::Uuid::new_v4();
    let storage_path = temp_dir.path().join("messages");

    let mut writer = MessageManager::new_with_storage(EventBus::new(), storage_path.clone())?;
    writer
        .add_message(session_id, create_test_message("Persist me"))
        .await?;

    let reader = MessageManager::new_with_storage(EventBus::new(), storage_path.clone())?;
    let messages = reader.get_session_messages_owned(session_id);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Persist me");

    let mut cleaner = MessageManager::new_with_storage(EventBus::new(), storage_path.clone())?;
    cleaner.clear_session_messages(session_id);

    let reader = MessageManager::new_with_storage(EventBus::new(), storage_path)?;
    assert!(reader.get_session_messages_owned(session_id).is_empty());

    Ok(())
}

#[tokio::test]
async fn test_messages_are_stored_outside_session_metadata() -> Result<(), EnhancedError> {
    let (_temp_dir, mut session_manager, mut message_manager) = create_managers();
    let session_id = session_manager
        .create_session(Some("Test Session".to_string()))
        .await?;

    message_manager
        .add_message(session_id, create_test_message("Hello"))
        .await?;
    message_manager
        .add_message(session_id, create_test_message("World"))
        .await?;
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    let session = session_manager
        .get_session(session_id)
        .ok_or_else(|| EnhancedError::session("Session not found"))?;
    let messages = message_manager.get_session_messages_owned(session_id);

    assert_eq!(messages.len(), 2);
    let contents: Vec<&str> = messages
        .iter()
        .map(|message| message.content.as_str())
        .collect();
    assert!(contents.contains(&"Hello"));
    assert!(contents.contains(&"World"));
    assert_eq!(session.message_count, 2);

    Ok(())
}

#[tokio::test]
async fn test_export_reads_current_message_manager_state() -> Result<(), EnhancedError> {
    let (temp_dir, mut session_manager, mut message_manager) = create_managers();
    let export_dir = temp_dir.path().join("exports");
    let export_service = ExportService::new(export_dir);
    export_service.initialize()?;

    let session_id = session_manager
        .create_session(Some("Export Test".to_string()))
        .await?;
    let message_id = message_manager
        .add_message(session_id, create_test_message("Test content"))
        .await?;

    message_manager
        .edit_message(session_id, message_id, "Updated content".to_string())
        .await?;
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    let session = session_manager.get_session(session_id).unwrap();
    let result = export_service.export_session(
        session,
        SessionExportRequest {
            session_id,
            format: ExportFormat::Markdown,
            options: ExportOptions::default(),
            output_path: None,
        },
        &message_manager,
    )?;

    let exported_content = std::fs::read_to_string(&result.file_path)
        .map_err(|e| EnhancedError::storage(format!("Failed to read export: {}", e)))?;

    assert!(exported_content.contains("Updated content"));
    assert!(!exported_content.contains("Test content"));

    Ok(())
}

#[tokio::test]
async fn test_session_file_has_no_embedded_messages() -> Result<(), EnhancedError> {
    let (temp_dir, mut session_manager, mut message_manager) = create_managers();
    let session_id = session_manager
        .create_session(Some("Persistence Test".to_string()))
        .await?;

    message_manager
        .add_message(session_id, create_test_message("Test message"))
        .await?;
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    let session_file = temp_dir
        .path()
        .join("sessions")
        .join(format!("{}.json", session_id));
    let file_content = std::fs::read_to_string(session_file)
        .map_err(|e| EnhancedError::storage(format!("Failed to read session file: {}", e)))?;
    let json: serde_json::Value = serde_json::from_str(&file_content)?;

    assert!(json.get("messages").is_none());
    assert_eq!(json["message_count"], 1);

    Ok(())
}

#[tokio::test]
async fn test_message_operations_update_session_metadata_explicitly() -> Result<(), EnhancedError> {
    let (_temp_dir, mut session_manager, mut message_manager) = create_managers();
    let session_id = session_manager
        .create_session(Some("Independence Test".to_string()))
        .await?;

    let message_id = message_manager
        .add_message(session_id, create_test_message("Original"))
        .await?;
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    assert_eq!(
        session_manager
            .get_session(session_id)
            .unwrap()
            .message_count,
        1
    );

    message_manager
        .edit_message(session_id, message_id, "Edited".to_string())
        .await?;
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    assert_eq!(
        session_manager
            .get_session(session_id)
            .unwrap()
            .message_count,
        1
    );
    assert_eq!(
        message_manager.get_session_messages_owned(session_id)[0].content,
        "Edited"
    );

    message_manager
        .delete_message(session_id, message_id, false)
        .await?;
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    assert_eq!(
        session_manager
            .get_session(session_id)
            .unwrap()
            .message_count,
        0
    );
    assert!(message_manager
        .get_session_messages_owned(session_id)
        .is_empty());

    Ok(())
}
