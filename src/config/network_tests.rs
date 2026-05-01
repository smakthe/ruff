use super::*;
use tempfile::TempDir;

async fn create_test_service() -> (ConfigurationService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let mut service = ConfigurationService::new().unwrap();
    service.config_dir = temp_dir.path().to_path_buf();
    service.initialize().await.unwrap();
    (service, temp_dir)
}

#[tokio::test]
async fn test_proxy_configuration() {
    let (service, _temp_dir) = create_test_service().await;

    let proxy_config = ProxyConfig {
        url: "http://proxy.example.com:8080".to_string(),
        username: Some("user".to_string()),
        password: Some("pass".to_string()),
        no_proxy: vec!["localhost".to_string(), "127.0.0.1".to_string()],
    };

    // Set proxy config
    service.set_proxy_config(proxy_config.clone()).unwrap();

    // Verify proxy config
    let retrieved_config = service.get_proxy_config().unwrap();
    assert_eq!(retrieved_config.url, proxy_config.url);
    assert_eq!(retrieved_config.username, proxy_config.username);
    assert_eq!(retrieved_config.password, proxy_config.password);
    assert_eq!(retrieved_config.no_proxy, proxy_config.no_proxy);

    // Remove proxy config
    service.remove_proxy_config().unwrap();
    assert!(service.get_proxy_config().is_none());
}

#[tokio::test]
async fn test_invalid_proxy_configuration() {
    let (service, _temp_dir) = create_test_service().await;

    let invalid_proxy_config = ProxyConfig {
        url: "not-a-valid-url".to_string(),
        username: None,
        password: None,
        no_proxy: vec![],
    };

    // Should fail with invalid URL
    assert!(service.set_proxy_config(invalid_proxy_config).is_err());
}

#[tokio::test]
async fn test_custom_endpoint_management() {
    let (service, _temp_dir) = create_test_service().await;

    let model_name = "test-model";
    let custom_endpoint = "https://api.custom.com/v1";

    // Set custom endpoint
    service
        .set_custom_endpoint(model_name, custom_endpoint)
        .unwrap();

    // Verify custom endpoint
    let retrieved_endpoint = service.get_custom_endpoint(model_name).unwrap();
    assert_eq!(retrieved_endpoint, custom_endpoint);

    // Remove custom endpoint
    service.remove_custom_endpoint(model_name);
    assert!(service.get_custom_endpoint(model_name).is_none());
}

#[tokio::test]
async fn test_invalid_custom_endpoint() {
    let (service, _temp_dir) = create_test_service().await;

    // Should fail with invalid URL
    assert!(service
        .set_custom_endpoint("test-model", "not-a-url")
        .is_err());

    // Should fail with non-HTTP(S) URL
    assert!(service
        .set_custom_endpoint("test-model", "ftp://example.com")
        .is_err());
}

#[tokio::test]
async fn test_effective_endpoint_resolution() {
    let (service, _temp_dir) = create_test_service().await;

    // Create a test model config
    let model_config = ModelConfig {
        name: "test-model".to_string(),
        provider: "openai".to_string(),
        custom_endpoint: None,
        ..ModelConfig::default()
    };

    service
        .update_model_config("test-model", model_config)
        .await
        .unwrap();

    // Should return default endpoint for provider
    let endpoint = service.get_effective_endpoint("test-model").unwrap();
    assert_eq!(endpoint, "https://api.openai.com/v1");

    // Set custom endpoint in network config
    service
        .set_custom_endpoint("test-model", "https://custom.api.com/v1")
        .unwrap();
    let endpoint = service.get_effective_endpoint("test-model").unwrap();
    assert_eq!(endpoint, "https://custom.api.com/v1");

    // Set custom endpoint in model config (should take precedence)
    let mut model_config_with_custom = service.get_model_config("test-model").unwrap();
    model_config_with_custom.custom_endpoint = Some("https://model.custom.com/v1".to_string());
    service
        .update_model_config("test-model", model_config_with_custom)
        .await
        .unwrap();

    let endpoint = service.get_effective_endpoint("test-model").unwrap();
    assert_eq!(endpoint, "https://model.custom.com/v1");
}

#[tokio::test]
async fn test_effective_endpoint_nonexistent_model() {
    let (service, _temp_dir) = create_test_service().await;

    // Should fail for non-existent model
    assert!(service
        .get_effective_endpoint("non-existent-model")
        .is_err());
}

#[tokio::test]
async fn test_connection_testing() {
    let (service, _temp_dir) = create_test_service().await;

    // Test with invalid endpoint
    let result = service.test_custom_endpoint("not-a-url").await;
    assert!(result.is_err());

    // Test with valid but unreachable endpoint (should not panic)
    let result = service
        .test_custom_endpoint("https://nonexistent.example.com")
        .await;
    // Don't assert success/failure as it depends on network conditions
    let _ = result;
}

#[tokio::test]
async fn test_proxy_connection_testing() {
    let (service, _temp_dir) = create_test_service().await;

    // Should fail when no proxy is configured
    let result = service.test_proxy_connection().await;
    assert!(result.is_err());

    // Configure a proxy (this won't actually work in tests, but shouldn't panic)
    let proxy_config = ProxyConfig {
        url: "http://proxy.example.com:8080".to_string(),
        username: None,
        password: None,
        no_proxy: vec![],
    };

    service.set_proxy_config(proxy_config).unwrap();

    // Test proxy connection (will likely fail, but shouldn't panic)
    let result = service.test_proxy_connection().await;
    let _ = result; // Don't assert as it depends on network conditions
}

#[tokio::test]
async fn test_http_client_access() {
    let (service, _temp_dir) = create_test_service().await;

    // Should be able to get HTTP client
    let client = service.get_http_client();

    // Client should be usable (basic smoke test)
    let request = client.get("https://httpbin.org/status/200");
    let _ = request; // Don't actually send the request in tests
}

#[tokio::test]
async fn test_proxy_with_authentication() {
    let (service, _temp_dir) = create_test_service().await;

    let proxy_config = ProxyConfig {
        url: "http://proxy.example.com:8080".to_string(),
        username: Some("testuser".to_string()),
        password: Some("testpass".to_string()),
        no_proxy: vec!["*.local".to_string(), "127.0.0.1".to_string()],
    };

    // Should successfully configure proxy with auth
    assert!(service.set_proxy_config(proxy_config.clone()).is_ok());

    let retrieved_config = service.get_proxy_config().unwrap();
    assert_eq!(retrieved_config.username, Some("testuser".to_string()));
    assert_eq!(retrieved_config.password, Some("testpass".to_string()));
}

#[tokio::test]
async fn test_socks5_proxy_configuration() {
    let (service, _temp_dir) = create_test_service().await;

    let socks_proxy_config = ProxyConfig {
        url: "socks5://proxy.example.com:1080".to_string(),
        username: None,
        password: None,
        no_proxy: vec![],
    };

    // Should successfully configure SOCKS5 proxy
    assert!(service.set_proxy_config(socks_proxy_config).is_ok());
}

#[tokio::test]
async fn test_unsupported_proxy_scheme() {
    let (service, _temp_dir) = create_test_service().await;

    let invalid_proxy_config = ProxyConfig {
        url: "ftp://proxy.example.com:21".to_string(),
        username: None,
        password: None,
        no_proxy: vec![],
    };

    // Should fail with unsupported scheme
    assert!(service.set_proxy_config(invalid_proxy_config).is_err());
}

#[test]
fn test_default_provider_endpoints() {
    let network_config = NetworkConfig::new().unwrap();

    // Test all supported providers have correct default endpoints
    assert_eq!(
        network_config.get_default_endpoint("openai"),
        "https://api.openai.com/v1"
    );
    assert_eq!(
        network_config.get_default_endpoint("anthropic"),
        "https://api.anthropic.com/v1"
    );
    assert_eq!(
        network_config.get_default_endpoint("cohere"),
        "https://api.cohere.ai/v1"
    );
    assert_eq!(
        network_config.get_default_endpoint("together"),
        "https://api.together.xyz/v1"
    );
    assert_eq!(
        network_config.get_default_endpoint("groq"),
        "https://api.groq.com/openai/v1"
    );
    assert_eq!(
        network_config.get_default_endpoint("huggingface"),
        "https://api-inference.huggingface.co/models"
    );

    // Test unknown provider gets generic endpoint
    assert_eq!(
        network_config.get_default_endpoint("unknown"),
        "https://api.unknown.com/v1"
    );
}

#[tokio::test]
async fn test_model_config_with_proxy() {
    let (service, _temp_dir) = create_test_service().await;

    // Create model config with proxy
    let model_config = ModelConfig {
        name: "proxy-model".to_string(),
        provider: "openai".to_string(),
        proxy_config: Some(ProxyConfig {
            url: "http://model-proxy.example.com:8080".to_string(),
            username: None,
            password: None,
            no_proxy: vec![],
        }),
        ..ModelConfig::default()
    };

    // Should successfully save model config with proxy
    assert!(service
        .update_model_config("proxy-model", model_config.clone())
        .await
        .is_ok());

    // Verify proxy config is saved
    let retrieved_config = service.get_model_config("proxy-model").unwrap();
    assert!(retrieved_config.proxy_config.is_some());
    assert_eq!(
        retrieved_config.proxy_config.unwrap().url,
        "http://model-proxy.example.com:8080"
    );
}
