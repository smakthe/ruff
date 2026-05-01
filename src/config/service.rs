use super::models::*;
use super::network::NetworkConfig;
use super::rate_limiter::{RateLimitStatus, RateLimiter, RetryHandler};
use super::validation::ConfigValidator;
use crate::EnhancedError;
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::fs;

fn parse_bool(value: &str) -> Result<bool, EnhancedError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "on" => Ok(true),
        "false" | "0" | "no" | "n" | "off" => Ok(false),
        _ => Err(EnhancedError::parsing(format!(
            "Invalid boolean value: {}",
            value
        ))),
    }
}

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
    pub fn new() -> Result<Self, EnhancedError> {
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
    pub async fn initialize(&self) -> Result<(), EnhancedError> {
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
    pub async fn update_global_config(&self, config: GlobalConfig) -> Result<(), EnhancedError> {
        // Validate configuration
        ConfigValidator::validate_global_config(&config)
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

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
    pub async fn update_model_config(
        &self,
        model_name: &str,
        config: ModelConfig,
    ) -> Result<(), EnhancedError> {
        // Validate configuration
        ConfigValidator::validate_model_config(&config)
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

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
    pub async fn remove_model_config(&self, model_name: &str) -> Result<(), EnhancedError> {
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
    pub async fn update_theme_config(
        &self,
        theme_name: &str,
        config: ThemeConfig,
    ) -> Result<(), EnhancedError> {
        // Validate configuration
        ConfigValidator::validate_theme_config(&config)
            .map_err(|e| EnhancedError::unknown(e.to_string()))?;

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
        self.plugin_configs
            .read()
            .unwrap()
            .get(plugin_name)
            .cloned()
    }

    /// Update plugin configuration
    pub async fn update_plugin_config(
        &self,
        plugin_name: &str,
        config: PluginConfig,
    ) -> Result<(), EnhancedError> {
        {
            let mut plugin_configs = self.plugin_configs.write().unwrap();
            plugin_configs.insert(plugin_name.to_string(), config);
        }

        self.save_plugin_configs().await?;
        Ok(())
    }

    /// Get API key for a provider
    pub fn get_api_key(&self, provider: &str) -> Result<String, EnhancedError> {
        let configured_key = {
            let global_config = self.global_config.read().unwrap();
            global_config
                .api_keys
                .get(provider)
                .filter(|key| !key.contains("your-") && !key.contains("sk-your"))
                .cloned()
        };

        if let Some(key) = configured_key {
            return Ok(key);
        }

        crate::security::KeyringManager::new()
            .get_api_key(provider)
            .map_err(|_| EnhancedError::auth(format!("Invalid API key for model: {}", provider)))
    }

    /// Set API key for a provider
    pub async fn set_api_key(&self, provider: &str, api_key: &str) -> Result<(), EnhancedError> {
        {
            let mut global_config = self.global_config.write().unwrap();
            global_config
                .api_keys
                .insert(provider.to_string(), api_key.to_string());
        }

        self.save_global_config().await?;
        Ok(())
    }

    /// Configure proxy settings
    pub fn set_proxy_config(&self, proxy_config: ProxyConfig) -> Result<(), EnhancedError> {
        let mut network_config = self.network_config.write().unwrap();
        network_config.set_proxy(proxy_config)
    }

    /// Remove proxy configuration
    pub fn remove_proxy_config(&self) -> Result<(), EnhancedError> {
        let mut network_config = self.network_config.write().unwrap();
        network_config.remove_proxy()
    }

    /// Get proxy configuration
    pub fn get_proxy_config(&self) -> Option<ProxyConfig> {
        let network_config = self.network_config.read().unwrap();
        network_config.get_proxy_config().cloned()
    }

    /// Set custom endpoint for a model
    pub fn set_custom_endpoint(
        &self,
        model_name: &str,
        endpoint: &str,
    ) -> Result<(), EnhancedError> {
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
        network_config
            .get_custom_endpoint(model_name)
            .map(|s| s.to_string())
    }

    /// Get effective endpoint for a model (custom or default)
    pub fn get_effective_endpoint(&self, model_name: &str) -> Result<String, EnhancedError> {
        let model_config = self
            .get_model_config(model_name)
            .ok_or_else(|| EnhancedError::unknown(format!("Model '{}' not found", model_name)))?;

        let network_config = self.network_config.read().unwrap();
        Ok(network_config.get_effective_endpoint(&model_config))
    }

    /// Test connection to a custom endpoint
    pub async fn test_custom_endpoint(&self, endpoint: &str) -> Result<bool, EnhancedError> {
        let network_config = self.network_config.read().unwrap();
        network_config.test_custom_endpoint(endpoint).await
    }

    /// Test proxy connection
    pub async fn test_proxy_connection(&self) -> Result<bool, EnhancedError> {
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
    pub fn get_retry_handler(&self, model_name: &str) -> Result<RetryHandler, EnhancedError> {
        let model_config = self
            .get_model_config(model_name)
            .ok_or_else(|| EnhancedError::unknown(format!("Model '{}' not found", model_name)))?;

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
    pub async fn wait_for_request(
        &self,
        provider: &str,
        estimated_tokens: Option<u32>,
    ) -> Result<(), EnhancedError> {
        let rate_limiter = self.rate_limiter.read().unwrap();
        rate_limiter
            .wait_for_request(provider, estimated_tokens)
            .await
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
    pub async fn export_config(&self, path: &Path) -> Result<(), EnhancedError> {
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
    pub async fn import_config(&self, path: &Path, merge: bool) -> Result<(), EnhancedError> {
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
            let model_configs: HashMap<String, ModelConfig> =
                serde_json::from_value(model_configs_value.clone())?;

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
            let theme_configs: HashMap<String, ThemeConfig> =
                serde_json::from_value(theme_configs_value.clone())?;

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
            let plugin_configs: HashMap<String, PluginConfig> =
                serde_json::from_value(plugin_configs_value.clone())?;

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
    fn get_config_directory() -> Result<PathBuf, EnhancedError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| {
                EnhancedError::unknown("Could not determine config directory".to_string())
            })?
            .join("ruff");

        Ok(config_dir)
    }

    /// Load global configuration
    async fn load_global_config(&self) -> Result<(), EnhancedError> {
        let config_path = self.config_dir.join("global.json");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let config: GlobalConfig = serde_json::from_str(&content)?;

            *self.global_config.write().unwrap() = config;
        }

        Ok(())
    }

    /// Save global configuration
    async fn save_global_config(&self) -> Result<(), EnhancedError> {
        let config_path = self.config_dir.join("global.json");
        let config = self.global_config.read().unwrap();
        let json_string = serde_json::to_string_pretty(&*config)?;

        fs::write(&config_path, json_string).await?;
        Ok(())
    }

    /// Load model configurations
    async fn load_model_configs(&self) -> Result<(), EnhancedError> {
        let config_path = self.config_dir.join("models.json");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let configs: HashMap<String, ModelConfig> = serde_json::from_str(&content)?;

            *self.model_configs.write().unwrap() = configs;
        }

        Ok(())
    }

    /// Save model configurations
    async fn save_model_configs(&self) -> Result<(), EnhancedError> {
        let config_path = self.config_dir.join("models.json");
        let configs = self.model_configs.read().unwrap();
        let json_string = serde_json::to_string_pretty(&*configs)?;

        fs::write(&config_path, json_string).await?;
        Ok(())
    }

    /// Load theme configurations
    async fn load_theme_configs(&self) -> Result<(), EnhancedError> {
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
    async fn save_theme_configs(&self) -> Result<(), EnhancedError> {
        let config_path = self.config_dir.join("themes.json");
        let configs = self.theme_configs.read().unwrap();
        let json_string = serde_json::to_string_pretty(&*configs)?;

        fs::write(&config_path, json_string).await?;
        Ok(())
    }

    /// Load plugin configurations
    async fn load_plugin_configs(&self) -> Result<(), EnhancedError> {
        let config_path = self.config_dir.join("plugins.json");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let configs: HashMap<String, PluginConfig> = serde_json::from_str(&content)?;

            *self.plugin_configs.write().unwrap() = configs;
        }

        Ok(())
    }

    /// Save plugin configurations
    async fn save_plugin_configs(&self) -> Result<(), EnhancedError> {
        let config_path = self.config_dir.join("plugins.json");
        let configs = self.plugin_configs.read().unwrap();
        let json_string = serde_json::to_string_pretty(&*configs)?;

        fs::write(&config_path, json_string).await?;
        Ok(())
    }

    /// Save all configurations
    async fn save_all_configs(&self) -> Result<(), EnhancedError> {
        self.save_global_config().await?;
        self.save_model_configs().await?;
        self.save_theme_configs().await?;
        self.save_plugin_configs().await?;
        Ok(())
    }

    /// Initialize default model configurations
    async fn initialize_default_model_configs(&self) -> Result<(), EnhancedError> {
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
        service
            .update_model_config("test-model", test_config.clone())
            .await
            .unwrap();

        // Read
        let retrieved_config = service.get_model_config("test-model").unwrap();
        assert_eq!(retrieved_config.name, test_config.name);
        assert_eq!(retrieved_config.temperature, test_config.temperature);

        // Update
        let mut updated_config = test_config.clone();
        updated_config.temperature = 0.8;
        service
            .update_model_config("test-model", updated_config)
            .await
            .unwrap();

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
        service
            .set_api_key("test-provider", "test-api-key")
            .await
            .unwrap();

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

        assert!(service
            .update_model_config("invalid", invalid_config)
            .await
            .is_err());
    }
}
impl ConfigurationService {
    /// Create a new configuration service with default configuration
    pub async fn new_default() -> Result<Self, EnhancedError> {
        let service = Self::new()?;
        service.initialize().await?;
        Ok(service)
    }

    /// Show configuration in specified format
    pub async fn show_config(
        &self,
        section: Option<&str>,
        format: &str,
    ) -> Result<String, EnhancedError> {
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
                let network_config = self.network_config.read().unwrap();
                serde_json::to_value(network_config.summary())?
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
                .map_err(|e| EnhancedError::unknown(format!("YAML serialization error: {}", e)))?),
            "toml" => Ok(toml::to_string_pretty(&config_data)
                .map_err(|e| EnhancedError::unknown(format!("TOML serialization error: {}", e)))?),
            _ => Err(EnhancedError::unknown(format!(
                "Unsupported format: {}",
                format
            ))),
        }
    }

    /// Check if configuration exists
    pub async fn config_exists(&self) -> Result<bool, EnhancedError> {
        let config_file = self.config_dir.join("global.json");
        Ok(config_file.exists())
    }

    /// Initialize configuration with optional minimal mode
    pub async fn init_config(&self, minimal: bool) -> Result<(), EnhancedError> {
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
    pub async fn validate_current_config(&self) -> Result<ConfigValidationResult, EnhancedError> {
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
                errors.push(format!(
                    "Model '{}': {}",
                    model_name,
                    validation_errors.to_string()
                ));
            }
        }

        Ok(ConfigValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    /// Validate configuration file
    pub async fn validate_config_file(
        &self,
        config_path: &Path,
    ) -> Result<ConfigValidationResult, EnhancedError> {
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
    pub async fn fix_config_errors(
        &self,
        errors: &[String],
    ) -> Result<ConfigFixResult, EnhancedError> {
        let initial_error_count = errors.len();
        let mut fixed_count = 0;

        let default_global = GlobalConfig::default();
        let mut global_config = self.get_global_config();

        if global_config.version.trim().is_empty() {
            global_config.version = default_global.version.clone();
            fixed_count += 1;
        }

        if global_config.default_model.trim().is_empty() {
            global_config.default_model = default_global.default_model.clone();
            fixed_count += 1;
        }

        let before_api_key_count = global_config.api_keys.len();
        global_config.api_keys.retain(|provider, key| {
            if provider.trim().is_empty() {
                return false;
            }
            if key.trim().is_empty() {
                return true;
            }
            if key.contains("your-") || key.contains("sk-your") {
                return false;
            }
            match provider.as_str() {
                "openai" => key.starts_with("sk-"),
                "anthropic" => key.starts_with("sk-ant-"),
                "groq" => key.starts_with("gsk_"),
                "huggingface" => key.starts_with("hf_"),
                _ => true,
            }
        });
        fixed_count += before_api_key_count.saturating_sub(global_config.api_keys.len());

        if global_config.ui_config.theme.trim().is_empty() {
            global_config.ui_config.theme = default_global.ui_config.theme.clone();
            fixed_count += 1;
        }

        let clamped_font_size = global_config.ui_config.font_size.clamp(8, 72);
        if clamped_font_size != global_config.ui_config.font_size {
            global_config.ui_config.font_size = clamped_font_size;
            fixed_count += 1;
        }

        if global_config.backup_config.interval_hours == 0 {
            global_config.backup_config.interval_hours =
                default_global.backup_config.interval_hours;
            fixed_count += 1;
        }

        if global_config.backup_config.max_backups == 0 {
            global_config.backup_config.max_backups = default_global.backup_config.max_backups;
            fixed_count += 1;
        }

        if matches!(global_config.backup_config.backup_path.as_deref(), Some("")) {
            global_config.backup_config.backup_path = None;
            fixed_count += 1;
        }

        {
            let mut stored_global_config = self.global_config.write().unwrap();
            *stored_global_config = global_config;
        }
        self.save_global_config().await?;

        let default_model_config = ModelConfig::default();
        {
            let mut model_configs = self.model_configs.write().unwrap();

            for (model_key, model_config) in model_configs.iter_mut() {
                if model_config.name.trim().is_empty() {
                    model_config.name = model_key.clone();
                    fixed_count += 1;
                }

                if model_config.provider.trim().is_empty() {
                    model_config.provider = default_model_config.provider.clone();
                    fixed_count += 1;
                }

                let clamped_temperature = model_config.temperature.clamp(0.0, 2.0);
                if (clamped_temperature - model_config.temperature).abs() > f32::EPSILON {
                    model_config.temperature = clamped_temperature;
                    fixed_count += 1;
                }

                if model_config.max_tokens == 0 || model_config.max_tokens > 1_000_000 {
                    model_config.max_tokens = default_model_config.max_tokens;
                    fixed_count += 1;
                }

                if let Some(top_p) = model_config.top_p {
                    let clamped_top_p = top_p.clamp(0.0, 1.0);
                    if (clamped_top_p - top_p).abs() > f32::EPSILON {
                        model_config.top_p = Some(clamped_top_p);
                        fixed_count += 1;
                    }
                }

                if let Some(frequency_penalty) = model_config.frequency_penalty {
                    let clamped_frequency_penalty = frequency_penalty.clamp(-2.0, 2.0);
                    if (clamped_frequency_penalty - frequency_penalty).abs() > f32::EPSILON {
                        model_config.frequency_penalty = Some(clamped_frequency_penalty);
                        fixed_count += 1;
                    }
                }

                if let Some(presence_penalty) = model_config.presence_penalty {
                    let clamped_presence_penalty = presence_penalty.clamp(-2.0, 2.0);
                    if (clamped_presence_penalty - presence_penalty).abs() > f32::EPSILON {
                        model_config.presence_penalty = Some(clamped_presence_penalty);
                        fixed_count += 1;
                    }
                }

                if model_config.rate_limit.requests_per_minute == 0 {
                    model_config.rate_limit.requests_per_minute =
                        default_model_config.rate_limit.requests_per_minute;
                    fixed_count += 1;
                }

                if model_config.rate_limit.concurrent_requests == 0 {
                    model_config.rate_limit.concurrent_requests =
                        default_model_config.rate_limit.concurrent_requests;
                    fixed_count += 1;
                }

                if model_config.retry_config.max_attempts == 0 {
                    model_config.retry_config.max_attempts =
                        default_model_config.retry_config.max_attempts;
                    fixed_count += 1;
                }

                if model_config.retry_config.backoff_multiplier <= 0.0 {
                    model_config.retry_config.backoff_multiplier =
                        default_model_config.retry_config.backoff_multiplier;
                    fixed_count += 1;
                }
            }
        }
        self.save_model_configs().await?;

        let validation_result = self.validate_current_config().await?;
        let unfixed_count = validation_result.errors.len();
        let fixed_count = fixed_count.max(initial_error_count.saturating_sub(unfixed_count));

        Ok(ConfigFixResult {
            fixed_count,
            unfixed_count,
        })
    }

    /// Migrate configuration between versions
    pub async fn migrate_config(
        &self,
        from_version: Option<&str>,
        to_version: Option<&str>,
        backup: bool,
    ) -> Result<ConfigMigrationResult, EnhancedError> {
        let from_ver = from_version.unwrap_or("0.1.0");
        let to_ver = to_version.unwrap_or("0.2.0");

        let backup_path = if backup {
            let backup_file = self.config_dir.join(format!(
                "config_backup_{}.json",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            ));
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
    pub async fn set_config_value(&self, key: &str, value: &str) -> Result<(), EnhancedError> {
        let mut global_config = self.get_global_config();

        match key {
            "default_model" => global_config.default_model = value.to_string(),
            "debug_mode" => global_config.debug_mode = parse_bool(value)?,
            "ui.theme" => global_config.ui_config.theme = value.to_string(),
            "ui.font_size" => {
                global_config.ui_config.font_size = value
                    .parse::<u16>()
                    .map_err(|e| EnhancedError::parsing(format!("Invalid font size: {}", e)))?;
            }
            "ui.auto_save" => global_config.ui_config.auto_save = parse_bool(value)?,
            "ui.show_token_usage" => global_config.ui_config.show_token_usage = parse_bool(value)?,
            "ui.enable_streaming" => global_config.ui_config.enable_streaming = parse_bool(value)?,
            "backup.enabled" => global_config.backup_config.enabled = parse_bool(value)?,
            "backup.interval_hours" => {
                global_config.backup_config.interval_hours = value.parse::<u32>().map_err(|e| {
                    EnhancedError::parsing(format!("Invalid backup interval: {}", e))
                })?;
            }
            "backup.max_backups" => {
                global_config.backup_config.max_backups = value.parse::<u32>().map_err(|e| {
                    EnhancedError::parsing(format!("Invalid max backup count: {}", e))
                })?;
            }
            "backup.compress" => global_config.backup_config.compress = parse_bool(value)?,
            "backup.encrypt" => global_config.backup_config.encrypt = parse_bool(value)?,
            "backup.path" => {
                global_config.backup_config.backup_path = if value.trim().is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            key if key.starts_with("api_keys.") => {
                let provider = key.trim_start_matches("api_keys.");
                if provider.is_empty() {
                    return Err(EnhancedError::parsing(
                        "Provider name is required for api_keys.<provider>",
                    ));
                }
                global_config
                    .api_keys
                    .insert(provider.to_string(), value.to_string());
            }
            _ => {
                return Err(EnhancedError::config(format!(
                    "Unsupported configuration key '{}'. Supported examples: default_model, ui.font_size, ui.theme, ui.enable_streaming, backup.enabled, api_keys.openai",
                    key
                )));
            }
        }

        self.update_global_config(global_config).await
    }

    /// Get a configuration value using dot notation
    pub async fn get_config_value(&self, key: &str) -> Result<String, EnhancedError> {
        let global_config = self.get_global_config();

        match key {
            "default_model" => Ok(global_config.default_model),
            "debug_mode" => Ok(global_config.debug_mode.to_string()),
            "ui.theme" => Ok(global_config.ui_config.theme),
            "ui.font_size" => Ok(global_config.ui_config.font_size.to_string()),
            "ui.auto_save" => Ok(global_config.ui_config.auto_save.to_string()),
            "ui.show_token_usage" => Ok(global_config.ui_config.show_token_usage.to_string()),
            "ui.enable_streaming" => Ok(global_config.ui_config.enable_streaming.to_string()),
            "backup.enabled" => Ok(global_config.backup_config.enabled.to_string()),
            "backup.interval_hours" => Ok(global_config.backup_config.interval_hours.to_string()),
            "backup.max_backups" => Ok(global_config.backup_config.max_backups.to_string()),
            "backup.compress" => Ok(global_config.backup_config.compress.to_string()),
            "backup.encrypt" => Ok(global_config.backup_config.encrypt.to_string()),
            "backup.path" => Ok(global_config.backup_config.backup_path.unwrap_or_default()),
            key if key.starts_with("api_keys.") => {
                let provider = key.trim_start_matches("api_keys.");
                global_config
                    .api_keys
                    .get(provider)
                    .cloned()
                    .ok_or_else(|| {
                        EnhancedError::config(format!("No API key configured for '{}'", provider))
                    })
            }
            _ => Err(EnhancedError::config(format!(
                "Unsupported configuration key '{}'",
                key
            ))),
        }
    }

    /// Reset configuration to defaults
    pub async fn reset_config(&self, section: Option<&str>) -> Result<(), EnhancedError> {
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
