use ruff::{
    events::EventBus,
    export::{
        import::ImportOptions, ExportFormat, ExportOptions, ExportService, ImportFormat,
        ImportService, SessionExportRequest,
    },
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
async fn test_json_export_includes_message_manager_messages() -> Result<(), EnhancedError> {
    let (temp_dir, mut session_manager, mut message_manager) = create_managers();
    let export_service = ExportService::new(temp_dir.path().join("exports"));
    export_service.initialize()?;

    let session_id = session_manager
        .create_session(Some("JSON Export Test".to_string()))
        .await?;
    for i in 1..=5 {
        message_manager
            .add_message(session_id, create_test_message(&format!("Message {}", i)))
            .await?;
    }
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    let result = export_service.export_session(
        session_manager.get_session(session_id).unwrap(),
        SessionExportRequest {
            session_id,
            format: ExportFormat::Json,
            options: ExportOptions::default(),
            output_path: None,
        },
        &message_manager,
    )?;

    let content = std::fs::read_to_string(&result.file_path)
        .map_err(|e| EnhancedError::storage(format!("Failed to read export: {}", e)))?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    assert_eq!(result.message_count, 5);
    assert_eq!(json["messages"].as_array().unwrap().len(), 5);
    assert!(content.contains("Message 5"));

    Ok(())
}

#[tokio::test]
async fn test_export_all_formats() -> Result<(), EnhancedError> {
    let (temp_dir, mut session_manager, mut message_manager) = create_managers();
    let export_service = ExportService::new(temp_dir.path().join("exports"));
    export_service.initialize()?;

    let session_id = session_manager
        .create_session(Some("Format Test".to_string()))
        .await?;
    message_manager
        .add_message(session_id, create_test_message("Test message"))
        .await?;
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    for format in [
        ExportFormat::Markdown,
        ExportFormat::Json,
        ExportFormat::Html,
        ExportFormat::PlainText,
    ] {
        let result = export_service.export_session(
            session_manager.get_session(session_id).unwrap(),
            SessionExportRequest {
                session_id,
                format,
                options: ExportOptions::default(),
                output_path: None,
            },
            &message_manager,
        )?;

        let content = std::fs::read_to_string(&result.file_path)
            .map_err(|e| EnhancedError::storage(format!("Failed to read export: {}", e)))?;
        assert!(!content.is_empty());
        assert!(content.contains("Test message"));
    }

    Ok(())
}

#[tokio::test]
async fn test_import_validation() -> Result<(), EnhancedError> {
    let temp_dir = TempDir::new().expect("create temporary test directory");
    let mut import_service = ImportService::new(EventBus::new());
    let invalid_file = temp_dir.path().join("invalid_import.json");
    std::fs::write(&invalid_file, "{ invalid json }")
        .map_err(|e| EnhancedError::storage(format!("Failed to write invalid import: {}", e)))?;

    let result = import_service
        .import_from_file(
            &invalid_file,
            Some(ImportFormat::Json),
            ImportOptions::default(),
        )
        .await;

    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_import_persists_sessions_and_messages() -> Result<(), EnhancedError> {
    let (temp_dir, mut session_manager, mut message_manager) = create_managers();
    let mut import_service = ImportService::new(EventBus::new());
    let import_file = temp_dir.path().join("import.json");

    std::fs::write(
        &import_file,
        r#"{
            "title": "Imported Session",
            "model": "groq-llama3",
            "messages": [
                { "role": "user", "content": "Hello import" },
                { "role": "assistant", "content": "Imported response" }
            ]
        }"#,
    )
    .map_err(|e| EnhancedError::storage(format!("Failed to write import fixture: {}", e)))?;

    let result = import_service
        .import_from_file_into_managers(
            &import_file,
            Some(ImportFormat::Json),
            ImportOptions::default(),
            &mut session_manager,
            &mut message_manager,
        )
        .await?;

    assert_eq!(result.imported_sessions.len(), 1);
    assert_eq!(result.total_messages, 2);

    let session_id = result.imported_sessions[0];
    let session = session_manager.get_session(session_id).unwrap();
    let messages = message_manager.get_session_messages_owned(session_id);

    assert_eq!(session.title, "Imported Session");
    assert_eq!(session.message_count, 2);
    assert_eq!(messages.len(), 2);
    assert!(messages
        .iter()
        .any(|message| message.content == "Hello import"));
    assert!(messages
        .iter()
        .any(|message| message.content == "Imported response"));

    Ok(())
}

#[tokio::test]
async fn test_export_preserves_session_metadata() -> Result<(), EnhancedError> {
    let (temp_dir, mut session_manager, mut message_manager) = create_managers();
    let export_service = ExportService::new(temp_dir.path().join("exports"));
    export_service.initialize()?;

    let session_id = session_manager
        .create_session(Some("Metadata Test".to_string()))
        .await?;
    message_manager
        .add_message(session_id, create_test_message("Content"))
        .await?;
    session_manager
        .update_session_metadata(session_id, &message_manager)
        .await?;

    let result = export_service.export_session(
        session_manager.get_session(session_id).unwrap(),
        SessionExportRequest {
            session_id,
            format: ExportFormat::Json,
            options: ExportOptions {
                include_metadata: true,
                ..Default::default()
            },
            output_path: None,
        },
        &message_manager,
    )?;

    let content = std::fs::read_to_string(&result.file_path)
        .map_err(|e| EnhancedError::storage(format!("Failed to read export: {}", e)))?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    assert_eq!(json["session"]["title"], "Metadata Test");
    assert!(json["session"].get("model").is_some());
    assert!(json["session"].get("created_at").is_some());

    Ok(())
}
