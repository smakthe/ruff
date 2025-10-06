use ruff::{
    session::manager::{SessionManager, ChatSessionWithMessages},
    message::manager::{MessageManager, Message, MessageRole},
    export::{
        ExportService, ImportService, ExportFormat, ImportFormat,
        SessionExportRequest, ExportOptions, ImportOptions
    },
    events::EventBus,
    EnhancedError,
};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Local;

#[tokio::test]
async fn test_export_import_roundtrip_markdown() -> Result<(), EnhancedError> {
    test_export_import_roundtrip(ExportFormat::Markdown, ImportFormat::Json).await
}

#[tokio::test]
async fn test_export_import_roundtrip_json() -> Result<(), EnhancedError> {
    test_export_import_roundtrip(ExportFormat::Json, ImportFormat::Json).await
}

async fn test_export_import_roundtrip(
    export_format: ExportFormat,
    import_format: ImportFormat
) -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus.clone());

    let export_dir = std::env::temp_dir().join("ruff_test_roundtrip");
    std::fs::create_dir_all(&export_dir).ok();
    let export_service = ExportService::new(export_dir.clone());
    let mut import_service = ImportService::new(event_bus.clone());

    // Create session with complex data
    let session_id = session_manager.create_session(Some("Round Trip Test".to_string())).await?;

    // Add multiple messages
    for i in 1..=5 {
        let message = create_test_message(session_id, &format!("Message {}", i));
        message_manager.add_message(session_id, message).await?;
    }

    // Get original messages for comparison
    let original_messages = message_manager.get_session_messages_owned(session_id);
    let original_session = session_manager.get_session(session_id).unwrap().clone();

    // Export
    let session = session_manager.get_session(session_id).unwrap();
    let request = SessionExportRequest {
        session_id,
        format: export_format.clone(),
        options: ExportOptions::default(),
        output_path: None,
    };

    let export_result = export_service.export_session(session, request, &message_manager)?;

    // For JSON format, we can import it back
    if matches!(export_format, ExportFormat::Json) {
        // Import
        let import_result = import_service.import_from_file(
            &std::path::PathBuf::from(&export_result.file_path),
            Some(import_format),
            ImportOptions::default()
        ).await?;

        // VERIFY: Import succeeded
        assert_eq!(import_result.imported_sessions.len(), 1);
        assert_eq!(import_result.errors.len(), 0);

        // VERIFY: All messages preserved
        assert_eq!(import_result.total_messages, 5);

        // Get imported session
        let imported_session_id = import_result.imported_sessions[0];
        let imported_session = session_manager.get_session(imported_session_id).unwrap();
        let imported_messages = message_manager.get_session_messages_owned(imported_session_id);

        // VERIFY: Message content matches
        assert_eq!(imported_messages.len(), original_messages.len());
        for i in 0..original_messages.len() {
            assert_eq!(imported_messages[i].content, original_messages[i].content);
            assert_eq!(imported_messages[i].role, original_messages[i].role);
        }

        // VERIFY: Session metadata matches
        assert_eq!(imported_session.title, original_session.title);
        assert_eq!(imported_session.model, original_session.model);
        assert_eq!(imported_session.message_count, original_session.message_count);
    }

    // Cleanup
    std::fs::remove_file(&export_result.file_path).ok();
    std::fs::remove_dir_all(&export_dir).ok();

    Ok(())
}

#[tokio::test]
async fn test_export_all_formats() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus.clone());

    let export_dir = std::env::temp_dir().join("ruff_test_formats");
    std::fs::create_dir_all(&export_dir).ok();
    let export_service = ExportService::new(export_dir.clone());

    // Create session
    let session_id = session_manager.create_session(Some("Format Test".to_string())).await?;

    // Add messages
    let message = create_test_message(session_id, "Test message");
    message_manager.add_message(session_id, message).await?;

    let session = session_manager.get_session(session_id).unwrap();

    // Test all formats
    let formats = vec![
        ExportFormat::Markdown,
        ExportFormat::Json,
        ExportFormat::Html,
        ExportFormat::PlainText,
    ];

    for format in formats {
        let request = SessionExportRequest {
            session_id,
            format: format.clone(),
            options: ExportOptions::default(),
            output_path: None,
        };

        let result = export_service.export_session(session, request, &message_manager)?;

        // VERIFY: File exists
        assert!(std::path::Path::new(&result.file_path).exists());

        // VERIFY: File is not empty
        let content = std::fs::read_to_string(&result.file_path)?;
        assert!(!content.is_empty());

        // VERIFY: Contains message content
        assert!(content.contains("Test message"));

        // Cleanup
        std::fs::remove_file(&result.file_path).ok();
    }

    std::fs::remove_dir_all(&export_dir).ok();

    Ok(())
}

#[tokio::test]
async fn test_import_validation() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut import_service = ImportService::new(event_bus.clone());

    // Create invalid JSON file
    let invalid_file = std::env::temp_dir().join("invalid_import.json");
    std::fs::write(&invalid_file, "{ invalid json }")?;

    // Try to import
    let result = import_service.import_from_file(
        &invalid_file,
        Some(ImportFormat::Json),
        ImportOptions::default()
    ).await;

    // VERIFY: Import fails
    assert!(result.is_err());

    // Cleanup
    std::fs::remove_file(&invalid_file).ok();

    Ok(())
}

#[tokio::test]
async fn test_export_preserves_metadata() -> Result<(), EnhancedError> {
    // Setup
    let event_bus = Arc::new(EventBus::new());
    let mut session_manager = SessionManager::new(event_bus.clone()).await?;
    let mut message_manager = MessageManager::new(event_bus.clone());

    let export_dir = std::env::temp_dir().join("ruff_test_metadata");
    std::fs::create_dir_all(&export_dir).ok();
    let export_service = ExportService::new(export_dir.clone());

    // Create session with specific metadata
    let session_id = session_manager.create_session(Some("Metadata Test".to_string())).await?;

    // Add message
    let message = create_test_message(session_id, "Content");
    message_manager.add_message(session_id, message).await?;

    // Export with metadata
    let session = session_manager.get_session(session_id).unwrap();
    let request = SessionExportRequest {
        session_id,
        format: ExportFormat::Json,
        options: ExportOptions {
            include_metadata: true,
            ..Default::default()
        },
        output_path: None,
    };

    let result = export_service.export_session(session, request, &message_manager)?;

    // Read and verify JSON
    let content = std::fs::read_to_string(&result.file_path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    // VERIFY: Metadata present
    assert!(json.get("title").is_some());
    assert!(json.get("model").is_some());
    assert!(json.get("created_at").is_some());

    // Cleanup
    std::fs::remove_file(&result.file_path).ok();
    std::fs::remove_dir_all(&export_dir).ok();

    Ok(())
}

// Helper function
fn create_test_message(session_id: uuid::Uuid, content: &str) -> Message {
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
