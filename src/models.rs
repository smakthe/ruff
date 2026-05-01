use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModel {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub max_tokens: u32,
    pub input_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
    pub supports_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Clone)]
pub struct ModelRegistry {
    models: HashMap<String, AIModel>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut models = HashMap::new();

        // OpenAI Models
        models.insert(
            "openai-gpt4".to_string(),
            AIModel {
                id: "gpt-4".to_string(),
                name: "GPT-4".to_string(),
                provider: "openai".to_string(),
                max_tokens: 8192,
                input_cost_per_1k: 0.03,
                output_cost_per_1k: 0.06,
                supports_system: true,
            },
        );

        models.insert(
            "openai-gpt3.5".to_string(),
            AIModel {
                id: "gpt-3.5-turbo".to_string(),
                name: "GPT-3.5 Turbo".to_string(),
                provider: "openai".to_string(),
                max_tokens: 4096,
                input_cost_per_1k: 0.001,
                output_cost_per_1k: 0.002,
                supports_system: true,
            },
        );

        // Anthropic Models
        models.insert(
            "anthropic-claude3".to_string(),
            AIModel {
                id: "claude-3-sonnet-20240229".to_string(),
                name: "Claude 3 Sonnet".to_string(),
                provider: "anthropic".to_string(),
                max_tokens: 4096,
                input_cost_per_1k: 0.003,
                output_cost_per_1k: 0.015,
                supports_system: true,
            },
        );

        // Cohere Models
        models.insert(
            "cohere-command".to_string(),
            AIModel {
                id: "command".to_string(),
                name: "Command".to_string(),
                provider: "cohere".to_string(),
                max_tokens: 4096,
                input_cost_per_1k: 0.001,
                output_cost_per_1k: 0.002,
                supports_system: false,
            },
        );

        // Together AI Models
        models.insert(
            "together-llama2".to_string(),
            AIModel {
                id: "meta-llama/Llama-2-70b-chat-hf".to_string(),
                name: "Llama 2 70B Chat".to_string(),
                provider: "together".to_string(),
                max_tokens: 4096,
                input_cost_per_1k: 0.0009,
                output_cost_per_1k: 0.0009,
                supports_system: true,
            },
        );

        // Groq Models (Fast inference)
        models.insert(
            "groq-llama3".to_string(),
            AIModel {
                id: "llama3-70b-8192".to_string(),
                name: "Llama 3 70B".to_string(),
                provider: "groq".to_string(),
                max_tokens: 8192,
                input_cost_per_1k: 0.0005,
                output_cost_per_1k: 0.0008,
                supports_system: true,
            },
        );

        models.insert(
            "groq-mixtral".to_string(),
            AIModel {
                id: "mixtral-8x7b-32768".to_string(),
                name: "Mixtral 8x7B".to_string(),
                provider: "groq".to_string(),
                max_tokens: 32768,
                input_cost_per_1k: 0.0002,
                output_cost_per_1k: 0.0002,
                supports_system: true,
            },
        );

        // Hugging Face Models
        models.insert(
            "hf-mistral".to_string(),
            AIModel {
                id: "mistralai/Mistral-7B-Instruct-v0.1".to_string(),
                name: "Mistral 7B Instruct".to_string(),
                provider: "huggingface".to_string(),
                max_tokens: 4096,
                input_cost_per_1k: 0.0001,
                output_cost_per_1k: 0.0001,
                supports_system: false,
            },
        );

        Self { models }
    }

    pub fn get_model(&self, model_key: &str) -> Option<&AIModel> {
        self.models.get(model_key)
    }

    pub fn list_models(&self) -> Vec<(&String, &AIModel)> {
        self.models.iter().collect()
    }

    pub fn get_models_by_provider(&self, provider: &str) -> Vec<(&String, &AIModel)> {
        self.models
            .iter()
            .filter(|(_, model)| model.provider == provider)
            .collect()
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}
