use super::models::*;
use crate::EnhancedError;
use std::collections::HashMap;
use url::Url;

/// Configuration validation result
pub type ValidationResult<T> = Result<T, ConfigValidationError>;

/// Configuration validation errors
#[derive(Debug, Clone)]
pub struct ConfigValidationError {
    pub field: String,
    pub message: String,
    pub errors: Vec<String>,
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Configuration validation failed for '{}': {}",
            self.field, self.message
        )?;
        if !self.errors.is_empty() {
            write!(f, "\nErrors:\n")?;
            for error in &self.errors {
                write!(f, "  - {}\n", error)?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ConfigValidationError {}

impl From<ConfigValidationError> for EnhancedError {
    fn from(error: ConfigValidationError) -> Self {
        EnhancedError::config(error.to_string())
    }
}

/// Configuration validator
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate global configuration
    pub fn validate_global_config(config: &GlobalConfig) -> ValidationResult<()> {
        let mut errors = Vec::new();

        // Validate version format
        if config.version.is_empty() {
            errors.push("Version cannot be empty".to_string());
        }

        // Validate default model
        if config.default_model.is_empty() {
            errors.push("Default model cannot be empty".to_string());
        }

        // Validate API keys
        if let Err(api_errors) = Self::validate_api_keys(&config.api_keys) {
            errors.extend(api_errors);
        }

        // Validate UI config
        if let Err(ui_errors) = Self::validate_ui_config(&config.ui_config) {
            errors.extend(ui_errors);
        }

        // Validate backup config
        if let Err(backup_errors) = Self::validate_backup_config(&config.backup_config) {
            errors.extend(backup_errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigValidationError {
                field: "global_config".to_string(),
                message: "Global configuration validation failed".to_string(),
                errors,
            })
        }
    }

    /// Validate model configuration
    pub fn validate_model_config(config: &ModelConfig) -> ValidationResult<()> {
        match config.validate() {
            Ok(()) => Ok(()),
            Err(errors) => Err(ConfigValidationError {
                field: format!("model_config.{}", config.name),
                message: "Model configuration validation failed".to_string(),
                errors,
            }),
        }
    }

    /// Validate API keys
    fn validate_api_keys(api_keys: &HashMap<String, String>) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Note: Empty API keys map is OK - keys can be set via keyring
        // We'll only validate keys that are present in the config

        for (provider, key) in api_keys {
            if provider.is_empty() {
                errors.push("Provider name cannot be empty".to_string());
                continue;
            }

            // Skip validation for empty keys - they can be set later via keyring
            if key.is_empty() {
                continue;
            }

            // Check for placeholder keys
            if key.contains("your-") || key.contains("sk-your") {
                errors.push(format!(
                    "API key for '{}' appears to be a placeholder. Remove it or set a real key.",
                    provider
                ));
                continue;
            }

            // Validate key format based on provider (only if key is not empty)
            match provider.as_str() {
                "openai" => {
                    if !key.starts_with("sk-") {
                        errors.push(format!("OpenAI API key should start with 'sk-'"));
                    }
                }
                "anthropic" => {
                    if !key.starts_with("sk-ant-") {
                        errors.push(format!("Anthropic API key should start with 'sk-ant-'"));
                    }
                }
                "groq" => {
                    if !key.starts_with("gsk_") {
                        errors.push(format!("Groq API key should start with 'gsk_'"));
                    }
                }
                "huggingface" => {
                    if !key.starts_with("hf_") {
                        errors.push(format!("Hugging Face API key should start with 'hf_'"));
                    }
                }
                _ => {} // Other providers don't have strict format requirements
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate UI configuration
    fn validate_ui_config(config: &UIConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if config.theme.is_empty() {
            errors.push("Theme name cannot be empty".to_string());
        }

        if config.font_size < 8 || config.font_size > 72 {
            errors.push("Font size must be between 8 and 72".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate backup configuration
    fn validate_backup_config(config: &BackupConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if config.interval_hours == 0 {
            errors.push("Backup interval must be greater than 0 hours".to_string());
        }

        if config.max_backups == 0 {
            errors.push("Max backups must be greater than 0".to_string());
        }

        if let Some(ref path) = config.backup_path {
            if path.is_empty() {
                errors.push("Backup path cannot be empty if specified".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate proxy configuration
    pub fn validate_proxy_config(config: &ProxyConfig) -> ValidationResult<()> {
        let mut errors = Vec::new();

        // Validate proxy URL
        match Url::parse(&config.url) {
            Ok(url) => {
                if !matches!(url.scheme(), "http" | "https" | "socks5") {
                    errors.push("Proxy URL must use http, https, or socks5 scheme".to_string());
                }
            }
            Err(_) => {
                errors.push("Invalid proxy URL format".to_string());
            }
        }

        // Validate authentication
        if config.username.is_some() && config.password.is_none() {
            errors.push("Password is required when username is provided".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigValidationError {
                field: "proxy_config".to_string(),
                message: "Proxy configuration validation failed".to_string(),
                errors,
            })
        }
    }

    /// Validate theme configuration
    pub fn validate_theme_config(config: &ThemeConfig) -> ValidationResult<()> {
        let mut errors = Vec::new();

        if config.name.is_empty() {
            errors.push("Theme name cannot be empty".to_string());
        }

        // Validate color formats (hex colors)
        let colors = [
            ("primary_color", &config.primary_color),
            ("secondary_color", &config.secondary_color),
            ("accent_color", &config.accent_color),
            ("error_color", &config.error_color),
            ("success_color", &config.success_color),
            ("background_color", &config.background_color),
            ("text_color", &config.text_color),
        ];

        for (name, color) in colors {
            if !Self::is_valid_hex_color(color) {
                errors.push(format!(
                    "{} must be a valid hex color (e.g., #FF0000)",
                    name
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigValidationError {
                field: "theme_config".to_string(),
                message: "Theme configuration validation failed".to_string(),
                errors,
            })
        }
    }

    /// Validate custom endpoint URL
    pub fn validate_custom_endpoint(endpoint: &str) -> ValidationResult<()> {
        match Url::parse(endpoint) {
            Ok(url) => {
                if !matches!(url.scheme(), "http" | "https") {
                    Err(ConfigValidationError {
                        field: "custom_endpoint".to_string(),
                        message: "Custom endpoint must use http or https scheme".to_string(),
                        errors: vec![],
                    })
                } else {
                    Ok(())
                }
            }
            Err(_) => Err(ConfigValidationError {
                field: "custom_endpoint".to_string(),
                message: "Invalid custom endpoint URL format".to_string(),
                errors: vec![],
            }),
        }
    }

    /// Check if a string is a valid hex color
    fn is_valid_hex_color(color: &str) -> bool {
        if !color.starts_with('#') || color.len() != 7 {
            return false;
        }

        color[1..].chars().all(|c| c.is_ascii_hexdigit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_global_config_default() {
        let config = GlobalConfig::default();
        assert!(ConfigValidator::validate_global_config(&config).is_ok());
    }

    #[test]
    fn test_validate_model_config_default() {
        let config = ModelConfig::default();
        assert!(ConfigValidator::validate_model_config(&config).is_ok());
    }

    #[test]
    fn test_validate_hex_color() {
        assert!(ConfigValidator::is_valid_hex_color("#FF0000"));
        assert!(ConfigValidator::is_valid_hex_color("#123ABC"));
        assert!(!ConfigValidator::is_valid_hex_color("FF0000"));
        assert!(!ConfigValidator::is_valid_hex_color("#FF00"));
        assert!(!ConfigValidator::is_valid_hex_color("#GG0000"));
    }

    #[test]
    fn test_validate_custom_endpoint() {
        assert!(ConfigValidator::validate_custom_endpoint("https://api.example.com").is_ok());
        assert!(ConfigValidator::validate_custom_endpoint("http://localhost:8080").is_ok());
        assert!(ConfigValidator::validate_custom_endpoint("ftp://example.com").is_err());
        assert!(ConfigValidator::validate_custom_endpoint("not-a-url").is_err());
    }

    #[test]
    fn test_validate_proxy_config() {
        let config = ProxyConfig {
            url: "http://proxy.example.com:8080".to_string(),
            username: None,
            password: None,
            no_proxy: vec![],
        };
        assert!(ConfigValidator::validate_proxy_config(&config).is_ok());

        let invalid_config = ProxyConfig {
            url: "invalid-url".to_string(),
            username: None,
            password: None,
            no_proxy: vec![],
        };
        assert!(ConfigValidator::validate_proxy_config(&invalid_config).is_err());
    }
}
