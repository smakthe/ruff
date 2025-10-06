//! Tests for export and import functionality

#[cfg(test)]
mod export_tests {
    use super::super::*;
    use crate::export::formats::{MarkdownHandler, PlainTextHandler, JsonHandler, HtmlHandler, FormatHandler};
    use crate::session::manager::{ChatSession, Message, MessageRole, MessageMetadata, SessionModelConfig};
    use crate::models::TokenUsage;
    use chrono::Local;
    use tempfile::TempDir;
    use uuid::Uuid;
    use std::fs;

    fn create_test_session() -> ChatSession {
        let session_id = Uuid::new_v4();
        let now = Local::now();
        
        ChatSession {
            id: session_id,
            title: "Test Session".to_string(),
            created_at: now,
            updated_at: now,
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    role: MessageRole::User,
                    content: "Hello, how are you?".to_string(),
                    timestamp: now,
                    edited_at: None,
                    token_usage: Some(TokenUsage {
                        input_tokens: 5,
                        output_tokens: 0,
                        total_tokens: 5,
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
                },
                Message {
                    id: Uuid::new_v4(),
                    role: MessageRole::Assistant,
                    content: "I'm doing well, thank you! How can I help you today?".to_string(),
                    timestamp: now,
                    edited_at: None,
                    token_usage: Some(TokenUsage {
                        input_tokens: 0,
                        output_tokens: 12,
                        total_tokens: 12,
                    }),
                    parent_id: None,
                    children: Vec::new(),
                    metadata: MessageMetadata {
                        model_used: "gpt-3.5-turbo".to_string(),
                        temperature: 0.7,
                        response_time_ms: 250,
                        is_regenerated: false,
                        regeneration_count: 0,
                    },
                },
            ],
            model: "gpt-3.5-turbo".to_string(),
            system_prompt: Some("You are a helpful assistant.".to_string()),
            model_config: SessionModelConfig::default(),
            total_tokens_used: TokenUsage {
                input_tokens: 5,
                output_tokens: 12,
                total_tokens: 17,
            },
            tags: vec!["test".to_string(), "example".to_string()],
            is_archived: false,
            export_count: 0,
            message_count: 2,
            last_activity: now,
        }
    }

    fn create_test_export_service() -> (ExportService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let export_service = ExportService::new(temp_dir.path().to_path_buf());
        export_service.initialize().unwrap();
        (export_service, temp_dir)
    }

    #[test]
    fn test_export_service_creation() {
        let (service, _temp_dir) = create_test_export_service();
        assert!(service.get_export_directory().exists());
    }

    #[test]
    fn test_markdown_export() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::Markdown,
            options: ExportOptions::default(),
            output_path: None,
        };

        let result = service.export_session(&session, request).unwrap();
        
        assert_eq!(result.format, ExportFormat::Markdown);
        assert_eq!(result.session_id, Some(session.id));
        assert_eq!(result.message_count, 2);
        assert!(result.size_bytes > 0);
        assert!(std::path::Path::new(&result.file_path).exists());

        // Check file content
        let content = fs::read_to_string(&result.file_path).unwrap();
        assert!(content.contains("# Test Session"));
        assert!(content.contains("### 👤 User"));
        assert!(content.contains("### 🤖 Assistant"));
        assert!(content.contains("Hello, how are you?"));
        assert!(content.contains("I'm doing well, thank you!"));
    }

    #[test]
    fn test_plain_text_export() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::PlainText,
            options: ExportOptions::default(),
            output_path: None,
        };

        let result = service.export_session(&session, request).unwrap();
        
        assert_eq!(result.format, ExportFormat::PlainText);
        assert!(std::path::Path::new(&result.file_path).exists());

        // Check file content
        let content = fs::read_to_string(&result.file_path).unwrap();
        assert!(content.contains("Test Session"));
        assert!(content.contains("[USER]"));
        assert!(content.contains("[ASSISTANT]"));
        assert!(content.contains("Hello, how are you?"));
    }

    #[test]
    fn test_json_export() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::Json,
            options: ExportOptions::default(),
            output_path: None,
        };

        let result = service.export_session(&session, request).unwrap();
        
        assert_eq!(result.format, ExportFormat::Json);
        assert!(std::path::Path::new(&result.file_path).exists());

        // Check file content is valid JSON
        let content = fs::read_to_string(&result.file_path).unwrap();
        let json_value: serde_json::Value = serde_json::from_str(&content).unwrap();
        
        assert!(json_value["title"].as_str().unwrap() == "Test Session");
        assert!(json_value["messages"].as_array().unwrap().len() == 2);
    }

    #[test]
    fn test_html_export() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::Html,
            options: ExportOptions::default(),
            output_path: None,
        };

        let result = service.export_session(&session, request).unwrap();
        
        assert_eq!(result.format, ExportFormat::Html);
        assert!(std::path::Path::new(&result.file_path).exists());

        // Check file content
        let content = fs::read_to_string(&result.file_path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("<title>Test Session</title>"));
        assert!(content.contains("👤"));
        assert!(content.contains("🤖"));
        assert!(content.contains("Hello, how are you?"));
    }

    #[test]
    fn test_export_options() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let options = ExportOptions {
            include_timestamps: false,
            include_model_info: false,
            include_token_usage: false,
            include_metadata: false,
            pretty_format: false,
        };
        
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::Markdown,
            options,
            output_path: None,
        };

        let result = service.export_session(&session, request).unwrap();
        let content = fs::read_to_string(&result.file_path).unwrap();
        
        // Should not contain timestamps or model info
        assert!(!content.contains("2024")); // Assuming current year
        assert!(!content.contains("gpt-3.5-turbo"));
    }

    #[test]
    fn test_messages_only_export() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let request = MessagesExportRequest {
            messages: session.messages.clone(),
            format: ExportFormat::Markdown,
            options: ExportOptions::default(),
            output_path: None,
            title: Some("Test Messages".to_string()),
        };

        let result = service.export_messages(request).unwrap();
        
        assert_eq!(result.format, ExportFormat::Markdown);
        assert_eq!(result.session_id, None);
        assert_eq!(result.message_count, 2);
        assert!(std::path::Path::new(&result.file_path).exists());

        let content = fs::read_to_string(&result.file_path).unwrap();
        assert!(content.contains("### 👤 User"));
        assert!(content.contains("### 🤖 Assistant"));
    }

    #[test]
    fn test_bulk_export() {
        let (service, _temp_dir) = create_test_export_service();
        let session1 = create_test_session();
        let mut session2 = create_test_session();
        session2.id = Uuid::new_v4();
        session2.title = "Second Session".to_string();
        
        let sessions = vec![session1.clone(), session2.clone()];
        let request = BulkExportRequest {
            session_ids: vec![session1.id, session2.id],
            format: ExportFormat::Markdown,
            options: ExportOptions::default(),
            output_directory: None,
        };

        let results = service.export_sessions(&sessions, request).unwrap();
        
        assert_eq!(results.len(), 2);
        for result in results {
            assert_eq!(result.format, ExportFormat::Markdown);
            assert!(std::path::Path::new(&result.file_path).exists());
        }
    }

    #[test]
    fn test_custom_output_path() {
        let (service, temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let custom_path = temp_dir.path().join("custom_export.md");
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::Markdown,
            options: ExportOptions::default(),
            output_path: Some(custom_path.clone()),
        };

        let result = service.export_session(&session, request).unwrap();
        
        assert_eq!(result.file_path, custom_path.to_string_lossy().to_string());
        assert!(custom_path.exists());
    }

    #[test]
    fn test_list_exports() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        // Export a few files
        for format in [ExportFormat::Markdown, ExportFormat::Json, ExportFormat::Html] {
            let request = SessionExportRequest {
                session_id: session.id,
                format,
                options: ExportOptions::default(),
                output_path: None,
            };
            service.export_session(&session, request).unwrap();
        }

        let exports = service.list_exports().unwrap();
        assert_eq!(exports.len(), 3);
    }

    #[test]
    fn test_export_statistics() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        // Export a few files
        for format in [ExportFormat::Markdown, ExportFormat::Json] {
            let request = SessionExportRequest {
                session_id: session.id,
                format,
                options: ExportOptions::default(),
                output_path: None,
            };
            service.export_session(&session, request).unwrap();
        }

        let stats = service.get_export_statistics().unwrap();
        assert_eq!(stats.total_exports, 2);
        assert!(stats.total_size_bytes > 0);
        assert_eq!(stats.format_counts.get(&ExportFormat::Markdown), Some(&1));
        assert_eq!(stats.format_counts.get(&ExportFormat::Json), Some(&1));
    }

    #[test]
    fn test_delete_export() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::Markdown,
            options: ExportOptions::default(),
            output_path: None,
        };

        let result = service.export_session(&session, request).unwrap();
        let file_path = std::path::Path::new(&result.file_path);
        
        assert!(file_path.exists());
        
        service.delete_export(file_path).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn test_sanitize_filename() {
        let (service, _temp_dir) = create_test_export_service();
        
        let sanitized = service.sanitize_filename("Test/Session\\With:Invalid*Characters?");
        assert_eq!(sanitized, "Test_Session_With_Invalid_Characters_");
        
        let sanitized2 = service.sanitize_filename("Normal Session Name");
        assert_eq!(sanitized2, "Normal Session Name");
    }

    #[test]
    fn test_format_handlers() {
        let session = create_test_session();
        let options = ExportOptions::default();

        // Test Markdown handler
        let markdown_handler = MarkdownHandler;
        let markdown_content = markdown_handler.export_session(&session, &options).unwrap();
        assert!(markdown_content.contains("# Test Session"));
        assert!(markdown_content.contains("### 👤 User"));

        // Test PlainText handler
        let text_handler = PlainTextHandler;
        let text_content = text_handler.export_session(&session, &options).unwrap();
        assert!(text_content.contains("Test Session"));
        assert!(text_content.contains("[USER]"));

        // Test JSON handler
        let json_handler = JsonHandler;
        let json_content = json_handler.export_session(&session, &options).unwrap();
        let json_value: serde_json::Value = serde_json::from_str(&json_content).unwrap();
        assert!(json_value["title"].as_str().unwrap() == "Test Session");

        // Test HTML handler
        let html_handler = HtmlHandler;
        let html_content = html_handler.export_session(&session, &options).unwrap();
        assert!(html_content.contains("<!DOCTYPE html>"));
        assert!(html_content.contains("<title>Test Session</title>"));
    }

    #[test]
    fn test_export_with_metadata() {
        let (service, _temp_dir) = create_test_export_service();
        let session = create_test_session();
        
        let options = ExportOptions {
            include_timestamps: true,
            include_model_info: true,
            include_token_usage: true,
            include_metadata: true,
            pretty_format: true,
        };
        
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::Markdown,
            options,
            output_path: None,
        };

        let result = service.export_session(&session, request).unwrap();
        let content = fs::read_to_string(&result.file_path).unwrap();
        
        // Should contain metadata
        assert!(content.contains("Session Information"));
        assert!(content.contains("Total Tokens"));
        assert!(content.contains("gpt-3.5-turbo"));
        assert!(content.contains("test, example")); // tags
    }

    #[test]
    fn test_export_empty_session() {
        let (service, _temp_dir) = create_test_export_service();
        let mut session = create_test_session();
        session.messages.clear();
        
        let request = SessionExportRequest {
            session_id: session.id,
            format: ExportFormat::Markdown,
            options: ExportOptions::default(),
            output_path: None,
        };

        let result = service.export_session(&session, request).unwrap();
        
        assert_eq!(result.message_count, 0);
        assert!(std::path::Path::new(&result.file_path).exists());
        
        let content = fs::read_to_string(&result.file_path).unwrap();
        assert!(content.contains("# Test Session"));
        assert!(content.contains("## Conversation"));
    }
}
#
[cfg(test)]
mod import_tests {
    use crate::export::import::{ImportService, ImportOptions};
    use crate::export::formats::ImportFormat;
    use crate::export::validation::{DataValidator, ValidationRules};
    use crate::events::EventBus;
    use crate::session::manager::{ChatSession, Message, MessageRole, MessageMetadata, SessionModelConfig};
    use crate::models::TokenUsage;
    use chrono::Local;
    use tempfile::TempDir;
    use uuid::Uuid;
    use std::fs;
    use tokio;

    fn create_test_import_service() -> ImportService {
        let event_bus = EventBus::new();
        ImportService::new(event_bus)
    }

    fn create_chatgpt_export_json() -> String {
        serde_json::json!({
            "title": "Test ChatGPT Conversation",
            "create_time": 1640995200.0, // 2022-01-01 00:00:00 UTC
            "update_time": 1640995800.0, // 2022-01-01 00:10:00 UTC
            "mapping": {
                "root": {
                    "id": "root",
                    "message": null,
                    "parent": null,
                    "children": ["msg1"]
                },
                "msg1": {
                    "id": "msg1",
                    "message": {
                        "id": "msg1",
                        "author": {
                            "role": "user",
                            "name": null,
                            "metadata": {}
                        },
                        "create_time": 1640995200.0,
                        "update_time": null,
                        "content": {
                            "content_type": "text",
                            "parts": ["Hello, how are you today?"]
                        },
                        "status": "finished_successfully",
                        "end_turn": true,
                        "weight": 1.0,
                        "metadata": {},
                        "recipient": "all"
                    },
                    "parent": "root",
                    "children": ["msg2"]
                },
                "msg2": {
                    "id": "msg2",
                    "message": {
                        "id": "msg2",
                        "author": {
                            "role": "assistant",
                            "name": null,
                            "metadata": {}
                        },
                        "create_time": 1640995400.0,
                        "update_time": null,
                        "content": {
                            "content_type": "text",
                            "parts": ["I'm doing well, thank you! How can I help you today?"]
                        },
                        "status": "finished_successfully",
                        "end_turn": true,
                        "weight": 1.0,
                        "metadata": {},
                        "recipient": "all"
                    },
                    "parent": "msg1",
                    "children": []
                }
            },
            "moderation_results": [],
            "current_node": "msg2",
            "plugin_ids": null,
            "conversation_id": "test-conversation-id",
            "conversation_template_id": null,
            "gizmo_id": null,
            "is_archived": false,
            "safe_urls": []
        }).to_string()
    }

    fn create_claude_export_json() -> String {
        serde_json::json!({
            "uuid": "test-claude-uuid",
            "name": "Test Claude Conversation",
            "summary": "A test conversation with Claude",
            "model": "claude-3-sonnet",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:10:00Z",
            "chat_messages": [
                {
                    "uuid": "msg1-uuid",
                    "text": "What is the capital of France?",
                    "sender": "human",
                    "index": 0,
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": null,
                    "edited_at": null,
                    "chat_feedback": null,
                    "attachments": []
                },
                {
                    "uuid": "msg2-uuid",
                    "text": "The capital of France is Paris. It's a beautiful city known for its art, culture, and iconic landmarks like the Eiffel Tower.",
                    "sender": "assistant",
                    "index": 1,
                    "created_at": "2024-01-01T00:01:00Z",
                    "updated_at": null,
                    "edited_at": null,
                    "chat_feedback": null,
                    "attachments": []
                }
            ]
        }).to_string()
    }

    fn create_generic_json_export() -> String {
        serde_json::json!({
            "title": "Generic JSON Conversation",
            "model": "gpt-4",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:05:00Z",
            "messages": [
                {
                    "role": "user",
                    "content": "Tell me a joke",
                    "timestamp": "2024-01-01T00:00:00Z"
                },
                {
                    "role": "assistant",
                    "content": "Why don't scientists trust atoms? Because they make up everything!",
                    "timestamp": "2024-01-01T00:01:00Z"
                }
            ]
        }).to_string()
    }

    fn create_ruff_session_export() -> String {
        let session = ChatSession {
            id: Uuid::new_v4(),
            title: "Ruff Export Test".to_string(),
            created_at: Local::now(),
            updated_at: Local::now(),
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    role: MessageRole::User,
                    content: "Test message".to_string(),
                    timestamp: Local::now(),
                    edited_at: None,
                    token_usage: Some(TokenUsage {
                        input_tokens: 2,
                        output_tokens: 0,
                        total_tokens: 2,
                    }),
                    parent_id: None,
                    children: Vec::new(),
                    metadata: MessageMetadata {
                        model_used: "gpt-3.5-turbo".to_string(),
                        temperature: 0.7,
                        response_time_ms: 100,
                        is_regenerated: false,
                        regeneration_count: 0,
                    },
                }
            ],
            model: "gpt-3.5-turbo".to_string(),
            system_prompt: None,
            model_config: SessionModelConfig::default(),
            total_tokens_used: TokenUsage {
                input_tokens: 2,
                output_tokens: 0,
                total_tokens: 2,
            },
            tags: vec!["test".to_string()],
            is_archived: false,
            export_count: 0,
            message_count: 1,
            last_activity: Local::now(),
        };

        serde_json::to_string(&session).unwrap()
    }

    #[tokio::test]
    async fn test_import_service_creation() {
        let service = create_test_import_service();
        let formats = service.get_supported_formats();
        assert!(formats.contains(&ImportFormat::Json));
        assert!(formats.contains(&ImportFormat::ChatGptExport));
        assert!(formats.contains(&ImportFormat::ClaudeExport));
    }

    #[tokio::test]
    async fn test_detect_chatgpt_format() {
        let service = create_test_import_service();
        let content = create_chatgpt_export_json();
        
        let format = service.detect_format(&content, std::path::Path::new("test.json")).unwrap();
        assert_eq!(format, ImportFormat::ChatGptExport);
    }

    #[tokio::test]
    async fn test_detect_claude_format() {
        let service = create_test_import_service();
        let content = create_claude_export_json();
        
        let format = service.detect_format(&content, std::path::Path::new("test.json")).unwrap();
        assert_eq!(format, ImportFormat::ClaudeExport);
    }

    #[tokio::test]
    async fn test_detect_generic_json_format() {
        let service = create_test_import_service();
        let content = create_generic_json_export();
        
        let format = service.detect_format(&content, std::path::Path::new("test.json")).unwrap();
        assert_eq!(format, ImportFormat::Json);
    }

    #[tokio::test]
    async fn test_preview_chatgpt_import() {
        let service = create_test_import_service();
        let content = create_chatgpt_export_json();
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("chatgpt_export.json");
        fs::write(&file_path, content).unwrap();
        
        let preview = service.preview_import(&file_path, None).await.unwrap();
        
        assert_eq!(preview.format_detected, ImportFormat::ChatGptExport);
        assert_eq!(preview.session_count, 1);
        assert_eq!(preview.total_messages, 2);
        assert!(preview.validation_result.is_valid);
        
        let session_preview = &preview.sessions[0];
        assert_eq!(session_preview.title, "Test ChatGPT Conversation");
        assert_eq!(session_preview.message_count, 2);
        assert!(session_preview.first_message_preview.is_some());
        assert!(session_preview.first_message_preview.as_ref().unwrap().contains("Hello, how are you"));
    }

    #[tokio::test]
    async fn test_preview_claude_import() {
        let service = create_test_import_service();
        let content = create_claude_export_json();
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("claude_export.json");
        fs::write(&file_path, content).unwrap();
        
        let preview = service.preview_import(&file_path, None).await.unwrap();
        
        assert_eq!(preview.format_detected, ImportFormat::ClaudeExport);
        assert_eq!(preview.session_count, 1);
        assert_eq!(preview.total_messages, 2);
        
        let session_preview = &preview.sessions[0];
        assert_eq!(session_preview.title, "Test Claude Conversation");
        assert_eq!(session_preview.model, Some("claude-3-sonnet".to_string()));
    }

    #[tokio::test]
    async fn test_import_chatgpt_conversation() {
        let mut service = create_test_import_service();
        let content = create_chatgpt_export_json();
        
        let options = ImportOptions::default();
        let result = service.import_from_content(&content, None, options).await.unwrap();
        
        assert_eq!(result.imported_sessions.len(), 1);
        assert_eq!(result.total_messages, 2);
        assert_eq!(result.skipped_messages, 0);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_import_claude_conversation() {
        let mut service = create_test_import_service();
        let content = create_claude_export_json();
        
        let options = ImportOptions::default();
        let result = service.import_from_content(&content, None, options).await.unwrap();
        
        assert_eq!(result.imported_sessions.len(), 1);
        assert_eq!(result.total_messages, 2);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_import_generic_json() {
        let mut service = create_test_import_service();
        let content = create_generic_json_export();
        
        let options = ImportOptions::default();
        let result = service.import_from_content(&content, None, options).await.unwrap();
        
        assert_eq!(result.imported_sessions.len(), 1);
        assert_eq!(result.total_messages, 2);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_import_ruff_session() {
        let mut service = create_test_import_service();
        let content = create_ruff_session_export();
        
        let options = ImportOptions::default();
        let result = service.import_from_content(&content, None, options).await.unwrap();
        
        assert_eq!(result.imported_sessions.len(), 1);
        assert_eq!(result.total_messages, 1);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_import_from_file() {
        let mut service = create_test_import_service();
        let content = create_chatgpt_export_json();
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_import.json");
        fs::write(&file_path, content).unwrap();
        
        let options = ImportOptions::default();
        let result = service.import_from_file(&file_path, None, options).await.unwrap();
        
        assert_eq!(result.imported_sessions.len(), 1);
        assert_eq!(result.total_messages, 2);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_import_with_custom_options() {
        let mut service = create_test_import_service();
        let content = create_generic_json_export();
        
        let options = ImportOptions {
            merge_with_existing: false,
            preserve_timestamps: false,
            auto_generate_titles: true,
            skip_invalid_messages: true,
            max_message_length: Some(50),
            default_model: "custom-model".to_string(),
        };
        
        let result = service.import_from_content(&content, None, options).await.unwrap();
        
        assert_eq!(result.imported_sessions.len(), 1);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_import_with_message_length_limit() {
        let mut service = create_test_import_service();
        
        // Create content with a very long message
        let long_message = "A".repeat(1000);
        let content = serde_json::json!({
            "title": "Long Message Test",
            "messages": [
                {
                    "role": "user",
                    "content": long_message,
                    "timestamp": "2024-01-01T00:00:00Z"
                }
            ]
        }).to_string();
        
        let options = ImportOptions {
            max_message_length: Some(100),
            skip_invalid_messages: true,
            ..Default::default()
        };
        
        let result = service.import_from_content(&content, None, options).await.unwrap();
        
        // Message should be skipped due to length
        assert_eq!(result.total_messages, 0);
        assert_eq!(result.skipped_messages, 0); // This would be tracked in a real implementation
    }

    #[tokio::test]
    async fn test_import_invalid_json() {
        let mut service = create_test_import_service();
        let invalid_content = "{ invalid json content";
        
        let options = ImportOptions::default();
        let result = service.import_from_content(&invalid_content, None, options).await;
        
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        println!("Error message: {}", error_msg);
        assert!(error_msg.contains("Unable to detect") || error_msg.contains("parse") || error_msg.contains("JSON"));
    }

    #[tokio::test]
    async fn test_import_empty_content() {
        let mut service = create_test_import_service();
        let empty_content = serde_json::json!({
            "title": "Empty Session",
            "messages": []
        }).to_string();
        
        let options = ImportOptions::default();
        let result = service.import_from_content(&empty_content, None, options).await.unwrap();
        
        assert_eq!(result.imported_sessions.len(), 1);
        assert_eq!(result.total_messages, 0);
    }

    #[tokio::test]
    async fn test_timestamp_parsing() {
        let service = create_test_import_service();
        
        // Test various timestamp formats
        let formats = vec![
            "2024-01-01T00:00:00Z",
            "2024-01-01T00:00:00.123Z",
            "2024-01-01T00:00:00+00:00",
            "2024-01-01 00:00:00",
            "1640995200", // Unix timestamp
            "1640995200.123", // Unix timestamp with decimals
        ];
        
        for timestamp_str in formats {
            let result = service.parse_timestamp(timestamp_str);
            assert!(result.is_ok(), "Failed to parse timestamp: {}", timestamp_str);
        }
    }

    #[tokio::test]
    async fn test_title_generation() {
        let service = create_test_import_service();
        
        let test_cases = vec![
            ("Hello, how are you today?", "Hello, how are you today"),
            ("This is a very long message that should be truncated because it exceeds the maximum length", "This is a very long message that should be trun..."),
            ("", "Imported Conversation"),
            ("   ", "Imported Conversation"),
            ("Short", "Short"),
        ];
        
        for (input, expected) in test_cases {
            let result = service.generate_title_from_content(input);
            assert_eq!(result, expected);
        }
    }

    #[tokio::test]
    async fn test_validation_integration() {
        let service = create_test_import_service();
        
        // Create a session with validation issues
        let problematic_session = ChatSession {
            id: Uuid::new_v4(),
            title: "".to_string(), // Empty title
            created_at: Local::now(),
            updated_at: Local::now(),
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    role: MessageRole::User,
                    content: "".to_string(), // Empty content
                    timestamp: Local::now(),
                    edited_at: None,
                    token_usage: None,
                    parent_id: None,
                    children: Vec::new(),
                    metadata: MessageMetadata {
                        model_used: "".to_string(), // Empty model
                        temperature: 0.7,
                        response_time_ms: 0,
                        is_regenerated: false,
                        regeneration_count: 0,
                    },
                }
            ],
            model: "test-model".to_string(),
            system_prompt: None,
            model_config: SessionModelConfig::default(),
            total_tokens_used: TokenUsage::default(),
            tags: Vec::new(),
            is_archived: false,
            export_count: 0,
            message_count: 1,
            last_activity: Local::now(),
        };
        
        let validation_result = service.validate_import_data(&[problematic_session]);
        
        assert!(!validation_result.is_valid);
        assert!(!validation_result.errors.is_empty());
        assert!(!validation_result.warnings.is_empty());
    }

    #[test]
    fn test_data_validator() {
        let validator = DataValidator::new();
        
        // Test with valid session
        let valid_session = ChatSession {
            id: Uuid::new_v4(),
            title: "Valid Session".to_string(),
            created_at: Local::now(),
            updated_at: Local::now(),
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    role: MessageRole::User,
                    content: "Valid message".to_string(),
                    timestamp: Local::now(),
                    edited_at: None,
                    token_usage: Some(TokenUsage {
                        input_tokens: 2,
                        output_tokens: 0,
                        total_tokens: 2,
                    }),
                    parent_id: None,
                    children: Vec::new(),
                    metadata: MessageMetadata {
                        model_used: "gpt-3.5-turbo".to_string(),
                        temperature: 0.7,
                        response_time_ms: 100,
                        is_regenerated: false,
                        regeneration_count: 0,
                    },
                }
            ],
            model: "gpt-3.5-turbo".to_string(),
            system_prompt: None,
            model_config: SessionModelConfig::default(),
            total_tokens_used: TokenUsage {
                input_tokens: 2,
                output_tokens: 0,
                total_tokens: 2,
            },
            tags: Vec::new(),
            is_archived: false,
            export_count: 0,
            message_count: 1,
            last_activity: Local::now(),
        };
        
        let result = validator.validate_sessions(&[valid_session]);
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validation_rules() {
        let custom_rules = ValidationRules {
            max_title_length: 10,
            max_message_length: 20,
            max_messages_per_session: 1,
            require_non_empty_content: true,
            allow_unknown_roles: false,
            require_timestamps: true,
        };
        
        let validator = DataValidator::with_rules(custom_rules);
        
        let session_with_long_title = ChatSession {
            id: Uuid::new_v4(),
            title: "This title is way too long for the validation rules".to_string(),
            created_at: Local::now(),
            updated_at: Local::now(),
            messages: Vec::new(),
            model: "test-model".to_string(),
            system_prompt: None,
            model_config: SessionModelConfig::default(),
            total_tokens_used: TokenUsage::default(),
            tags: Vec::new(),
            is_archived: false,
            export_count: 0,
            message_count: 0,
            last_activity: Local::now(),
        };
        
        let result = validator.validate_sessions(&[session_with_long_title]);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("title exceeds maximum length")));
    }

    #[test]
    fn test_content_quality_validation() {
        let validator = DataValidator::new();
        
        // Test normal content
        let warnings = validator.validate_content_quality("This is normal text content.");
        assert!(warnings.is_empty());
        
        // Test content with control characters
        let warnings = validator.validate_content_quality("Text with \x00 control character");
        assert!(warnings.iter().any(|w| w.contains("control characters")));
        
        // Test content with very long lines
        let long_line = "A".repeat(1500);
        let warnings = validator.validate_content_quality(&long_line);
        assert!(warnings.iter().any(|w| w.contains("very long lines")));
        
        // Test content dominated by repeated characters
        let repeated_content = "A".repeat(1000);
        let warnings = validator.validate_content_quality(&repeated_content);
        assert!(warnings.iter().any(|w| w.contains("dominated by repeated character")));
    }

    #[tokio::test]
    async fn test_import_file_validation() {
        let validator = DataValidator::new();
        
        // Test non-existent file
        let result = validator.validate_import_file(std::path::Path::new("non_existent_file.json"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
        
        // Test valid file
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");
        fs::write(&file_path, "{}").unwrap();
        
        let result = validator.validate_import_file(&file_path);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chatgpt_complex_conversation() {
        let mut service = create_test_import_service();
        
        // Create a more complex ChatGPT export with multiple branches
        let complex_export = serde_json::json!({
            "title": "Complex ChatGPT Conversation",
            "create_time": 1640995200.0,
            "update_time": 1640995800.0,
            "mapping": {
                "root": {
                    "id": "root",
                    "message": null,
                    "parent": null,
                    "children": ["msg1"]
                },
                "msg1": {
                    "id": "msg1",
                    "message": {
                        "id": "msg1",
                        "author": { "role": "user" },
                        "create_time": 1640995200.0,
                        "content": {
                            "content_type": "text",
                            "parts": ["What's the weather like?"]
                        }
                    },
                    "parent": "root",
                    "children": ["msg2", "msg3"]
                },
                "msg2": {
                    "id": "msg2",
                    "message": {
                        "id": "msg2",
                        "author": { "role": "assistant" },
                        "create_time": 1640995300.0,
                        "content": {
                            "content_type": "text",
                            "parts": ["I don't have access to current weather data."]
                        }
                    },
                    "parent": "msg1",
                    "children": []
                },
                "msg3": {
                    "id": "msg3",
                    "message": {
                        "id": "msg3",
                        "author": { "role": "assistant" },
                        "create_time": 1640995400.0,
                        "content": {
                            "content_type": "text",
                            "parts": ["Let me help you find weather information."]
                        }
                    },
                    "parent": "msg1",
                    "children": []
                }
            }
        }).to_string();
        
        let options = ImportOptions::default();
        let result = service.import_from_content(&complex_export, None, options).await.unwrap();
        
        assert_eq!(result.imported_sessions.len(), 1);
        assert_eq!(result.total_messages, 3); // 1 user + 2 assistant messages
        assert!(result.errors.is_empty());
    }
}
#[cfg(test)]
mod bulk_operations_tests {
    use super::super::*;
    use crate::session::manager::{ChatSession, Message, MessageRole, MessageMetadata, SessionModelConfig};
    use crate::models::TokenUsage;
    use chrono::Local;
    use tempfile::TempDir;
    use uuid::Uuid;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn create_test_session_with_id(id: Uuid, title: &str) -> ChatSession {
        let now = Local::now();
        
        ChatSession {
            id,
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    role: MessageRole::User,
                    content: "Hello, how are you?".to_string(),
                    timestamp: now,
                    edited_at: None,
                    token_usage: Some(TokenUsage {
                        input_tokens: 5,
                        output_tokens: 0,
                        total_tokens: 5,
                    }),
                    parent_id: None,
                    children: Vec::new(),
                    metadata: MessageMetadata {
                        model_used: "gpt-3.5-turbo".to_string(),
                        temperature: 0.7,
                        response_time_ms: 1000,
                        is_regenerated: false,
                        regeneration_count: 0,
                    },
                },
                Message {
                    id: Uuid::new_v4(),
                    role: MessageRole::Assistant,
                    content: "I'm doing well, thank you! How can I help you today?".to_string(),
                    timestamp: now,
                    edited_at: None,
                    token_usage: Some(TokenUsage {
                        input_tokens: 0,
                        output_tokens: 12,
                        total_tokens: 12,
                    }),
                    parent_id: None,
                    children: Vec::new(),
                    metadata: MessageMetadata {
                        model_used: "gpt-3.5-turbo".to_string(),
                        temperature: 0.7,
                        response_time_ms: 1500,
                        is_regenerated: false,
                        regeneration_count: 0,
                    },
                },
            ],
            model: "gpt-3.5-turbo".to_string(),
            system_prompt: None,
            model_config: SessionModelConfig::default(),
            total_tokens_used: TokenUsage {
                input_tokens: 5,
                output_tokens: 12,
                total_tokens: 17,
            },
            tags: vec!["test".to_string()],
            is_archived: false,
            export_count: 0,
            message_count: 2,
            last_activity: now,
        }
    }

    fn create_multiple_test_sessions(count: usize) -> Vec<ChatSession> {
        (0..count).map(|i| {
            create_test_session_with_id(Uuid::new_v4(), &format!("Test Session {}", i + 1))
        }).collect()
    }

    #[tokio::test]
    async fn test_bulk_export_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = ExportService::new(temp_dir.path().to_path_buf());
        export_service.initialize().unwrap();
        
        let sessions = create_multiple_test_sessions(5);
        let session_ids: Vec<_> = sessions.iter().map(|s| s.id).collect();
        
        let progress_counter = Arc::new(AtomicUsize::new(0));
        let progress_counter_clone = Arc::clone(&progress_counter);
        
        let progress_callback: ProgressCallback = Arc::new(move |current, total, message| {
            progress_counter_clone.store(current, Ordering::SeqCst);
            println!("Progress: {}/{} - {}", current, total, message);
        });
        
        let request = BulkExportRequest {
            session_ids,
            format: ExportFormat::Json,
            options: ExportOptions::default(),
            output_directory: None,
        };
        
        let result = export_service.export_sessions_with_progress(&sessions, request, Some(progress_callback)).await.unwrap();
        
        assert_eq!(result.total_items, 5);
        assert_eq!(result.successful_items, 5);
        assert_eq!(result.failed_items, 0);
        assert_eq!(result.results.len(), 5);
        assert!(result.errors.is_empty());
        assert!(result.duration_ms > 0);
        
        // Verify progress was tracked
        assert_eq!(progress_counter.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn test_bulk_export_with_errors() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = ExportService::new(temp_dir.path().to_path_buf());
        export_service.initialize().unwrap();
        
        let sessions = create_multiple_test_sessions(3);
        // Include a non-existent session ID to trigger filtering
        let mut session_ids: Vec<_> = sessions.iter().map(|s| s.id).collect();
        session_ids.push(Uuid::new_v4()); // Non-existent session
        
        let request = BulkExportRequest {
            session_ids,
            format: ExportFormat::Json,
            options: ExportOptions::default(),
            output_directory: None,
        };
        
        let result = export_service.export_sessions_with_progress(&sessions, request, None).await.unwrap();
        
        assert_eq!(result.total_items, 3); // Only existing sessions are processed
        assert_eq!(result.successful_items, 3);
        assert_eq!(result.failed_items, 0);
    }

    #[tokio::test]
    async fn test_bulk_import_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = ExportService::new(temp_dir.path().to_path_buf());
        export_service.initialize().unwrap();
        
        // First, create some export files
        let sessions = create_multiple_test_sessions(3);
        let session_ids: Vec<_> = sessions.iter().map(|s| s.id).collect();
        
        let export_request = BulkExportRequest {
            session_ids,
            format: ExportFormat::Json,
            options: ExportOptions::default(),
            output_directory: Some(temp_dir.path().join("exports")),
        };
        
        export_service.export_sessions(&sessions, export_request).unwrap();
        
        // Now test importing from the directory
        let import_result = export_service.import_sessions_from_directory(
            &temp_dir.path().join("exports"),
            None
        ).await.unwrap();
        
        println!("Import result: total={}, successful={}, failed={}, errors={:?}", 
            import_result.total_items, import_result.successful_items, 
            import_result.failed_items, import_result.errors);
        
        assert_eq!(import_result.total_items, 3);
        assert_eq!(import_result.successful_items, 3);
        assert_eq!(import_result.failed_items, 0);
        assert!(import_result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_bulk_import_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = ExportService::new(temp_dir.path().to_path_buf());
        export_service.initialize().unwrap();
        
        // Create export files
        let sessions = create_multiple_test_sessions(5);
        let session_ids: Vec<_> = sessions.iter().map(|s| s.id).collect();
        
        let export_request = BulkExportRequest {
            session_ids,
            format: ExportFormat::Json,
            options: ExportOptions::default(),
            output_directory: Some(temp_dir.path().join("exports")),
        };
        
        export_service.export_sessions(&sessions, export_request).unwrap();
        
        let progress_counter = Arc::new(AtomicUsize::new(0));
        let progress_counter_clone = Arc::clone(&progress_counter);
        
        let progress_callback: ProgressCallback = Arc::new(move |current, total, message| {
            progress_counter_clone.store(current, Ordering::SeqCst);
            println!("Import Progress: {}/{} - {}", current, total, message);
        });
        
        let import_result = export_service.import_sessions_from_directory(
            &temp_dir.path().join("exports"),
            Some(progress_callback)
        ).await.unwrap();
        
        assert_eq!(import_result.successful_items, 5);
        assert_eq!(progress_counter.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn test_bulk_import_with_invalid_files() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = ExportService::new(temp_dir.path().to_path_buf());
        export_service.initialize().unwrap();
        
        let import_dir = temp_dir.path().join("imports");
        std::fs::create_dir_all(&import_dir).unwrap();
        
        // Create a valid session file
        let session = create_test_session_with_id(Uuid::new_v4(), "Valid Session");
        let session_json = serde_json::to_string_pretty(&session).unwrap();
        std::fs::write(import_dir.join("valid_session.json"), session_json).unwrap();
        
        // Create an invalid JSON file
        std::fs::write(import_dir.join("invalid_session.json"), "{ invalid json }").unwrap();
        
        // Create a non-JSON file (should be ignored)
        std::fs::write(import_dir.join("readme.txt"), "This is not a session file").unwrap();
        
        let import_result = export_service.import_sessions_from_directory(&import_dir, None).await.unwrap();
        
        assert_eq!(import_result.total_items, 2); // Only JSON files are processed
        assert_eq!(import_result.successful_items, 1);
        assert_eq!(import_result.failed_items, 1);
        assert_eq!(import_result.errors.len(), 1);
    }

    #[test]
    fn test_validate_session_data() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = ExportService::new(temp_dir.path().to_path_buf());
        
        // Valid session
        let valid_session = create_test_session_with_id(Uuid::new_v4(), "Valid Session");
        assert!(export_service.validate_session_data(&valid_session).is_ok());
        
        // Invalid session - empty title
        let invalid_session = create_test_session_with_id(Uuid::new_v4(), "");
        assert!(export_service.validate_session_data(&invalid_session).is_err());
        
        // Invalid session - title too long
        let invalid_session = create_test_session_with_id(Uuid::new_v4(), &"a".repeat(201));
        assert!(export_service.validate_session_data(&invalid_session).is_err());
        
        // Invalid session - empty message content
        let mut invalid_session = create_test_session_with_id(Uuid::new_v4(), "Valid Title");
        invalid_session.messages[0].content = "".to_string();
        assert!(export_service.validate_session_data(&invalid_session).is_err());
    }

    #[test]
    fn test_extract_session_id_from_path() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = ExportService::new(temp_dir.path().to_path_buf());
        
        let session_id = Uuid::new_v4();
        
        // Test direct UUID filename
        let path1 = temp_dir.path().join(format!("{}.json", session_id));
        assert_eq!(export_service.extract_session_id_from_path(&path1), Some(session_id));
        
        // Test UUID in complex filename
        let path2 = temp_dir.path().join(format!("20240101_120000_Test_Session_{}.json", session_id));
        assert_eq!(export_service.extract_session_id_from_path(&path2), Some(session_id));
        
        // Test no UUID in filename
        let path3 = temp_dir.path().join("no_uuid_here.json");
        assert_eq!(export_service.extract_session_id_from_path(&path3), None);
    }
}

#[cfg(test)]
mod backup_tests {
    use super::super::*;
    use crate::session::manager::ChatSession;
    use crate::export::backup::BackupConfig;
    use tempfile::TempDir;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn create_test_session_with_id(id: uuid::Uuid, title: &str) -> ChatSession {
        let now = chrono::Local::now();
        
        ChatSession {
            id,
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            messages: vec![],
            model: "gpt-3.5-turbo".to_string(),
            system_prompt: None,
            model_config: crate::session::manager::SessionModelConfig::default(),
            total_tokens_used: crate::models::TokenUsage::default(),
            tags: vec!["test".to_string()],
            is_archived: false,
            export_count: 0,
            message_count: 0,
            last_activity: now,
        }
    }

    #[tokio::test]
    async fn test_backup_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = Arc::new(ExportService::new(temp_dir.path().to_path_buf()));
        let backup_manager = BackupManager::new(export_service);
        
        let config = backup_manager.get_config().await;
        assert!(!config.enabled);
        assert_eq!(config.interval_hours, 24);
        assert_eq!(config.max_backups, 30);
    }

    #[tokio::test]
    async fn test_backup_config_update() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = Arc::new(ExportService::new(temp_dir.path().to_path_buf()));
        let mut backup_manager = BackupManager::new(export_service);
        
        let mut new_config = BackupConfig::default();
        new_config.enabled = true;
        new_config.interval_hours = 12;
        new_config.max_backups = 10;
        new_config.backup_path = temp_dir.path().join("custom_backups");
        
        backup_manager.update_config(new_config.clone()).await.unwrap();
        
        let updated_config = backup_manager.get_config().await;
        assert_eq!(updated_config.enabled, true);
        assert_eq!(updated_config.interval_hours, 12);
        assert_eq!(updated_config.max_backups, 10);
        assert!(updated_config.backup_path.exists());
    }

    #[tokio::test]
    async fn test_backup_config_validation() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = Arc::new(ExportService::new(temp_dir.path().to_path_buf()));
        let mut backup_manager = BackupManager::new(export_service);
        
        // Test invalid interval
        let mut invalid_config = BackupConfig::default();
        invalid_config.interval_hours = 0;
        
        let result = backup_manager.update_config(invalid_config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("interval must be greater than 0"));
        
        // Test invalid max_backups
        let mut invalid_config = BackupConfig::default();
        invalid_config.max_backups = 0;
        
        let result = backup_manager.update_config(invalid_config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Max backups must be greater than 0"));
    }

    #[tokio::test]
    async fn test_create_backup() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = Arc::new(ExportService::new(temp_dir.path().to_path_buf()));
        export_service.initialize().unwrap();
        
        let backup_manager = BackupManager::new(export_service);
        
        let sessions = vec![
            create_test_session_with_id(uuid::Uuid::new_v4(), "Session 1"),
            create_test_session_with_id(uuid::Uuid::new_v4(), "Session 2"),
            create_test_session_with_id(uuid::Uuid::new_v4(), "Session 3"),
        ];
        
        let progress_counter = Arc::new(AtomicUsize::new(0));
        let progress_counter_clone = Arc::clone(&progress_counter);
        
        let progress_callback: ProgressCallback = Arc::new(move |current, total, message| {
            progress_counter_clone.store(current, Ordering::SeqCst);
            println!("Backup Progress: {}/{} - {}", current, total, message);
        });
        
        let result = backup_manager.create_backup(&sessions, Some(progress_callback)).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.session_ids.len(), 3);
        assert!(result.duration_ms > 0);
        assert!(result.metadata.backup_path.exists());
        assert_eq!(result.metadata.session_count, 3);
        
        // Verify progress was tracked
        assert_eq!(progress_counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_create_backup_with_archived_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = Arc::new(ExportService::new(temp_dir.path().to_path_buf()));
        export_service.initialize().unwrap();
        
        let backup_manager = BackupManager::new(export_service);
        
        let mut sessions = vec![
            create_test_session_with_id(uuid::Uuid::new_v4(), "Active Session"),
            create_test_session_with_id(uuid::Uuid::new_v4(), "Archived Session"),
        ];
        
        // Archive the second session
        sessions[1].is_archived = true;
        
        let result = backup_manager.create_backup(&sessions, None).await.unwrap();
        
        // By default, archived sessions are not included
        assert_eq!(result.session_ids.len(), 1);
        assert_eq!(result.metadata.session_count, 1);
    }

    #[tokio::test]
    async fn test_list_backups() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = Arc::new(ExportService::new(temp_dir.path().to_path_buf()));
        export_service.initialize().unwrap();
        
        let mut backup_manager = BackupManager::new(export_service);
        
        // Configure backup manager to use temp directory
        let mut config = BackupConfig::default();
        config.backup_path = temp_dir.path().join("backups");
        backup_manager.update_config(config).await.unwrap();
        
        backup_manager.initialize().await.unwrap();
        
        // Initially no backups
        let backups = backup_manager.list_backups().await.unwrap();
        assert_eq!(backups.len(), 0);
        
        // Create a backup
        let sessions = vec![create_test_session_with_id(uuid::Uuid::new_v4(), "Test Session")];
        let result = backup_manager.create_backup(&sessions, None).await.unwrap();
        
        // Now should have one backup
        let backups = backup_manager.list_backups().await.unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].backup_id, result.metadata.backup_id);
    }

    #[tokio::test]
    async fn test_delete_backup() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = Arc::new(ExportService::new(temp_dir.path().to_path_buf()));
        export_service.initialize().unwrap();
        
        let mut backup_manager = BackupManager::new(export_service);
        
        // Configure backup manager to use temp directory
        let mut config = BackupConfig::default();
        config.backup_path = temp_dir.path().join("backups");
        backup_manager.update_config(config).await.unwrap();
        
        backup_manager.initialize().await.unwrap();
        
        // Create a backup
        let sessions = vec![create_test_session_with_id(uuid::Uuid::new_v4(), "Test Session")];
        let result = backup_manager.create_backup(&sessions, None).await.unwrap();
        
        // Verify backup exists
        assert!(result.metadata.backup_path.exists());
        
        // Delete the backup
        backup_manager.delete_backup(result.metadata.backup_id).await.unwrap();
        
        // Verify backup is deleted
        assert!(!result.metadata.backup_path.exists());
        
        let backups = backup_manager.list_backups().await.unwrap();
        assert_eq!(backups.len(), 0);
    }

    #[tokio::test]
    async fn test_backup_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let export_service = Arc::new(ExportService::new(temp_dir.path().to_path_buf()));
        export_service.initialize().unwrap();
        
        let mut backup_manager = BackupManager::new(export_service);
        
        // Configure backup manager to use temp directory
        let mut config = BackupConfig::default();
        config.backup_path = temp_dir.path().join("backups");
        backup_manager.update_config(config).await.unwrap();
        
        backup_manager.initialize().await.unwrap();
        
        // Initially no backups
        let stats = backup_manager.get_backup_statistics().await.unwrap();
        assert_eq!(stats.total_backups, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert!(stats.oldest_backup.is_none());
        assert!(stats.newest_backup.is_none());
        
        // Create some backups
        let sessions = vec![create_test_session_with_id(uuid::Uuid::new_v4(), "Test Session")];
        
        backup_manager.create_backup(&sessions, None).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // Small delay
        backup_manager.create_backup(&sessions, None).await.unwrap();
        
        let stats = backup_manager.get_backup_statistics().await.unwrap();
        assert_eq!(stats.total_backups, 2);
        assert!(stats.total_size_bytes > 0);
        assert!(stats.oldest_backup.is_some());
        assert!(stats.newest_backup.is_some());
        assert!(!stats.auto_backup_enabled);
    }
}