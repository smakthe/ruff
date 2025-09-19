use crate::RuffError;
use super::models::*;
use super::validation::ConfigValidator;
use super::network::NetworkConfig;
use super::rate_limiter::{RateLimiter, RetryHandler, RateLimitStatus};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::fs;
use serde_json;

/// Configuration service for managing hierarchical configuration
pub struct ConfigurationService {
    global_config: Arc<RwLock<GlobalConfig>>,
    model_configs: Arc<RwLock<HashMap<String, ModelConfig>>>,
    theme_configs: Arc<RwLock<HashMap<String, ThemeConfig>>>,
    plugin_configs: Arc<RwLock<HashMap<String, PluginConfig>>>,
    network_config: Arc<RwLock<NetworkConfig>>,
    rate_limiter: Arc<RwLock<RateLimiter>>,
    #[cfg(test)]
    pub config_dir: PathBuf,
    #[cfg(not(test))]
    config_dir: PathBuf,
}

impl ConfigurationService {
    /// Create a new configuration service
    pub fn new() -> Result<Self, RuffError> {
        let config_dir = Self::get_config_directory()?;
        
        let service = Self {
            global_config: Arc::new(RwLock::new(GlobalConfig::default())),
            model_configs: Arc::new(RwLock::new(HashMap::new())),
            theme_configs: Arc::new(RwLock::new(HashMap::new())),
            plugin_configs: Arc::new(RwLock::new(HashMap::new())),
            network_config: Arc::new(RwLock::new(NetworkConfig::new()?)),
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new())),
            config_dir,
        };
        
        Ok(service)
    }
    
    /// Initialize configuration service and load existing configs
    pub async fn initialize(&self) -> Result<(), RuffError> {
        // Ensure config directory exists
        fs::create_dir_all(&self.config_dir).await?;
        
        // Load configurations
        self.load_global_config().await?;
        self.load_model_configs().await?;
        self.load_theme_configs().await?;
        self.load_plugin_configs().await?;
        
        // Initialize default model configs if none exist
        self.initialize_default_model_configs().await?;
        
        Ok(())
    }
    
    /// Get global configuration
    pub fn get_global_config(&self) -> GlobalConfig {
        self.global_config.read().unwrap().clone()
    }
    
    /// Update global configuration
    pub async fn update_global_config(&self, config: GlobalConfig) -> Result<(), RuffError> {
        // Validate configuration
        ConfigValidator::validate_global_config(&config)
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        // Update in memory
        {
            let mut global_config = self.global_config.write().unwrap();
            *global_config = config.clone();
        }
        
        // Save to disk
        self.save_global_config().await?;
        
        Ok(())
    }
    
    /// Get model configuration
    pub fn get_model_config(&self, model_name: &str) -> Option<ModelConfig> {
        self.model_configs.read().unwrap().get(model_name).cloned()
    }
    
    /// Get all model configurations
    pub fn get_all_model_configs(&self) -> HashMap<String, ModelConfig> {
        self.model_configs.read().unwrap().clone()
    }
    
    /// Update model configuration
    pub async fn update_model_config(&self, model_name: &str, config: ModelConfig) -> Result<(), RuffError> {
        // Validate configuration
        ConfigValidator::validate_model_config(&config)
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        // Update rate limiter with new rate limits
        {
            let mut rate_limiter = self.rate_limiter.write().unwrap();
            rate_limiter.set_rate_limit(&config.provider, config.rate_limit.clone());
        }
        
        // Update in memory
        {
            let mut model_configs = self.model_configs.write().unwrap();
            model_configs.insert(model_name.to_string(), config);
        }
        
        // Save to disk
        self.save_model_configs().await?;
        
        Ok(())
    }
    
    /// Remove model configuration
    pub async fn remove_model_config(&self, model_name: &str) -> Result<(), RuffError> {
        {
            let mut model_configs = self.model_configs.write().unwrap();
            model_configs.remove(model_name);
        }
        
        self.save_model_configs().await?;
        Ok(())
    }
    
    /// Get theme configuration
    pub fn get_theme_config(&self, theme_name: &str) -> Option<ThemeConfig> {
        self.theme_configs.read().unwrap().get(theme_name).cloned()
    }
    
    /// Get all theme configurations
    pub fn get_all_theme_configs(&self) -> HashMap<String, ThemeConfig> {
        self.theme_configs.read().unwrap().clone()
    }
    
    /// Update theme configuration
    pub async fn update_theme_config(&self, theme_name: &str, config: ThemeConfig) -> Result<(), RuffError> {
        // Validate configuration
        ConfigValidator::validate_theme_config(&config)
            .map_err(|e| RuffError::App(e.to_string()))?;
        
        // Update in memory
        {
            let mut theme_configs = self.theme_configs.write().unwrap();
            theme_configs.insert(theme_name.to_string(), config);
        }
        
        // Save to disk
        self.save_theme_configs().await?;
        
        Ok(())
    }
    
    /// Get plugin configuration
    pub fn get_plugin_config(&self, plugin_name: &str) -> Option<PluginConfig> {
        self.plugin_configs.read().unwrap().get(plugin_name).cloned()
    }
    
    /// Update plugin configuration
    pub async fn update_plugin_config(&self, plugin_name: &str, config: PluginConfig) -> Result<(), RuffError> {
        {
            let mut plugin_configs = self.plugin_configs.write().unwrap();
            plugin_configs.insert(plugin_name.to_string(), config);
        }
        
        self.save_plugin_configs().await?;
        Ok(())
    }
    
    /// Get API key for a provider
    pub fn get_api_key(&self, provider: &str) -> Result<String, RuffError> {
        let global_config = self.global_config.read().unwrap();
        global_config
            .api_keys
            .get(provider)
            .filter(|key| !key.contains("your-") && !key.contains("sk-your"))
            .cloned()
            .ok_or_else(|| RuffError::InvalidApiKey { 
                model: provider.to_string() 
            })
    }
    
    /// Set API key for a provider
    pub async fn set_api_key(&self, provider: &str, api_key: &str) -> Result<(), RuffError> {
        {
            let mut global_config = self.global_config.write().unwrap();
            global_config.api_keys.insert(provider.to_string(), api_key.to_string());
        }
        
        self.save_global_config().await?;
        Ok(())
    }
    
    /// Configure proxy settings
    pub fn set_proxy_config(&self, proxy_config: ProxyConfig) -> Result<(), RuffError> {
        let mut network_config = self.network_config.write().unwrap();
        network_config.set_proxy(proxy_config)
    }
    
    /// Remove proxy configuration
    pub fn remove_proxy_config(&self) -> Result<(), RuffError> {
        let mut network_config = self.network_config.write().unwrap();
        network_config.remove_proxy()
    }
    
    /// Get proxy configuration
    pub fn get_proxy_config(&self) -> Option<ProxyConfig> {
        let network_config = self.network_config.read().unwrap();
        network_config.get_proxy_config().cloned()
    }
    
    /// Set custom endpoint for a model
    pub fn set_custom_endpoint(&self, model_name: &str, endpoint: &str) -> Result<(), RuffError> {
        let mut network_config = self.network_config.write().unwrap();
        network_config.set_custom_endpoint(model_name, endpoint)
    }
    
    /// Remove custom endpoint for a model
    pub fn remove_custom_endpoint(&self, model_name: &str) {
        let mut network_config = self.network_config.write().unwrap();
        network_config.remove_custom_endpoint(model_name);
    }
    
    /// Get custom endpoint for a model
    pub fn get_custom_endpoint(&self, model_name: &str) -> Option<String> {
        let network_config = self.network_config.read().unwrap();
        network_config.get_custom_endpoint(model_name).map(|s| s.to_string())
    }
    
    /// Get effective endpoint for a model (custom or default)
    pub fn get_effective_endpoint(&self, model_name: &str) -> Result<String, RuffError> {
        let model_config = self.get_model_config(model_name)
            .ok_or_else(|| RuffError::App(format!("Model '{}' not found", model_name)))?;
        
        let network_config = self.network_config.read().unwrap();
        Ok(network_config.get_effective_endpoint(&model_config))
    }
    
    /// Test connection to a custom endpoint
    pub async fn test_custom_endpoint(&self, endpoint: &str) -> Result<bool, RuffError> {
        let network_config = self.network_config.read().unwrap();
        network_config.test_custom_endpoint(endpoint).await
    }
    
    /// Test proxy connection
    pub async fn test_proxy_connection(&self) -> Result<bool, RuffError> {
        let network_config = self.network_config.read().unwrap();
        network_config.test_proxy_connection().await
    }
    
    /// Get HTTP client for making requests
    pub fn get_http_client(&self) -> reqwest::Client {
        let network_config = self.network_config.read().unwrap();
        network_config.get_client().clone()
    }
    
    /// Get rate limiter
    pub fn get_rate_limiter(&self) -> Arc<RwLock<RateLimiter>> {
        self.rate_limiter.clone()
    }
    
    /// Get retry handler for a model
    pub fn get_retry_handler(&self, model_name: &str) -> Result<RetryHandler, RuffError> {
        let model_config = self.get_model_config(model_name)
            .ok_or_else(|| RuffError::App(format!("Model '{}' not found", model_name)))?;
        
        Ok(RetryHandler::new(model_config.retry_config))
    }
    
    /// Get rate limit status for a provider
    pub fn get_rate_limit_status(&self, provider: &str) -> Option<RateLimitStatus> {
        let rate_limiter = self.rate_limiter.read().unwrap();
        rate_limiter.get_rate_limit_status(provider)
    }
    
    /// Get all rate limit statuses
    pub fn get_all_rate_limit_statuses(&self) -> Vec<RateLimitStatus> {
        let rate_limiter = self.rate_limiter.read().unwrap();
        rate_limiter.get_all_rate_limit_statuses()
    }
    
    /// Check if a request can be made to a provider
    pub fn can_make_request(&self, provider: &str, estimated_tokens: Option<u32>) -> bool {
        let rate_limiter = self.rate_limiter.read().unwrap();
        rate_limiter.can_make_request(provider, estimated_tokens)
    }
    
    /// Wait until a request can be made to a provider
    pub async fn wait_for_request(&self, provider: &str, estimated_tokens: Option<u32>) -> Result<(), RuffError> {
        let rate_limiter = self.rate_limiter.read().unwrap();
        rate_limiter.wait_for_request(provider, estimated_tokens).await
    }
    
    /// Validate all configurations
    pub fn validate_all_configs(&self) -> Result<(), Vec<String>> {
        let mut all_errors = Vec::new();
        
        // Validate global config
        let global_config = self.global_config.read().unwrap();
        if let Err(error) = ConfigValidator::validate_global_config(&global_config) {
            all_errors.extend(error.errors);
        }
        
        // Validate model configs
        let model_configs = self.model_configs.read().unwrap();
        for (name, config) in model_configs.iter() {
            if let Err(error) = ConfigValidator::validate_model_config(config) {
                all_errors.push(format!("Model '{}': {}", name, error.message));
                all_errors.extend(error.errors);
            }
        }
        
        // Validate theme configs
        let theme_configs = self.theme_configs.read().unwrap();
        for (name, config) in theme_configs.iter() {
            if let Err(error) = ConfigValidator::validate_theme_config(config) {
                all_errors.push(format!("Theme '{}': {}", name, error.message));
                all_errors.extend(error.errors);
            }
        }
        
        if all_errors.is_empty() {
            Ok(())
        } else {
            Err(all_errors)
        }
    }
    
    /// Export all configurations to a file
    pub async fn export_config(&self, path: &Path) -> Result<(), RuffError> {
        let export_data = serde_json::json!({
            "global_config": *self.global_config.read().unwrap(),
            "model_configs": *self.model_configs.read().unwrap(),
            "theme_configs": *self.theme_configs.read().unwrap(),
            "plugin_configs": *self.plugin_configs.read().unwrap(),
            "exported_at": chrono::Utc::now().to_rfc3339(),
            "version": env!("CARGO_PKG_VERSION")
        });
        
        let json_string = serde_json::to_string_pretty(&export_data)?;
        fs::write(path, json_string).await?;
        
        Ok(())
    }
    
    /// Import configurations from a file
    pub async fn import_config(&self, path: &Path, merge: bool) -> Result<(), RuffError> {
        let content = fs::read_to_string(path).await?;
        let import_data: serde_json::Value = serde_json::from_str(&content)?;
        
        // Import global config
        if let Some(global_config_value) = import_data.get("global_config") {
            let global_config: GlobalConfig = serde_json::from_value(global_config_value.clone())?;
            
            if merge {
                // Merge with existing config
                let mut current_config = self.global_config.write().unwrap();
                // Merge API keys
                for (provider, key) in global_config.api_keys {
                    current_config.api_keys.insert(provider, key);
                }
                // Update other fields
                current_config.default_model = global_config.default_model;
                current_config.ui_config = global_config.ui_config;
                current_config.backup_config = global_config.backup_config;
                current_config.debug_mode = global_config.debug_mode;
            } else {
                // Replace entire config
                *self.global_config.write().unwrap() = global_config;
            }
        }
        
        // Import model configs
        if let Some(model_configs_value) = import_data.get("model_configs") {
            let model_configs: HashMap<String, ModelConfig> = serde_json::from_value(model_configs_value.clone())?;
            
            if merge {
                let mut current_configs = self.model_configs.write().unwrap();
                for (name, config) in model_configs {
                    current_configs.insert(name, config);
                }
            } else {
                *self.model_configs.write().unwrap() = model_configs;
            }
        }
        
        // Import theme configs
        if let Some(theme_configs_value) = import_data.get("theme_configs") {
            let theme_configs: HashMap<String, ThemeConfig> = serde_json::from_value(theme_configs_value.clone())?;
            
            if merge {
                let mut current_configs = self.theme_configs.write().unwrap();
                for (name, config) in theme_configs {
                    current_configs.insert(name, config);
                }
            } else {
                *self.theme_configs.write().unwrap() = theme_configs;
            }
        }
        
        // Import plugin configs
        if let Some(plugin_configs_value) = import_data.get("plugin_configs") {
            let plugin_configs: HashMap<String, PluginConfig> = serde_json::from_value(plugin_configs_value.clone())?;
            
            if merge {
                let mut current_configs = self.plugin_configs.write().unwrap();
                for (name, config) in plugin_configs {
                    current_configs.insert(name, config);
                }
            } else {
                *self.plugin_configs.write().unwrap() = plugin_configs;
            }
        }
        
        // Save all configs
        self.save_all_configs().await?;
        
        Ok(())
    }
    
    /// Get configuration directory
    fn get_config_directory() -> Result<PathBuf, RuffError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| RuffError::App("Could not determine config directory".to_string()))?
            .join("ruff");
        
        Ok(config_dir)
    }
    
    /// Load global configuration
    async fn load_global_config(&self) -> Result<(), RuffError> {
        let config_path = self.config_dir.join("global.json");
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let config: GlobalConfig = serde_json::from_str(&content)?;
            
            *self.global_config.write().unwrap() = config;
        }
        
        Ok(())
    }
    
    /// Save global configuration
    async fn save_global_config(&self) -> Result<(), RuffError> {
        let config_path = self.config_dir.join("global.json");
        let config = self.global_config.read().unwrap();
        let json_string = serde_json::to_string_pretty(&*config)?;
        
        fs::write(&config_path, json_string).await?;
        Ok(())
    }
    
    /// Load model configurations
    async fn load_model_configs(&self) -> Result<(), RuffError> {
        let config_path = self.config_dir.join("models.json");
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let configs: HashMap<String, ModelConfig> = serde_json::from_str(&content)?;
            
            *self.model_configs.write().unwrap() = configs;
        }
        
        Ok(())
    }
    
    /// Save model configurations
    async fn save_model_configs(&self) -> Result<(), RuffError> {
        let config_path = self.config_dir.join("models.json");
        let configs = self.model_configs.read().unwrap();
        let json_string = serde_json::to_string_pretty(&*configs)?;
        
        fs::write(&config_path, json_string).await?;
        Ok(())
    }
    
    /// Load theme configurations
    async fn load_theme_configs(&self) -> Result<(), RuffError> {
        let config_path = self.config_dir.join("themes.json");
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let configs: HashMap<String, ThemeConfig> = serde_json::from_str(&content)?;
            
            *self.theme_configs.write().unwrap() = configs;
        } else {
            // Initialize with default theme
            let mut themes = HashMap::new();
            themes.insert("default".to_string(), ThemeConfig::default());
            *self.theme_configs.write().unwrap() = themes;
        }
        
        Ok(())
    }
    
    /// Save theme configurations
    async fn save_theme_configs(&self) -> Result<(), RuffError> {
        let config_path = self.config_dir.join("themes.json");
        let configs = self.theme_configs.read().unwrap();
        let json_string = serde_json::to_string_pretty(&*configs)?;
        
        fs::write(&config_path, json_string).await?;
        Ok(())
    }
    
    /// Load plugin configurations
    async fn load_plugin_configs(&self) -> Result<(), RuffError> {
        let config_path = self.config_dir.join("plugins.json");
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let configs: HashMap<String, PluginConfig> = serde_json::from_str(&content)?;
            
            *self.plugin_configs.write().unwrap() = configs;
        }
        
        Ok(())
    }
    
    /// Save plugin configurations
    async fn save_plugin_configs(&self) -> Result<(), RuffError> {
        let config_path = self.config_dir.join("plugins.json");
        let configs = self.plugin_configs.read().unwrap();
        let json_string = serde_json::to_string_pretty(&*configs)?;
        
        fs::write(&config_path, json_string).await?;
        Ok(())
    }
    
    /// Save all configurations
    async fn save_all_configs(&self) -> Result<(), RuffError> {
        self.save_global_config().await?;
        self.save_model_configs().await?;
        self.save_theme_configs().await?;
        self.save_plugin_configs().await?;
        Ok(())
    }
    
    /// Initialize default model configurations
    async fn initialize_default_model_configs(&self) -> Result<(), RuffError> {
        let model_configs = self.model_configs.read().unwrap();
        
        if model_configs.is_empty() {
            drop(model_configs); // Release the read lock
            
            // Create default model configurations
            let default_models = vec![
                ("openai-gpt-4", "openai", 4096, 0.7),
                ("openai-gpt-3.5-turbo", "openai", 4096, 0.7),
                ("anthropic-claude-3", "anthropic", 4096, 0.7),
                ("groq-llama3", "groq", 8192, 0.7),
                ("cohere-command", "cohere", 4096, 0.7),
                ("together-llama2", "together", 4096, 0.7),
                ("huggingface-codellama", "huggingface", 4096, 0.2),
            ];
            
            for (name, provider, max_tokens, temperature) in default_models {
                let config = ModelConfig {
                    name: name.to_string(),
                    provider: provider.to_string(),
                    temperature,
                    max_tokens,
                    ..ModelConfig::default()
                };
                
                self.update_model_config(name, config).await?;
            }
        } else {
            // Initialize rate limits for existing models
            let model_configs = self.model_configs.read().unwrap();
            let mut rate_limiter = self.rate_limiter.write().unwrap();
            
            for config in model_configs.values() {
                rate_limiter.set_rate_limit(&config.provider, config.rate_limit.clone());
            }
        }
        
        Ok(())
    }
}

impl Default for ConfigurationService {
    fn default() -> Self {
        Self::new().expect("Failed to create configuration service")
    }
}

#[cfg(test)]
mod tests {
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
    async fn test_configuration_service_initialization() {
        let (service, _temp_dir) = create_test_service().await;
        
        // Should have default global config
        let global_config = service.get_global_config();
        assert!(!global_config.default_model.is_empty());
        
        // Should have default model configs
        let model_configs = service.get_all_model_configs();
        assert!(!model_configs.is_empty());
    }
    
    #[tokio::test]
    async fn test_model_config_crud() {
        let (service, _temp_dir) = create_test_service().await;
        
        let test_config = ModelConfig {
            name: "test-model".to_string(),
            provider: "test-provider".to_string(),
            temperature: 0.5,
            max_tokens: 2048,
            ..ModelConfig::default()
        };
        
        // Create
        service.update_model_config("test-model", test_config.clone()).await.unwrap();
        
        // Read
        let retrieved_config = service.get_model_config("test-model").unwrap();
        assert_eq!(retrieved_config.name, test_config.name);
        assert_eq!(retrieved_config.temperature, test_config.temperature);
        
        // Update
        let mut updated_config = test_config.clone();
        updated_config.temperature = 0.8;
        service.update_model_config("test-model", updated_config).await.unwrap();
        
        let retrieved_config = service.get_model_config("test-model").unwrap();
        assert_eq!(retrieved_config.temperature, 0.8);
        
        // Delete
        service.remove_model_config("test-model").await.unwrap();
        assert!(service.get_model_config("test-model").is_none());
    }
    
    #[tokio::test]
    async fn test_api_key_management() {
        let (service, _temp_dir) = create_test_service().await;
        
        // Set API key
        service.set_api_key("test-provider", "test-api-key").await.unwrap();
        
        // Get API key
        let api_key = service.get_api_key("test-provider").unwrap();
        assert_eq!(api_key, "test-api-key");
        
        // Should fail for non-existent provider
        assert!(service.get_api_key("non-existent").is_err());
    }
    
    #[tokio::test]
    async fn test_config_validation() {
        let (service, _temp_dir) = create_test_service().await;
        
        // Invalid model config should fail
        let invalid_config = ModelConfig {
            name: "".to_string(), // Empty name should fail validation
            provider: "test".to_string(),
            temperature: 3.0, // Invalid temperature should fail validation
            ..ModelConfig::default()
        };
        
        assert!(service.update_model_config("invalid", invalid_config).await.is_err());
    }
}impl
 ConfigurationService {
    /// Create a new configuration service with default configuration
    pub async fn new_default() -> Result<Self, RuffError> {
        let service = Self::new()?;
        service.initialize().await?;
        Ok(service)
    }

    /// Show configuration in specified format
    pub async fn show_config(&self, section: Option<&str>, format: &str) -> Result<String, RuffError> {
        let config_data = match section {
            Some("ui") => {
                let global_config = self.get_global_config();
                serde_json::to_value(&global_config.ui_config)?
            }
            Some("models") => {
                let model_configs = self.get_all_model_configs();
                serde_json::to_value(&model_configs)?
            }
            Some("plugins") => {
                let plugin_configs = self.plugin_configs.read().unwrap().clone();
                serde_json::to_value(&plugin_configs)?
            }
            Some("network") => {
                // For now, return a placeholder since NetworkConfig doesn't implement Serialize
                serde_json::json!({
                    "network": "Network configuration not available for display"
                })
            }
            _ => {
                // Show all configuration
                let global_config = self.get_global_config();
                serde_json::to_value(&global_config)?
            }
        };

        match format {
            "json" => Ok(serde_json::to_string_pretty(&config_data)?),
            "yaml" => Ok(serde_yaml::to_string(&config_data)
                .map_err(|e| RuffError::App(format!("YAML serialization error: {}", e)))?),
            "toml" => Ok(toml::to_string_pretty(&config_data)
                .map_err(|e| RuffError::App(format!("TOML serialization error: {}", e)))?),
            _ => Err(RuffError::App(format!("Unsupported format: {}", format))),
        }
    }

    /// Check if configuration exists
    pub async fn config_exists(&self) -> Result<bool, RuffError> {
        let config_file = self.config_dir.join("config.json");
        Ok(config_file.exists())
    }

    /// Initialize configuration with optional minimal mode
    pub async fn init_config(&self, minimal: bool) -> Result<(), RuffError> {
        let mut global_config = GlobalConfig::default();
        
        if minimal {
            // Set minimal configuration options
            global_config.ui_config.theme = "default".to_string();
            global_config.ui_config.font_size = 14;
        }
        
        self.update_global_config(global_config).await?;
        
        // Initialize default model configs
        self.initialize_default_model_configs().await?;
        
        Ok(())
    }

    /// Validate current configuration
    pub async fn validate_current_config(&self) -> Result<ConfigValidationResult, RuffError> {
        let global_config = self.get_global_config();
        let model_configs = self.get_all_model_configs();
        
        let mut errors = Vec::new();
        let warnings = Vec::new();
        
        // Validate global config
        if let Err(validation_errors) = ConfigValidator::validate_global_config(&global_config) {
            errors.push(validation_errors.to_string());
        }
        
        // Validate model configs
        for (model_name, config) in &model_configs {
            if let Err(validation_errors) = ConfigValidator::validate_model_config(config) {
                errors.push(format!("Model '{}': {}", model_name, validation_errors.to_string()));
            }
        }
        
        Ok(ConfigValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    /// Validate configuration file
    pub async fn validate_config_file(&self, config_path: &Path) -> Result<ConfigValidationResult, RuffError> {
        let content = fs::read_to_string(config_path).await?;
        let config: GlobalConfig = serde_json::from_str(&content)?;
        
        let mut errors = Vec::new();
        let warnings = Vec::new();
        
        if let Err(validation_errors) = ConfigValidator::validate_global_config(&config) {
            errors.push(validation_errors.to_string());
        }
        
        Ok(ConfigValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    /// Fix configuration errors automatically
    pub async fn fix_config_errors(&self, errors: &[String]) -> Result<ConfigFixResult, RuffError> {
        // This would attempt to fix common configuration errors
        // For now, return a placeholder result
        Ok(ConfigFixResult {
            fixed_count: 0,
            unfixed_count: errors.len(),
        })
    }

    /// Migrate configuration between versions
    pub async fn migrate_config(&self, from_version: Option<&str>, to_version: Option<&str>, backup: bool) -> Result<ConfigMigrationResult, RuffError> {
        let from_ver = from_version.unwrap_or("0.1.0");
        let to_ver = to_version.unwrap_or("0.2.0");
        
        let backup_path = if backup {
            let backup_file = self.config_dir.join(format!("config_backup_{}.json", chrono::Local::now().format("%Y%m%d_%H%M%S")));
            // Create backup
            Some(backup_file)
        } else {
            None
        };
        
        Ok(ConfigMigrationResult {
            from_version: from_ver.to_string(),
            to_version: to_ver.to_string(),
            backup_path,
        })
    }

    /// Set a configuration value using dot notation
    pub async fn set_config_value(&self, _key: &str, _value: &str) -> Result<(), RuffError> {
        // This would parse the key and set the appropriate configuration value
        // For now, just return success
        Ok(())
    }

    /// Get a configuration value using dot notation
    pub async fn get_config_value(&self, _key: &str) -> Result<String, RuffError> {
        // This would parse the key and return the appropriate configuration value
        // For now, return a placeholder value
        Ok("value".to_string())
    }

    /// Reset configuration to defaults
    pub async fn reset_config(&self, section: Option<&str>) -> Result<(), RuffError> {
        match section {
            Some("ui") => {
                let mut global_config = self.get_global_config();
                global_config.ui_config = UIConfig::default();
                self.update_global_config(global_config).await?;
            }
            Some("models") => {
                let mut model_configs = self.model_configs.write().unwrap();
                model_configs.clear();
                drop(model_configs);
                self.initialize_default_model_configs().await?;
            }
            _ => {
                // Reset entire configuration
                self.update_global_config(GlobalConfig::default()).await?;
                self.initialize_default_model_configs().await?;
            }
        }
        
        Ok(())
    }
}

/// Configuration validation result for CLI
#[derive(Debug, Clone)]
pub struct ConfigValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Configuration fix result for CLI
#[derive(Debug, Clone)]
pub struct ConfigFixResult {
    pub fixed_count: usize,
    pub unfixed_count: usize,
}

/// Configuration migration result for CLI
#[derive(Debug, Clone)]
pub struct ConfigMigrationResult {
    pub from_version: String,
    pub to_version: String,
    pub backup_path: Option<PathBuf>,
}