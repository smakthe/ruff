// Legacy config module - kept for backward compatibility
// New enhanced configuration is in src/config/

type Result<T> = std::result::Result<T, EnhancedError>;
use crate::EnhancedError;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub api_keys: HashMap<String, String>,
    pub default_model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub theme: ThemeConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThemeConfig {
    pub primary_color: String,
    pub secondary_color: String,
    pub accent_color: String,
    pub error_color: String,
    pub success_color: String,
}

impl Default for Config {
    fn default() -> Self {
        let mut api_keys = HashMap::new();

        // Placeholder API keys for different models
        api_keys.insert(
            "openai".to_string(),
            "sk-your-openai-api-key-here".to_string(),
        );
        api_keys.insert(
            "anthropic".to_string(),
            "sk-ant-your-anthropic-api-key-here".to_string(),
        );
        api_keys.insert("cohere".to_string(), "your-cohere-api-key-here".to_string());
        api_keys.insert(
            "together".to_string(),
            "your-together-api-key-here".to_string(),
        );
        api_keys.insert("groq".to_string(), "gsk_your-groq-api-key-here".to_string());
        api_keys.insert(
            "huggingface".to_string(),
            "hf_your-huggingface-api-key-here".to_string(),
        );

        Self {
            api_keys,
            default_model: "groq-llama3".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            theme: ThemeConfig::default(),
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            primary_color: "#CE422B".to_string(),   // Rust orange
            secondary_color: "#8B4513".to_string(), // Rust brown
            accent_color: "#FF6347".to_string(),    // Rust red-orange
            error_color: "#DC143C".to_string(),     // Crimson
            success_color: "#228B22".to_string(),   // Forest green
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        use crate::security::KeyringManager;

        let config_path = confy::get_configuration_file_path("ruff", None)
            .map_err(|e| EnhancedError::config(format!("Failed to get config path: {}", e)))?;

        // Load basic config from file
        let mut config: Config = if config_path.exists() {
            confy::load("ruff", None).map_err(EnhancedError::from)?
        } else {
            Config::default()
        };

        // Try to load API keys from keyring
        let keyring = KeyringManager::new();
        let providers = [
            "openai",
            "anthropic",
            "groq",
            "cohere",
            "together",
            "huggingface",
        ];

        for provider in providers {
            if let Ok(key) = keyring.get_api_key(provider) {
                // Store in config's api_keys HashMap
                config.api_keys.insert(provider.to_string(), key);
            }
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        confy::store("ruff", None, self).map_err(EnhancedError::from)
    }

    pub fn init_config() -> Result<()> {
        let config = Config::default();
        config.save()?;

        println!("{}", "🦀 Ruff configuration initialized!".bright_red());
        println!();
        println!(
            "{}",
            "📝 Please edit your API keys in the config file:".bright_yellow()
        );

        if let Ok(config_path) = confy::get_configuration_file_path("ruff", None) {
            println!("   {}", config_path.display().to_string().bright_cyan());
        }

        println!();
        println!("{}", "🔑 Supported AI Models:".bright_green());
        println!("   {} - OpenAI GPT models", "openai".bright_red());
        println!("   {} - Anthropic Claude models", "anthropic".bright_red());
        println!("   {} - Cohere Command models", "cohere".bright_red());
        println!("   {} - Together AI models", "together".bright_red());
        println!("   {} - Groq models (fast inference)", "groq".bright_red());
        println!("   {} - Hugging Face models", "huggingface".bright_red());

        println!();
        println!("{}", "🚀 Run 'ruff' to start chatting!".bright_magenta());

        Ok(())
    }

    pub fn show_config() -> Result<()> {
        let config = Config::load()?;

        println!("{}", "🦀 Current Ruff Configuration".bright_red());
        println!();
        println!(
            "{}: {}",
            "Default Model".bright_yellow(),
            config.default_model.bright_cyan()
        );
        println!(
            "{}: {}",
            "Max Tokens".bright_yellow(),
            config.max_tokens.to_string().bright_cyan()
        );
        println!(
            "{}: {}",
            "Temperature".bright_yellow(),
            config.temperature.to_string().bright_cyan()
        );
        println!();
        println!("{}", "🔑 API Keys Status:".bright_green());

        for (provider, key) in &config.api_keys {
            let status = if key.contains("your-") || key.contains("sk-your") {
                "❌ Not configured".bright_red()
            } else {
                "✅ Configured".bright_green()
            };
            println!("   {}: {}", provider.bright_cyan(), status);
        }

        if let Ok(config_path) = confy::get_configuration_file_path("ruff", None) {
            println!();
            println!(
                "{}: {}",
                "Config File".bright_yellow(),
                config_path.display().to_string().bright_cyan()
            );
        }

        Ok(())
    }

    pub fn get_api_key(&self, provider: &str) -> Result<&str> {
        self.api_keys
            .get(provider)
            .filter(|key| !key.contains("your-") && !key.contains("sk-your"))
            .map(|s| s.as_str())
            .ok_or_else(|| EnhancedError::auth(format!("Invalid API key for model: {}", provider)))
    }

    /// Store an API key in the keyring (and optionally config file)
    pub fn set_api_key(&mut self, provider: &str, key: &str, use_keyring: bool) -> Result<()> {
        use crate::security::KeyringManager;

        if use_keyring {
            let keyring = KeyringManager::new();
            keyring.store_api_key(provider, key)?;
            // Update in-memory config but don't save to file
            self.api_keys.insert(provider.to_string(), key.to_string());
        } else {
            // Store in config file (legacy behavior)
            self.api_keys.insert(provider.to_string(), key.to_string());
            self.save()?;
        }
        Ok(())
    }
}
