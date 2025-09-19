//! Integration tests for App component interaction
//! 
//! Tests the integration between all managers and services in the App struct

#[cfg(test)]
mod tests {
    use super::super::App;
    use crate::{
        events::AppEvent,
        session::manager::{Message, MessageRole, MessageMetadata},
        export::formats::ExportFormat,
        RuffError,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{sleep, Duration};
    use tempfile::TempDir;
    use uuid::Uuid;

    /// Test helper to create a test app instance
    async fn create_test_app() -> Result<(App, TempDir), RuffError> {
        // Create temporary directory for test data
        let temp_dir = TempDir::new().map_err(|e| RuffError::App(e.to_string()))?;
        
        // Set environment variables to use temp directory
        std::env::set_var("RUFF_DATA_DIR", temp_dir.path());
        std::env::set_var("RUFF_CONFIG_DIR", temp_dir.path());
        
        let app = App::new().await?;
        Ok((app, temp_dir))
    }

    #[tokio::test]
    async fn test_app_initialization() {
        let (app, _temp_dir) = create_test_app().await.unwrap();
        
        // Verify all components are initialized
        assert!(app.get_session_manager().session_count() >= 0);
        assert!(app.get_current_session_id().is_some());
        assert!(app.get_plugin_manager().is_some());
        assert!(!app.get_event_bus().is_closed());
    }

    #[tokio::test]
    async fn test_session_creation_and_switching() {
        let (mut app, _temp_dir) = create_test_app().await.unwrap();
        
        let initial_session_count = app.get_session_manager().session_count();
        
        // Create a new session
        app.handle_create_new_session().await.unwrap();
        
        // Verify session was created
        assert_eq!(app.get_session_manager().session_count(), initial_session_count + 1);
        assert!(app.get_current_session_id().is_some());
        
        // Create another session
        let session_id = app.get_session_manager().create_session(Some("Test Session".to_string())).await.unwrap();
        
        // Switch to the new session
        app.handle_switch_session(session_id).await.unwrap();
        
        // Verify we switched to the correct session
        assert_eq!(app.get_current_session_id(), Some(session_id));
        
        let current_session = app.get_current_session().unwrap();
        assert_eq!(current_session.title, "Test Session");
    }

    #[tokio::test]
    async fn test_message_handling_integration() {
        let (mut app, _temp_dir) = create_test_app().await.unwrap();
        
        let session_id = app.get_current_session_id().unwrap();
        let initial_message_count = app.get_message_manager().get_message_count(session_id);
        
        // Create a test message
        let test_message = Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "Test message".to_string(),
            timestamp: chrono::Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test-model".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };
        
        // Add message through message manager
        let message_id = app.get_message_manager_mut().add_message(session_id, test_message).await.unwrap();
        
        // Verify message was added
        assert_eq!(app.get_message_manager().get_message_count(session_id), initial_message_count + 1);
        assert!(app.get_message_manager().message_exists(session_id, message_id));
        
        // Verify message content
        let retrieved_message = app.get_message_manager().get_message(session_id, message_id).unwrap();
        assert_eq!(retrieved_message.content, "Test message");
        assert_eq!(retrieved_message.role, MessageRole::User);
    }

    #[tokio::test]
    async fn test_configuration_service_integration() {
        let (app, _temp_dir) = create_test_app().await.unwrap();
        
        let config_service = app.get_configuration_service();
        
        // Test global configuration
        let global_config = config_service.get_global_config();
        assert!(!global_config.default_model.is_empty());
        
        // Test model configurations
        let model_configs = config_service.get_all_model_configs();
        assert!(!model_configs.is_empty());
        
        // Test API key management (should fail for non-existent provider)
        let result = config_service.get_api_key("non-existent-provider");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_export_service_integration() {
        let (mut app, _temp_dir) = create_test_app().await.unwrap();
        
        let session_id = app.get_current_session_id().unwrap();
        
        // Add some test messages to the session
        let test_message = Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "Test export message".to_string(),
            timestamp: chrono::Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test-model".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };
        
        app.get_message_manager_mut().add_message(session_id, test_message).await.unwrap();
        
        // Test export functionality
        let result = app.handle_export_session(ExportFormat::Json).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_event_bus_integration() {
        let (app, _temp_dir) = create_test_app().await.unwrap();
        
        let event_bus = app.get_event_bus();
        
        // Test event publishing
        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();
        
        // Subscribe to session events
        let _subscription = event_bus.subscribe(move |event| {
            if matches!(event, AppEvent::SessionCreated(_)) {
                event_count_clone.fetch_add(1, Ordering::SeqCst);
            }
        }).await.unwrap();
        
        // Create a session (should trigger event)
        let _session_id = app.get_session_manager().create_session(Some("Event Test".to_string())).await.unwrap();
        
        // Give some time for event processing
        sleep(Duration::from_millis(100)).await;
        
        // Verify event was received
        assert_eq!(event_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_search_integration() {
        let (mut app, _temp_dir) = create_test_app().await.unwrap();
        
        let session_id = app.get_current_session_id().unwrap();
        
        // Add searchable content
        let test_message = Message {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: "This is a searchable test message with unique keywords".to_string(),
            timestamp: chrono::Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: MessageMetadata {
                model_used: "test-model".to_string(),
                temperature: 0.7,
                response_time_ms: 100,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };
        
        app.get_message_manager_mut().add_message(session_id, test_message).await.unwrap();
        
        // Update session metadata to trigger indexing
        app.get_session_manager_mut().update_session_metadata(session_id).await.unwrap();
        app.get_session_manager_mut().update_session_index(session_id);
        
        // Test session search
        let search_query = crate::session::search::SessionSearchQuery {
            text: "searchable".to_string(),
            filters: crate::session::search::SessionSearchFilters::default(),
            sort_by: crate::session::search::SessionSortBy::Relevance,
            limit: 10,
        };
        
        let search_results = app.get_session_manager().search_sessions(&search_query);
        assert!(!search_results.is_empty());
        
        // Verify the search found our session
        let found_session = search_results.iter().find(|result| result.session_id == session_id);
        assert!(found_session.is_some());
    }

    #[tokio::test]
    async fn test_template_manager_integration() {
        let (app, _temp_dir) = create_test_app().await.unwrap();
        
        let template_manager = app.get_template_manager();
        
        // Test template loading (should have built-in templates)
        let templates = template_manager.list_templates().await;
        assert!(!templates.is_empty());
        
        // Test template search
        let search_results = template_manager.search_templates("code", None).await;
        assert!(!search_results.is_empty());
        
        // Verify we found code-related templates
        let code_template = search_results.iter().find(|result| {
            result.template.name.to_lowercase().contains("code") ||
            result.template.description.to_lowercase().contains("code")
        });
        assert!(code_template.is_some());
    }

    #[tokio::test]
    async fn test_plugin_manager_integration() {
        let (app, _temp_dir) = create_test_app().await.unwrap();
        
        let plugin_manager = app.get_plugin_manager().unwrap();
        
        // Test plugin manager initialization
        assert!(plugin_manager.get_loaded_plugins().is_empty()); // No plugins loaded initially
        
        // Test UI extension manager
        let ui_extension_manager = plugin_manager.get_ui_extension_manager();
        assert_eq!(ui_extension_manager.get_extension_count(), 0); // No extensions initially
    }

    #[tokio::test]
    async fn test_model_switching_integration() {
        let (mut app, _temp_dir) = create_test_app().await.unwrap();
        
        let initial_model = app.current_model_key.clone();
        
        // Get available models
        let available_models = app.model_registry.list_models();
        if available_models.len() > 1 {
            // Find a different model to switch to
            let different_model = available_models.iter()
                .find(|(key, _)| *key != &initial_model)
                .map(|(key, _)| key.clone());
            
            if let Some(new_model) = different_model {
                // Test model switching
                let result = app.handle_select_model(new_model.clone()).await;
                
                // Note: This might fail due to missing API keys, which is expected
                match result {
                    Ok(_) => {
                        assert_eq!(app.current_model_key, new_model);
                        
                        // Verify session was updated
                        if let Some(session) = app.get_current_session() {
                            assert_eq!(session.model, new_model);
                        }
                    }
                    Err(RuffError::InvalidApiKey { .. }) => {
                        // Expected if API key is not configured
                        println!("Model switch failed due to missing API key (expected in tests)");
                    }
                    Err(e) => panic!("Unexpected error: {}", e),
                }
            }
        }
    }

    #[tokio::test]
    async fn test_session_persistence() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create first app instance
        {
            std::env::set_var("RUFF_DATA_DIR", temp_dir.path());
            std::env::set_var("RUFF_CONFIG_DIR", temp_dir.path());
            
            let mut app = App::new().await.unwrap();
            
            // Create a session with specific content
            let session_id = app.get_session_manager_mut().create_session(Some("Persistence Test".to_string())).await.unwrap();
            
            let test_message = Message {
                id: Uuid::new_v4(),
                role: MessageRole::User,
                content: "This message should persist".to_string(),
                timestamp: chrono::Local::now(),
                edited_at: None,
                token_usage: None,
                parent_id: None,
                children: Vec::new(),
                metadata: MessageMetadata {
                    model_used: "test-model".to_string(),
                    temperature: 0.7,
                    response_time_ms: 100,
                    is_regenerated: false,
                    regeneration_count: 0,
                },
            };
            
            app.get_message_manager_mut().add_message(session_id, test_message).await.unwrap();
            
            // Save all sessions
            app.get_session_manager().save_all_sessions().await.unwrap();
        }
        
        // Create second app instance (should load persisted data)
        {
            let app = App::new().await.unwrap();
            
            // Verify session was loaded
            let sessions = app.get_session_manager().get_all_sessions();
            let persistence_session = sessions.iter().find(|s| s.title == "Persistence Test");
            assert!(persistence_session.is_some());
            
            let session = persistence_session.unwrap();
            assert_eq!(session.message_count, 1);
        }
    }
}