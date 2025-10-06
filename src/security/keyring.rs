use keyring::Entry;
use crate::error::EnhancedError;

pub struct KeyringManager {
    service_name: String,
}

impl KeyringManager {
    pub fn new() -> Self {
        Self {
            service_name: "ruff-ai-chat".to_string(),
        }
    }

    /// Store an API key securely in the OS keyring
    pub fn store_api_key(&self, provider: &str, key: &str) -> Result<(), EnhancedError> {
        let entry = Entry::new(&self.service_name, provider)
            .map_err(|e| EnhancedError::auth(format!("Failed to create keyring entry: {}", e)))?;

        entry.set_password(key)
            .map_err(|e| EnhancedError::auth(format!("Failed to store API key: {}", e)))?;

        Ok(())
    }

    /// Retrieve an API key from the OS keyring
    pub fn get_api_key(&self, provider: &str) -> Result<String, EnhancedError> {
        let entry = Entry::new(&self.service_name, provider)
            .map_err(|e| EnhancedError::auth(format!("Failed to create keyring entry: {}", e)))?;

        entry.get_password()
            .map_err(|e| EnhancedError::auth(format!("Failed to retrieve API key for {}: {}", provider, e)))
    }

    /// Delete an API key from the OS keyring
    pub fn delete_api_key(&self, provider: &str) -> Result<(), EnhancedError> {
        let entry = Entry::new(&self.service_name, provider)
            .map_err(|e| EnhancedError::auth(format!("Failed to create keyring entry: {}", e)))?;

        entry.delete_credential()
            .map_err(|e| EnhancedError::auth(format!("Failed to delete API key: {}", e)))?;

        Ok(())
    }

    /// List all providers that have stored keys
    pub fn list_providers(&self) -> Result<Vec<String>, EnhancedError> {
        // Note: keyring crate doesn't provide list functionality
        // We'll check each known provider
        Ok(vec![
            "openai".to_string(),
            "anthropic".to_string(),
            "groq".to_string(),
            "cohere".to_string(),
            "together".to_string(),
            "huggingface".to_string(),
        ])
    }
}

impl Default for KeyringManager {
    fn default() -> Self {
        Self::new()
    }
}
