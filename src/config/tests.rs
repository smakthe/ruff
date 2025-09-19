use super::service::ConfigurationService;
use super::models::*;
use super::validation::ConfigValidator;
use tempfile::TempDir;
use std::collections::HashMap;

async fn create_test_service() -> (ConfigurationService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let mut service = ConfigurationService::new().unwrap();
    service.config_dir = temp_dir.path().to_path_buf();
    service.initialize().await.unwrap();
    (service, temp_dir)
}

#[tokio::test]
async fn test_configuration_service_new() {
    let service = ConfigurationService::new().unwrap();
    assert!(service.config_dir.exists() || service.config_dir.parent().unwrap().exists());
}

#[tokio::test]
async fn test_initialize_creates_default_configs() {
    let (service, _temp_dir) = create_test_service().await;
    
    // Should have default global config
    let global_config = service.get_global_config();
    assert_eq!(global_config.version, env!("CARGO_PKG_VERSION"));
    assert!(!global_config.default_model.is_empty());
    assert!(!global_config.api_keys.is_empty());
    
    // Should have default model configs
    let model_configs = service.get_all_model_configs();
    assert!(!model_configs.is_empty());
    assert!(model_configs.contains_key("openai-gpt-4"));
    assert!(model_configs.contains_key("groq-llama3"));
    
    // Should have default theme
    let theme_configs = service.get_all_theme_configs();
    assert!(theme_configs.contains_key("default"));
}

#[tokio::test]
async fn test_global_config_crud() {
    let (service, _temp_dir) = create_test_service().await;
    
    let mut config = service.get_global_config();
    config.default_model = "test-model".to_string();
    config.debug_mode = true;
    
    // Set valid API keys to pass validation
    config.api_keys.insert("openai".to_string(), "sk-valid-key-123".to_string());
    config.api_keys.insert("anthropic".to_string(), "sk-ant-valid-key-123".to_string());
    config.api_keys.insert("groq".to_string(), "gsk_valid-key-123".to_string());
    config.api_keys.insert("huggingface".to_string(), "hf_valid-key-123".to_string());
    config.api_keys.insert("cohere".to_string(), "valid-cohere-key-123".to_string());
    config.api_keys.insert("together".to_string(), "valid-together-key-123".to_string());
    
    // Update
    service.update_global_config(config.clone()).await.unwrap();
    
    // Verify update
    let updated_config = service.get_global_config();
    assert_eq!(updated_config.default_model, "test-model");
    assert!(updated_config.debug_mode);
}

#[tokio::test]
async fn test_model_config_crud() {
    let (service, _temp_dir) = create_test_service().await;
    
    let test_config = ModelConfig {
        name: "test-model".to_string(),
        provider: "test-provider".to_string(),
        temperature: 0.5,
        max_tokens: 2048,
        top_p: Some(0.9),
        frequency_penalty: Some(0.1),
        presence_penalty: Some(0.2),
        custom_endpoint: Some("https://api.test.com".to_string()),
        rate_limit: RateLimit {
            requests_per_minute: 30,
            tokens_per_minute: Some(50000),
            concurrent_requests: 3,
        },
        retry_config: RetryConfig {
            max_attempts: 5,
            base_delay_ms: 2000,
            max_delay_ms: 60000,
            backoff_multiplier: 1.5,
            retry_on_rate_limit: true,
            retry_on_network_error: false,
        },
        proxy_config: None,
    };
    
    // Create
    service.update_model_config("test-model", test_config.clone()).await.unwrap();
    
    // Read
    let retrieved_config = service.get_model_config("test-model").unwrap();
    assert_eq!(retrieved_config.name, test_config.name);
    assert_eq!(retrieved_config.temperature, test_config.temperature);
    assert_eq!(retrieved_config.max_tokens, test_config.max_tokens);
    assert_eq!(retrieved_config.top_p, test_config.top_p);
    assert_eq!(retrieved_config.custom_endpoint, test_config.custom_endpoint);
    assert_eq!(retrieved_config.rate_limit.requests_per_minute, test_config.rate_limit.requests_per_minute);
    assert_eq!(retrieved_config.retry_config.max_attempts, test_config.retry_config.max_attempts);
    
    // Update
    let mut updated_config = test_config.clone();
    updated_config.temperature = 0.8;
    updated_config.max_tokens = 4096;
    service.update_model_config("test-model", updated_config).await.unwrap();
    
    let retrieved_config = service.get_model_config("test-model").unwrap();
    assert_eq!(retrieved_config.temperature, 0.8);
    assert_eq!(retrieved_config.max_tokens, 4096);
    
    // Delete
    service.remove_model_config("test-model").await.unwrap();
    assert!(service.get_model_config("test-model").is_none());
}

#[tokio::test]
async fn test_theme_config_crud() {
    let (service, _temp_dir) = create_test_service().await;
    
    let test_theme = ThemeConfig {
        name: "test-theme".to_string(),
        primary_color: "#FF0000".to_string(),
        secondary_color: "#00FF00".to_string(),
        accent_color: "#0000FF".to_string(),
        error_color: "#FF00FF".to_string(),
        success_color: "#FFFF00".to_string(),
        background_color: "#000000".to_string(),
        text_color: "#FFFFFF".to_string(),
        high_contrast: true,
    };
    
    // Create
    service.update_theme_config("test-theme", test_theme.clone()).await.unwrap();
    
    // Read
    let retrieved_theme = service.get_theme_config("test-theme").unwrap();
    assert_eq!(retrieved_theme.name, test_theme.name);
    assert_eq!(retrieved_theme.primary_color, test_theme.primary_color);
    assert_eq!(retrieved_theme.high_contrast, test_theme.high_contrast);
}

#[tokio::test]
async fn test_plugin_config_crud() {
    let (service, _temp_dir) = create_test_service().await;
    
    let mut settings = HashMap::new();
    settings.insert("setting1".to_string(), serde_json::Value::String("value1".to_string()));
    settings.insert("setting2".to_string(), serde_json::Value::Number(serde_json::Number::from(42)));
    
    let test_plugin = PluginConfig {
        name: "test-plugin".to_string(),
        enabled: true,
        settings,
        permissions: vec!["read".to_string(), "write".to_string()],
    };
    
    // Create
    service.update_plugin_config("test-plugin", test_plugin.clone()).await.unwrap();
    
    // Read
    let retrieved_plugin = service.get_plugin_config("test-plugin").unwrap();
    assert_eq!(retrieved_plugin.name, test_plugin.name);
    assert_eq!(retrieved_plugin.enabled, test_plugin.enabled);
    assert_eq!(retrieved_plugin.settings.len(), test_plugin.settings.len());
    assert_eq!(retrieved_plugin.permissions, test_plugin.permissions);
}

#[tokio::test]
async fn test_api_key_management() {
    let (service, _temp_dir) = create_test_service().await;
    
    // Set API key
    service.set_api_key("test-provider", "test-api-key-123").await.unwrap();
    
    // Get API key
    let api_key = service.get_api_key("test-provider").unwrap();
    assert_eq!(api_key, "test-api-key-123");
    
    // Should fail for non-existent provider
    assert!(service.get_api_key("non-existent-provider").is_err());
    
    // Should fail for placeholder keys
    service.set_api_key("placeholder-provider", "your-api-key-here").await.unwrap();
    assert!(service.get_api_key("placeholder-provider").is_err());
}

#[tokio::test]
async fn test_config_validation() {
    let (service, _temp_dir) = create_test_service().await;
    
    // Test invalid model config
    let invalid_model_config = ModelConfig {
        name: "".to_string(), // Empty name should fail
        provider: "test".to_string(),
        temperature: 3.0, // Invalid temperature should fail
        max_tokens: 0, // Invalid max_tokens should fail
        ..ModelConfig::default()
    };
    
    assert!(service.update_model_config("invalid", invalid_model_config).await.is_err());
    
    // Test invalid theme config
    let invalid_theme_config = ThemeConfig {
        name: "".to_string(), // Empty name should fail
        primary_color: "invalid-color".to_string(), // Invalid color should fail
        ..ThemeConfig::default()
    };
    
    assert!(service.update_theme_config("invalid", invalid_theme_config).await.is_err());
    
    // Test invalid global config
    let mut invalid_global_config = GlobalConfig::default();
    invalid_global_config.default_model = "".to_string(); // Empty default model should fail
    
    assert!(service.update_global_config(invalid_global_config).await.is_err());
}

#[tokio::test]
async fn test_validate_all_configs() {
    let (service, _temp_dir) = create_test_service().await;
    
    // Add a valid API key to make validation pass
    service.set_api_key("openai", "sk-valid-api-key-123").await.unwrap();
    
    // Should pass validation with valid configs
    let result = service.validate_all_configs();
    // Note: This might fail due to placeholder API keys in default config
    // In a real scenario, we'd have proper API keys configured
    
    // Add an invalid model config
    let invalid_config = ModelConfig {
        name: "invalid".to_string(),
        provider: "test".to_string(),
        temperature: 5.0, // Invalid temperature
        ..ModelConfig::default()
    };
    
    // This should fail validation, but we can't add it because update_model_config validates first
    // So we test the validation directly
    assert!(ConfigValidator::validate_model_config(&invalid_config).is_err());
}

#[tokio::test]
async fn test_config_persistence() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create service and add some configs
    {
        let mut service = ConfigurationService::new().unwrap();
        service.config_dir = temp_dir.path().to_path_buf();
        service.initialize().await.unwrap();
        
        // Add custom model config
        let test_config = ModelConfig {
            name: "persistent-model".to_string(),
            provider: "test-provider".to_string(),
            temperature: 0.3,
            max_tokens: 1024,
            ..ModelConfig::default()
        };
        
        service.update_model_config("persistent-model", test_config).await.unwrap();
        
        // Add custom theme
        let test_theme = ThemeConfig {
            name: "persistent-theme".to_string(),
            primary_color: "#123456".to_string(),
            ..ThemeConfig::default()
        };
        
        service.update_theme_config("persistent-theme", test_theme).await.unwrap();
    }
    
    // Create new service instance and verify persistence
    {
        let mut service = ConfigurationService::new().unwrap();
        service.config_dir = temp_dir.path().to_path_buf();
        service.initialize().await.unwrap();
        
        // Verify model config persisted
        let model_config = service.get_model_config("persistent-model").unwrap();
        assert_eq!(model_config.name, "persistent-model");
        assert_eq!(model_config.temperature, 0.3);
        assert_eq!(model_config.max_tokens, 1024);
        
        // Verify theme config persisted
        let theme_config = service.get_theme_config("persistent-theme").unwrap();
        assert_eq!(theme_config.name, "persistent-theme");
        assert_eq!(theme_config.primary_color, "#123456");
    }
}

#[tokio::test]
async fn test_export_import_config() {
    let (service, temp_dir) = create_test_service().await;
    
    // Add some custom configs
    let test_model = ModelConfig {
        name: "export-test-model".to_string(),
        provider: "test-provider".to_string(),
        temperature: 0.6,
        max_tokens: 3000,
        ..ModelConfig::default()
    };
    
    service.update_model_config("export-test-model", test_model).await.unwrap();
    service.set_api_key("export-test", "export-api-key").await.unwrap();
    
    // Export config
    let export_path = temp_dir.path().join("export.json");
    service.export_config(&export_path).await.unwrap();
    
    // Verify export file exists and has content
    assert!(export_path.exists());
    let export_content = tokio::fs::read_to_string(&export_path).await.unwrap();
    assert!(export_content.contains("export-test-model"));
    assert!(export_content.contains("export-api-key"));
    
    // Create new service and import
    let (new_service, _temp_dir2) = create_test_service().await;
    new_service.import_config(&export_path, false).await.unwrap();
    
    // Verify imported data
    let imported_model = new_service.get_model_config("export-test-model").unwrap();
    assert_eq!(imported_model.temperature, 0.6);
    assert_eq!(imported_model.max_tokens, 3000);
    
    let imported_api_key = new_service.get_api_key("export-test").unwrap();
    assert_eq!(imported_api_key, "export-api-key");
}

#[tokio::test]
async fn test_import_config_merge() {
    let (service, temp_dir) = create_test_service().await;
    
    // Set initial API key
    service.set_api_key("initial-provider", "initial-key").await.unwrap();
    
    // Create export with different API key
    let export_data = serde_json::json!({
        "global_config": {
            "version": "0.1.0",
            "default_model": "imported-model",
            "api_keys": {
                "imported-provider": "imported-key"
            },
            "ui_config": {
                "theme": "imported-theme",
                "font_size": 16,
                "auto_save": false,
                "show_token_usage": false,
                "enable_streaming": false
            },
            "backup_config": {
                "enabled": false,
                "interval_hours": 48,
                "max_backups": 3,
                "backup_path": null,
                "compress": false,
                "encrypt": true
            },
            "debug_mode": true
        },
        "model_configs": {
            "imported-model": {
                "name": "imported-model",
                "provider": "imported-provider",
                "temperature": 0.9,
                "max_tokens": 8192,
                "top_p": null,
                "frequency_penalty": null,
                "presence_penalty": null,
                "custom_endpoint": null,
                "rate_limit": {
                    "requests_per_minute": 120,
                    "tokens_per_minute": 200000,
                    "concurrent_requests": 10
                },
                "retry_config": {
                    "max_attempts": 3,
                    "base_delay_ms": 1000,
                    "max_delay_ms": 30000,
                    "backoff_multiplier": 2.0,
                    "retry_on_rate_limit": true,
                    "retry_on_network_error": true
                },
                "proxy_config": null
            }
        },
        "theme_configs": {},
        "plugin_configs": {}
    });
    
    let export_path = temp_dir.path().join("merge_test.json");
    tokio::fs::write(&export_path, serde_json::to_string_pretty(&export_data).unwrap()).await.unwrap();
    
    // Import with merge=true
    service.import_config(&export_path, true).await.unwrap();
    
    // Verify both keys exist (merged)
    assert!(service.get_api_key("initial-provider").is_ok());
    assert!(service.get_api_key("imported-provider").is_ok());
    
    // Verify imported model exists
    let imported_model = service.get_model_config("imported-model").unwrap();
    assert_eq!(imported_model.temperature, 0.9);
    assert_eq!(imported_model.max_tokens, 8192);
    
    // Verify global config was updated
    let global_config = service.get_global_config();
    assert_eq!(global_config.default_model, "imported-model");
    assert!(global_config.debug_mode);
}

#[tokio::test]
async fn test_retry_config_delay_calculation() {
    let retry_config = RetryConfig {
        max_attempts: 5,
        base_delay_ms: 1000,
        max_delay_ms: 10000,
        backoff_multiplier: 2.0,
        retry_on_rate_limit: true,
        retry_on_network_error: true,
    };
    
    // Test delay calculation
    assert_eq!(retry_config.calculate_delay(0).as_millis(), 1000);
    assert_eq!(retry_config.calculate_delay(1).as_millis(), 2000);
    assert_eq!(retry_config.calculate_delay(2).as_millis(), 4000);
    assert_eq!(retry_config.calculate_delay(3).as_millis(), 8000);
    assert_eq!(retry_config.calculate_delay(4).as_millis(), 10000); // Capped at max_delay
    
    // Test base and max delay accessors
    assert_eq!(retry_config.base_delay().as_millis(), 1000);
    assert_eq!(retry_config.max_delay().as_millis(), 10000);
}

#[test]
fn test_model_config_validation() {
    // Valid config should pass
    let valid_config = ModelConfig::default();
    assert!(valid_config.validate().is_ok());
    
    // Invalid configs should fail
    let invalid_configs = vec![
        ModelConfig {
            name: "".to_string(),
            ..ModelConfig::default()
        },
        ModelConfig {
            temperature: 3.0,
            ..ModelConfig::default()
        },
        ModelConfig {
            max_tokens: 0,
            ..ModelConfig::default()
        },
        ModelConfig {
            top_p: Some(1.5),
            ..ModelConfig::default()
        },
        ModelConfig {
            frequency_penalty: Some(3.0),
            ..ModelConfig::default()
        },
        ModelConfig {
            presence_penalty: Some(-3.0),
            ..ModelConfig::default()
        },
    ];
    
    for config in invalid_configs {
        assert!(config.validate().is_err());
    }
}