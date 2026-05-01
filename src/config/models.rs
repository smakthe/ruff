use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Global configuration for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub version: String,
    pub default_model: String,
    pub api_keys: HashMap<String, String>,
    pub ui_config: UIConfig,
    pub backup_config: BackupConfig,
    pub debug_mode: bool,
}

/// UI-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    pub theme: String,
    pub font_size: u16,
    pub auto_save: bool,
    pub show_token_usage: bool,
    pub enable_streaming: bool,
}

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: bool,
    pub interval_hours: u32,
    pub max_backups: u32,
    pub backup_path: Option<String>,
    pub compress: bool,
    pub encrypt: bool,
}

/// Model-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    pub provider: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub custom_endpoint: Option<String>,
    pub rate_limit: RateLimit,
    pub retry_config: RetryConfig,
    pub proxy_config: Option<ProxyConfig>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_minute: u32,
    pub tokens_per_minute: Option<u32>,
    pub concurrent_requests: u32,
}

/// Retry configuration with exponential backoff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f32,
    pub retry_on_rate_limit: bool,
    pub retry_on_network_error: bool,
}

/// Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub no_proxy: Vec<String>,
}

/// Theme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
    pub primary_color: String,
    pub secondary_color: String,
    pub accent_color: String,
    pub error_color: String,
    pub success_color: String,
    pub background_color: String,
    pub text_color: String,
    pub high_contrast: bool,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub name: String,
    pub enabled: bool,
    pub settings: HashMap<String, serde_json::Value>,
    pub permissions: Vec<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        // Start with empty API keys - users will add them via keyring or config
        // This allows initialization to succeed without validation errors
        let api_keys = HashMap::new();

        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            default_model: "groq-llama3".to_string(),
            api_keys,
            ui_config: UIConfig::default(),
            backup_config: BackupConfig::default(),
            debug_mode: false,
        }
    }
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            font_size: 14,
            auto_save: true,
            show_token_usage: true,
            enable_streaming: true,
        }
    }
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: 24,
            max_backups: 7,
            backup_path: None,
            compress: true,
            encrypt: false,
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            provider: "openai".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            custom_endpoint: None,
            rate_limit: RateLimit::default(),
            retry_config: RetryConfig::default(),
            proxy_config: None,
        }
    }
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            tokens_per_minute: Some(100_000),
            concurrent_requests: 5,
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            retry_on_rate_limit: true,
            retry_on_network_error: true,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            primary_color: "#CE422B".to_string(), // Rust orange
            secondary_color: "#8B4513".to_string(), // Rust brown
            accent_color: "#FF6347".to_string(),  // Rust red-orange
            error_color: "#DC143C".to_string(),   // Crimson
            success_color: "#228B22".to_string(), // Forest green
            background_color: "#000000".to_string(), // Black
            text_color: "#FFFFFF".to_string(),    // White
            high_contrast: false,
        }
    }
}

impl RetryConfig {
    pub fn base_delay(&self) -> Duration {
        Duration::from_millis(self.base_delay_ms)
    }

    pub fn max_delay(&self) -> Duration {
        Duration::from_millis(self.max_delay_ms)
    }

    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms =
            (self.base_delay_ms as f32 * self.backoff_multiplier.powi(attempt as i32)) as u64;
        Duration::from_millis(delay_ms.min(self.max_delay_ms))
    }
}

impl ModelConfig {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.name.is_empty() {
            errors.push("Model name cannot be empty".to_string());
        }

        if self.provider.is_empty() {
            errors.push("Provider cannot be empty".to_string());
        }

        if !(0.0..=2.0).contains(&self.temperature) {
            errors.push("Temperature must be between 0.0 and 2.0".to_string());
        }

        if self.max_tokens == 0 || self.max_tokens > 1_000_000 {
            errors.push("Max tokens must be between 1 and 1,000,000".to_string());
        }

        if let Some(top_p) = self.top_p {
            if !(0.0..=1.0).contains(&top_p) {
                errors.push("Top-p must be between 0.0 and 1.0".to_string());
            }
        }

        if let Some(freq_penalty) = self.frequency_penalty {
            if !(-2.0..=2.0).contains(&freq_penalty) {
                errors.push("Frequency penalty must be between -2.0 and 2.0".to_string());
            }
        }

        if let Some(pres_penalty) = self.presence_penalty {
            if !(-2.0..=2.0).contains(&pres_penalty) {
                errors.push("Presence penalty must be between -2.0 and 2.0".to_string());
            }
        }

        if self.rate_limit.requests_per_minute == 0 {
            errors.push("Requests per minute must be greater than 0".to_string());
        }

        if self.rate_limit.concurrent_requests == 0 {
            errors.push("Concurrent requests must be greater than 0".to_string());
        }

        if self.retry_config.max_attempts == 0 {
            errors.push("Max retry attempts must be greater than 0".to_string());
        }

        if self.retry_config.backoff_multiplier <= 0.0 {
            errors.push("Backoff multiplier must be greater than 0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
