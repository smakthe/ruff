use crate::EnhancedError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parameter preset for different use cases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterPreset {
    pub name: String,
    pub description: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub use_case: UseCase,
}

/// Use cases for parameter presets
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UseCase {
    Creative,
    Analytical,
    Conversational,
    CodeGeneration,
    Summarization,
    Translation,
    Custom,
}

/// Parameter validation ranges
#[derive(Debug, Clone)]
pub struct ParameterRanges {
    pub temperature: (f32, f32),
    pub max_tokens: (u32, u32),
    pub top_p: (f32, f32),
    pub frequency_penalty: (f32, f32),
    pub presence_penalty: (f32, f32),
}

/// Parameter manager for real-time parameter adjustment
pub struct ParameterManager {
    presets: HashMap<String, ParameterPreset>,
    ranges: ParameterRanges,
}

impl ParameterManager {
    /// Create a new parameter manager
    pub fn new() -> Self {
        let mut manager = Self {
            presets: HashMap::new(),
            ranges: ParameterRanges {
                temperature: (0.0, 2.0),
                max_tokens: (1, 32768),
                top_p: (0.0, 1.0),
                frequency_penalty: (-2.0, 2.0),
                presence_penalty: (-2.0, 2.0),
            },
        };

        manager.load_builtin_presets();
        manager
    }

    /// Get all parameter presets
    pub fn get_presets(&self) -> Vec<&ParameterPreset> {
        self.presets.values().collect()
    }

    /// Get presets by use case
    pub fn get_presets_by_use_case(&self, use_case: &UseCase) -> Vec<&ParameterPreset> {
        self.presets
            .values()
            .filter(|preset| &preset.use_case == use_case)
            .collect()
    }

    /// Get a specific preset
    pub fn get_preset(&self, name: &str) -> Option<&ParameterPreset> {
        self.presets.get(name)
    }

    /// Add a custom preset
    pub fn add_preset(&mut self, preset: ParameterPreset) -> Result<(), EnhancedError> {
        // Validate the preset parameters
        self.validate_parameters(
            preset.temperature,
            preset.max_tokens,
            preset.top_p,
            preset.frequency_penalty,
            preset.presence_penalty,
        )?;

        self.presets.insert(preset.name.clone(), preset);
        Ok(())
    }

    /// Remove a preset
    pub fn remove_preset(&mut self, name: &str) -> Result<(), EnhancedError> {
        if self.presets.remove(name).is_none() {
            return Err(EnhancedError::unknown(format!(
                "Preset '{}' not found",
                name
            )));
        }
        Ok(())
    }

    /// Validate parameter values
    pub fn validate_parameters(
        &self,
        temperature: f32,
        max_tokens: u32,
        top_p: Option<f32>,
        frequency_penalty: Option<f32>,
        presence_penalty: Option<f32>,
    ) -> Result<(), EnhancedError> {
        let mut errors = Vec::new();

        // Validate temperature
        if temperature < self.ranges.temperature.0 || temperature > self.ranges.temperature.1 {
            errors.push(format!(
                "Temperature must be between {} and {}",
                self.ranges.temperature.0, self.ranges.temperature.1
            ));
        }

        // Validate max_tokens
        if max_tokens < self.ranges.max_tokens.0 || max_tokens > self.ranges.max_tokens.1 {
            errors.push(format!(
                "Max tokens must be between {} and {}",
                self.ranges.max_tokens.0, self.ranges.max_tokens.1
            ));
        }

        // Validate top_p
        if let Some(top_p) = top_p {
            if top_p < self.ranges.top_p.0 || top_p > self.ranges.top_p.1 {
                errors.push(format!(
                    "Top-p must be between {} and {}",
                    self.ranges.top_p.0, self.ranges.top_p.1
                ));
            }
        }

        // Validate frequency_penalty
        if let Some(freq_penalty) = frequency_penalty {
            if freq_penalty < self.ranges.frequency_penalty.0
                || freq_penalty > self.ranges.frequency_penalty.1
            {
                errors.push(format!(
                    "Frequency penalty must be between {} and {}",
                    self.ranges.frequency_penalty.0, self.ranges.frequency_penalty.1
                ));
            }
        }

        // Validate presence_penalty
        if let Some(pres_penalty) = presence_penalty {
            if pres_penalty < self.ranges.presence_penalty.0
                || pres_penalty > self.ranges.presence_penalty.1
            {
                errors.push(format!(
                    "Presence penalty must be between {} and {}",
                    self.ranges.presence_penalty.0, self.ranges.presence_penalty.1
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(EnhancedError::unknown(errors.join("; ")))
        }
    }

    /// Get parameter ranges
    pub fn get_ranges(&self) -> &ParameterRanges {
        &self.ranges
    }

    /// Suggest parameters based on use case
    pub fn suggest_parameters(&self, use_case: &UseCase) -> Option<ParameterPreset> {
        self.get_presets_by_use_case(use_case)
            .first()
            .map(|preset| (*preset).clone())
    }

    /// Get parameter description
    pub fn get_parameter_description(&self, parameter: &str) -> &'static str {
        match parameter {
            "temperature" => "Controls randomness: 0.0 = deterministic, 2.0 = very creative",
            "max_tokens" => "Maximum number of tokens to generate in the response",
            "top_p" => {
                "Nucleus sampling: considers tokens with cumulative probability up to this value"
            }
            "frequency_penalty" => {
                "Reduces repetition of tokens based on their frequency in the text"
            }
            "presence_penalty" => {
                "Reduces repetition of tokens based on whether they appear in the text"
            }
            _ => "Unknown parameter",
        }
    }

    /// Load built-in parameter presets
    fn load_builtin_presets(&mut self) {
        let presets = vec![
            ParameterPreset {
                name: "Creative Writing".to_string(),
                description: "High creativity for storytelling and creative content".to_string(),
                temperature: 0.9,
                max_tokens: 2048,
                top_p: Some(0.95),
                frequency_penalty: Some(0.3),
                presence_penalty: Some(0.3),
                use_case: UseCase::Creative,
            },
            ParameterPreset {
                name: "Analytical".to_string(),
                description: "Low temperature for factual and analytical responses".to_string(),
                temperature: 0.2,
                max_tokens: 1500,
                top_p: Some(0.8),
                frequency_penalty: Some(0.0),
                presence_penalty: Some(0.0),
                use_case: UseCase::Analytical,
            },
            ParameterPreset {
                name: "Conversational".to_string(),
                description: "Balanced settings for natural conversation".to_string(),
                temperature: 0.7,
                max_tokens: 1000,
                top_p: Some(0.9),
                frequency_penalty: Some(0.1),
                presence_penalty: Some(0.1),
                use_case: UseCase::Conversational,
            },
            ParameterPreset {
                name: "Code Generation".to_string(),
                description: "Optimized for generating accurate code".to_string(),
                temperature: 0.1,
                max_tokens: 2048,
                top_p: Some(0.95),
                frequency_penalty: Some(0.0),
                presence_penalty: Some(0.0),
                use_case: UseCase::CodeGeneration,
            },
            ParameterPreset {
                name: "Summarization".to_string(),
                description: "Focused settings for summarizing content".to_string(),
                temperature: 0.3,
                max_tokens: 500,
                top_p: Some(0.8),
                frequency_penalty: Some(0.2),
                presence_penalty: Some(0.0),
                use_case: UseCase::Summarization,
            },
            ParameterPreset {
                name: "Translation".to_string(),
                description: "Precise settings for language translation".to_string(),
                temperature: 0.1,
                max_tokens: 1000,
                top_p: Some(0.9),
                frequency_penalty: Some(0.0),
                presence_penalty: Some(0.0),
                use_case: UseCase::Translation,
            },
            ParameterPreset {
                name: "Brainstorming".to_string(),
                description: "High creativity for idea generation".to_string(),
                temperature: 1.2,
                max_tokens: 1500,
                top_p: Some(0.95),
                frequency_penalty: Some(0.5),
                presence_penalty: Some(0.5),
                use_case: UseCase::Creative,
            },
            ParameterPreset {
                name: "Technical Documentation".to_string(),
                description: "Precise and structured for technical writing".to_string(),
                temperature: 0.3,
                max_tokens: 2048,
                top_p: Some(0.85),
                frequency_penalty: Some(0.1),
                presence_penalty: Some(0.0),
                use_case: UseCase::Analytical,
            },
        ];

        for preset in presets {
            self.presets.insert(preset.name.clone(), preset);
        }
    }
}

impl Default for ParameterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for UseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UseCase::Creative => write!(f, "Creative"),
            UseCase::Analytical => write!(f, "Analytical"),
            UseCase::Conversational => write!(f, "Conversational"),
            UseCase::CodeGeneration => write!(f, "Code Generation"),
            UseCase::Summarization => write!(f, "Summarization"),
            UseCase::Translation => write!(f, "Translation"),
            UseCase::Custom => write!(f, "Custom"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_manager_creation() {
        let manager = ParameterManager::new();
        assert!(!manager.presets.is_empty());
    }

    #[test]
    fn test_get_presets_by_use_case() {
        let manager = ParameterManager::new();

        let creative_presets = manager.get_presets_by_use_case(&UseCase::Creative);
        assert!(!creative_presets.is_empty());

        for preset in creative_presets {
            assert_eq!(preset.use_case, UseCase::Creative);
        }
    }

    #[test]
    fn test_validate_parameters() {
        let manager = ParameterManager::new();

        // Valid parameters
        assert!(manager
            .validate_parameters(0.7, 1000, Some(0.9), Some(0.1), Some(0.1))
            .is_ok());

        // Invalid temperature
        assert!(manager
            .validate_parameters(3.0, 1000, Some(0.9), Some(0.1), Some(0.1))
            .is_err());

        // Invalid max_tokens
        assert!(manager
            .validate_parameters(0.7, 0, Some(0.9), Some(0.1), Some(0.1))
            .is_err());

        // Invalid top_p
        assert!(manager
            .validate_parameters(0.7, 1000, Some(1.5), Some(0.1), Some(0.1))
            .is_err());
    }

    #[test]
    fn test_add_custom_preset() {
        let mut manager = ParameterManager::new();

        let custom_preset = ParameterPreset {
            name: "Custom Test".to_string(),
            description: "A custom test preset".to_string(),
            temperature: 0.5,
            max_tokens: 1200,
            top_p: Some(0.85),
            frequency_penalty: Some(0.2),
            presence_penalty: Some(0.1),
            use_case: UseCase::Custom,
        };

        assert!(manager.add_preset(custom_preset.clone()).is_ok());

        let retrieved = manager.get_preset("Custom Test").unwrap();
        assert_eq!(retrieved.name, "Custom Test");
        assert_eq!(retrieved.temperature, 0.5);
    }

    #[test]
    fn test_suggest_parameters() {
        let manager = ParameterManager::new();

        let suggestion = manager.suggest_parameters(&UseCase::CodeGeneration);
        assert!(suggestion.is_some());

        let preset = suggestion.unwrap();
        assert_eq!(preset.use_case, UseCase::CodeGeneration);
        assert!(preset.temperature <= 0.2); // Should be low for code generation
    }

    #[test]
    fn test_parameter_descriptions() {
        let manager = ParameterManager::new();

        let temp_desc = manager.get_parameter_description("temperature");
        assert!(temp_desc.contains("randomness"));

        let tokens_desc = manager.get_parameter_description("max_tokens");
        assert!(tokens_desc.contains("Maximum"));

        let unknown_desc = manager.get_parameter_description("unknown");
        assert_eq!(unknown_desc, "Unknown parameter");
    }
}
