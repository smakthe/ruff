use crate::RuffError;
use super::models::{ProxyConfig, ModelConfig};
use super::validation::ConfigValidator;
use reqwest::{Client, ClientBuilder, Proxy};
use std::time::Duration;
use url::Url;

/// Network configuration and client management
pub struct NetworkConfig {
    client: Client,
    proxy_config: Option<ProxyConfig>,
    custom_endpoints: std::collections::HashMap<String, String>,
    connection_timeout: Duration,
    request_timeout: Duration,
}

impl NetworkConfig {
    /// Create a new network configuration
    pub fn new() -> Result<Self, RuffError> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        
        Ok(Self {
            client,
            proxy_config: None,
            custom_endpoints: std::collections::HashMap::new(),
            connection_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
        })
    }
    
    /// Configure proxy settings
    pub fn set_proxy(&mut self, proxy_config: ProxyConfig) -> Result<(), RuffError> {
        // Validate proxy configuration
        ConfigValidator::validate_proxy_config(&proxy_config)
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        // Create new client with proxy
        let mut client_builder = ClientBuilder::new()
            .timeout(self.request_timeout)
            .connect_timeout(self.connection_timeout);
        
        // Parse proxy URL
        let proxy_url = Url::parse(&proxy_config.url)
            .map_err(|e| RuffError::App(format!("Invalid proxy URL: {}", e)))?;
        
        // Create proxy
        let mut proxy = match proxy_url.scheme() {
            "http" | "https" => Proxy::http(&proxy_config.url)?,
            "socks5" => Proxy::all(&proxy_config.url)?,
            scheme => return Err(RuffError::App(format!("Unsupported proxy scheme: {}", scheme))),
        };
        
        // Add authentication if provided
        if let (Some(username), Some(password)) = (&proxy_config.username, &proxy_config.password) {
            proxy = proxy.basic_auth(username, password);
        }
        
        client_builder = client_builder.proxy(proxy);
        
        // Add no_proxy settings
        if !proxy_config.no_proxy.is_empty() {
            client_builder = client_builder.no_proxy();
        }
        
        self.client = client_builder.build()?;
        self.proxy_config = Some(proxy_config);
        
        Ok(())
    }
    
    /// Remove proxy configuration
    pub fn remove_proxy(&mut self) -> Result<(), RuffError> {
        self.client = ClientBuilder::new()
            .timeout(self.request_timeout)
            .connect_timeout(self.connection_timeout)
            .build()?;
        
        self.proxy_config = None;
        Ok(())
    }
    
    /// Set custom endpoint for a model
    pub fn set_custom_endpoint(&mut self, model_name: &str, endpoint: &str) -> Result<(), RuffError> {
        // Validate endpoint URL
        ConfigValidator::validate_custom_endpoint(endpoint)
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        self.custom_endpoints.insert(model_name.to_string(), endpoint.to_string());
        Ok(())
    }
    
    /// Remove custom endpoint for a model
    pub fn remove_custom_endpoint(&mut self, model_name: &str) {
        self.custom_endpoints.remove(model_name);
    }
    
    /// Get custom endpoint for a model
    pub fn get_custom_endpoint(&self, model_name: &str) -> Option<&str> {
        self.custom_endpoints.get(model_name).map(|s| s.as_str())
    }
    
    /// Get the configured HTTP client
    pub fn get_client(&self) -> &Client {
        &self.client
    }
    
    /// Get proxy configuration
    pub fn get_proxy_config(&self) -> Option<&ProxyConfig> {
        self.proxy_config.as_ref()
    }
    
    /// Test connection to a custom endpoint
    pub async fn test_custom_endpoint(&self, endpoint: &str) -> Result<bool, RuffError> {
        // Validate endpoint first
        ConfigValidator::validate_custom_endpoint(endpoint)
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        // Try to make a simple HEAD request to test connectivity
        let response = self.client
            .head(endpoint)
            .timeout(Duration::from_secs(5))
            .send()
            .await;
        
        match response {
            Ok(resp) => {
                // Consider 2xx, 4xx responses as successful connection
                // 4xx means the endpoint exists but may require authentication
                Ok(resp.status().as_u16() < 500)
            }
            Err(e) => {
                if e.is_timeout() {
                    Err(RuffError::App("Connection timeout".to_string()))
                } else if e.is_connect() {
                    Err(RuffError::App("Connection failed".to_string()))
                } else {
                    Err(RuffError::Network(e))
                }
            }
        }
    }
    
    /// Test proxy connection
    pub async fn test_proxy_connection(&self) -> Result<bool, RuffError> {
        if self.proxy_config.is_none() {
            return Err(RuffError::App("No proxy configured".to_string()));
        }
        
        // Test connection through proxy by making a request to a known endpoint
        let test_url = "https://httpbin.org/ip";
        
        let response = self.client
            .get(test_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await;
        
        match response {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(e) => {
                if e.is_timeout() {
                    Err(RuffError::App("Proxy connection timeout".to_string()))
                } else if e.is_connect() {
                    Err(RuffError::App("Proxy connection failed".to_string()))
                } else {
                    Err(RuffError::Network(e))
                }
            }
        }
    }
    
    /// Set connection timeout
    pub fn set_connection_timeout(&mut self, timeout: Duration) -> Result<(), RuffError> {
        self.connection_timeout = timeout;
        self.rebuild_client()
    }
    
    /// Set request timeout
    pub fn set_request_timeout(&mut self, timeout: Duration) -> Result<(), RuffError> {
        self.request_timeout = timeout;
        self.rebuild_client()
    }
    
    /// Get connection timeout
    pub fn get_connection_timeout(&self) -> Duration {
        self.connection_timeout
    }
    
    /// Get request timeout
    pub fn get_request_timeout(&self) -> Duration {
        self.request_timeout
    }
    
    /// Rebuild the HTTP client with current settings
    fn rebuild_client(&mut self) -> Result<(), RuffError> {
        let mut client_builder = ClientBuilder::new()
            .timeout(self.request_timeout)
            .connect_timeout(self.connection_timeout);
        
        // Re-apply proxy if configured
        if let Some(ref proxy_config) = self.proxy_config {
            let proxy_url = Url::parse(&proxy_config.url)
                .map_err(|e| RuffError::App(format!("Invalid proxy URL: {}", e)))?;
            
            let mut proxy = match proxy_url.scheme() {
                "http" | "https" => Proxy::http(&proxy_config.url)?,
                "socks5" => Proxy::all(&proxy_config.url)?,
                scheme => return Err(RuffError::App(format!("Unsupported proxy scheme: {}", scheme))),
            };
            
            if let (Some(username), Some(password)) = (&proxy_config.username, &proxy_config.password) {
                proxy = proxy.basic_auth(username, password);
            }
            
            client_builder = client_builder.proxy(proxy);
            
            if !proxy_config.no_proxy.is_empty() {
                client_builder = client_builder.no_proxy();
            }
        }
        
        self.client = client_builder.build()?;
        Ok(())
    }
    
    /// Get effective endpoint for a model (custom or default)
    pub fn get_effective_endpoint(&self, model_config: &ModelConfig) -> String {
        if let Some(custom_endpoint) = &model_config.custom_endpoint {
            custom_endpoint.clone()
        } else if let Some(custom_endpoint) = self.get_custom_endpoint(&model_config.name) {
            custom_endpoint.to_string()
        } else {
            // Return default endpoint based on provider
            self.get_default_endpoint(&model_config.provider)
        }
    }
    
    /// Get default endpoint for a provider
    pub fn get_default_endpoint(&self, provider: &str) -> String {
        match provider {
            "openai" => "https://api.openai.com/v1".to_string(),
            "anthropic" => "https://api.anthropic.com/v1".to_string(),
            "cohere" => "https://api.cohere.ai/v1".to_string(),
            "together" => "https://api.together.xyz/v1".to_string(),
            "groq" => "https://api.groq.com/openai/v1".to_string(),
            "huggingface" => "https://api-inference.huggingface.co/models".to_string(),
            _ => format!("https://api.{}.com/v1", provider),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self::new().expect("Failed to create default network config")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::ProxyConfig;
    
    #[test]
    fn test_network_config_new() {
        let config = NetworkConfig::new().unwrap();
        assert!(config.proxy_config.is_none());
        assert!(config.custom_endpoints.is_empty());
        assert_eq!(config.connection_timeout, Duration::from_secs(10));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
    }
    
    #[test]
    fn test_custom_endpoint_management() {
        let mut config = NetworkConfig::new().unwrap();
        
        // Set custom endpoint
        config.set_custom_endpoint("test-model", "https://api.test.com/v1").unwrap();
        assert_eq!(config.get_custom_endpoint("test-model"), Some("https://api.test.com/v1"));
        
        // Remove custom endpoint
        config.remove_custom_endpoint("test-model");
        assert_eq!(config.get_custom_endpoint("test-model"), None);
    }
    
    #[test]
    fn test_invalid_custom_endpoint() {
        let mut config = NetworkConfig::new().unwrap();
        
        // Invalid URL should fail
        assert!(config.set_custom_endpoint("test-model", "not-a-url").is_err());
        
        // Non-HTTP(S) URL should fail
        assert!(config.set_custom_endpoint("test-model", "ftp://example.com").is_err());
    }
    
    #[test]
    fn test_proxy_config_validation() {
        let mut config = NetworkConfig::new().unwrap();
        
        // Valid proxy config
        let valid_proxy = ProxyConfig {
            url: "http://proxy.example.com:8080".to_string(),
            username: None,
            password: None,
            no_proxy: vec![],
        };
        
        assert!(config.set_proxy(valid_proxy).is_ok());
        assert!(config.get_proxy_config().is_some());
        
        // Remove proxy
        config.remove_proxy().unwrap();
        assert!(config.get_proxy_config().is_none());
    }
    
    #[test]
    fn test_invalid_proxy_config() {
        let mut config = NetworkConfig::new().unwrap();
        
        // Invalid proxy URL
        let invalid_proxy = ProxyConfig {
            url: "not-a-url".to_string(),
            username: None,
            password: None,
            no_proxy: vec![],
        };
        
        assert!(config.set_proxy(invalid_proxy).is_err());
    }
    
    #[test]
    fn test_timeout_configuration() {
        let mut config = NetworkConfig::new().unwrap();
        
        let new_connection_timeout = Duration::from_secs(5);
        let new_request_timeout = Duration::from_secs(60);
        
        config.set_connection_timeout(new_connection_timeout).unwrap();
        config.set_request_timeout(new_request_timeout).unwrap();
        
        assert_eq!(config.get_connection_timeout(), new_connection_timeout);
        assert_eq!(config.get_request_timeout(), new_request_timeout);
    }
    
    #[test]
    fn test_default_endpoints() {
        let config = NetworkConfig::new().unwrap();
        
        assert_eq!(config.get_default_endpoint("openai"), "https://api.openai.com/v1");
        assert_eq!(config.get_default_endpoint("anthropic"), "https://api.anthropic.com/v1");
        assert_eq!(config.get_default_endpoint("groq"), "https://api.groq.com/openai/v1");
        assert_eq!(config.get_default_endpoint("unknown"), "https://api.unknown.com/v1");
    }
    
    #[test]
    fn test_effective_endpoint() {
        let mut config = NetworkConfig::new().unwrap();
        
        // Test with default endpoint
        let model_config = ModelConfig {
            name: "test-model".to_string(),
            provider: "openai".to_string(),
            custom_endpoint: None,
            ..ModelConfig::default()
        };
        
        assert_eq!(config.get_effective_endpoint(&model_config), "https://api.openai.com/v1");
        
        // Test with custom endpoint in model config
        let model_config_with_custom = ModelConfig {
            name: "test-model".to_string(),
            provider: "openai".to_string(),
            custom_endpoint: Some("https://custom.api.com/v1".to_string()),
            ..ModelConfig::default()
        };
        
        assert_eq!(config.get_effective_endpoint(&model_config_with_custom), "https://custom.api.com/v1");
        
        // Test with custom endpoint in network config
        config.set_custom_endpoint("test-model", "https://network.custom.com/v1").unwrap();
        assert_eq!(config.get_effective_endpoint(&model_config), "https://network.custom.com/v1");
        
        // Model config custom endpoint should take precedence
        assert_eq!(config.get_effective_endpoint(&model_config_with_custom), "https://custom.api.com/v1");
    }
    
    #[tokio::test]
    async fn test_connection_testing() {
        let config = NetworkConfig::new().unwrap();
        
        // Test with a known good endpoint (this might fail in CI without internet)
        // In a real test environment, you'd use a mock server
        let result = config.test_custom_endpoint("https://httpbin.org/status/200").await;
        // Don't assert success since this depends on internet connectivity
        // Just ensure it doesn't panic
        let _ = result;
        
        // Test with invalid endpoint
        let result = config.test_custom_endpoint("not-a-url").await;
        assert!(result.is_err());
    }
}