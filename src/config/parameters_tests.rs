#[cfg(test)]
mod tests {

    use crate::config::{ParameterManager, ParameterPreset, UseCase};

    #[test]
    fn test_parameter_manager_creation() {
        let manager = ParameterManager::new();
        assert!(!manager.get_presets().is_empty());
        
        // Should have built-in presets
        assert!(manager.get_preset("Creative Writing").is_some());
        assert!(manager.get_preset("Analytical").is_some());
        assert!(manager.get_preset("Code Generation").is_some());
    }

    #[test]
    fn test_get_presets_by_use_case() {
        let manager = ParameterManager::new();
        
        let creative_presets = manager.get_presets_by_use_case(&UseCase::Creative);
        assert!(!creative_presets.is_empty());
        
        for preset in creative_presets {
            assert_eq!(preset.use_case, UseCase::Creative);
        }
        
        let analytical_presets = manager.get_presets_by_use_case(&UseCase::Analytical);
        assert!(!analytical_presets.is_empty());
        
        for preset in analytical_presets {
            assert_eq!(preset.use_case, UseCase::Analytical);
        }
    }

    #[test]
    fn test_validate_parameters_valid() {
        let manager = ParameterManager::new();
        
        // Valid parameters should pass
        assert!(manager.validate_parameters(0.7, 1000, Some(0.9), Some(0.1), Some(0.1)).is_ok());
        assert!(manager.validate_parameters(0.0, 1, None, None, None).is_ok());
        assert!(manager.validate_parameters(2.0, 32768, Some(1.0), Some(2.0), Some(2.0)).is_ok());
    }

    #[test]
    fn test_validate_parameters_invalid_temperature() {
        let manager = ParameterManager::new();
        
        // Temperature too low
        let result = manager.validate_parameters(-0.1, 1000, Some(0.9), Some(0.1), Some(0.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Temperature"));
        
        // Temperature too high
        let result = manager.validate_parameters(2.1, 1000, Some(0.9), Some(0.1), Some(0.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Temperature"));
    }

    #[test]
    fn test_validate_parameters_invalid_max_tokens() {
        let manager = ParameterManager::new();
        
        // Max tokens too low
        let result = manager.validate_parameters(0.7, 0, Some(0.9), Some(0.1), Some(0.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Max tokens"));
        
        // Max tokens too high
        let result = manager.validate_parameters(0.7, 32769, Some(0.9), Some(0.1), Some(0.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Max tokens"));
    }

    #[test]
    fn test_validate_parameters_invalid_top_p() {
        let manager = ParameterManager::new();
        
        // Top-p too low
        let result = manager.validate_parameters(0.7, 1000, Some(-0.1), Some(0.1), Some(0.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Top-p"));
        
        // Top-p too high
        let result = manager.validate_parameters(0.7, 1000, Some(1.1), Some(0.1), Some(0.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Top-p"));
    }

    #[test]
    fn test_validate_parameters_invalid_penalties() {
        let manager = ParameterManager::new();
        
        // Frequency penalty too low
        let result = manager.validate_parameters(0.7, 1000, Some(0.9), Some(-2.1), Some(0.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Frequency penalty"));
        
        // Frequency penalty too high
        let result = manager.validate_parameters(0.7, 1000, Some(0.9), Some(2.1), Some(0.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Frequency penalty"));
        
        // Presence penalty too low
        let result = manager.validate_parameters(0.7, 1000, Some(0.9), Some(0.1), Some(-2.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Presence penalty"));
        
        // Presence penalty too high
        let result = manager.validate_parameters(0.7, 1000, Some(0.9), Some(0.1), Some(2.1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Presence penalty"));
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
        assert_eq!(retrieved.max_tokens, 1200);
        assert_eq!(retrieved.use_case, UseCase::Custom);
    }

    #[test]
    fn test_add_invalid_preset() {
        let mut manager = ParameterManager::new();
        
        let invalid_preset = ParameterPreset {
            name: "Invalid Test".to_string(),
            description: "An invalid test preset".to_string(),
            temperature: 3.0, // Invalid temperature
            max_tokens: 1200,
            top_p: Some(0.85),
            frequency_penalty: Some(0.2),
            presence_penalty: Some(0.1),
            use_case: UseCase::Custom,
        };
        
        let result = manager.add_preset(invalid_preset);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Temperature"));
    }

    #[test]
    fn test_remove_preset() {
        let mut manager = ParameterManager::new();
        
        let custom_preset = ParameterPreset {
            name: "To Remove".to_string(),
            description: "Will be removed".to_string(),
            temperature: 0.5,
            max_tokens: 1200,
            top_p: Some(0.85),
            frequency_penalty: Some(0.2),
            presence_penalty: Some(0.1),
            use_case: UseCase::Custom,
        };
        
        manager.add_preset(custom_preset).unwrap();
        assert!(manager.get_preset("To Remove").is_some());
        
        manager.remove_preset("To Remove").unwrap();
        assert!(manager.get_preset("To Remove").is_none());
    }

    #[test]
    fn test_remove_nonexistent_preset() {
        let mut manager = ParameterManager::new();
        
        let result = manager.remove_preset("Nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_suggest_parameters() {
        let manager = ParameterManager::new();
        
        let creative_suggestion = manager.suggest_parameters(&UseCase::Creative);
        assert!(creative_suggestion.is_some());
        
        let preset = creative_suggestion.unwrap();
        assert_eq!(preset.use_case, UseCase::Creative);
        assert!(preset.temperature > 0.5); // Should be high for creativity
        
        let analytical_suggestion = manager.suggest_parameters(&UseCase::Analytical);
        assert!(analytical_suggestion.is_some());
        
        let analytical_preset = analytical_suggestion.unwrap();
        assert_eq!(analytical_preset.use_case, UseCase::Analytical);
        assert!(analytical_preset.temperature < 0.5); // Should be low for analysis
        
        let code_suggestion = manager.suggest_parameters(&UseCase::CodeGeneration);
        assert!(code_suggestion.is_some());
        
        let code_preset = code_suggestion.unwrap();
        assert_eq!(code_preset.use_case, UseCase::CodeGeneration);
        assert!(code_preset.temperature <= 0.2); // Should be very low for code generation
    }

    #[test]
    fn test_parameter_descriptions() {
        let manager = ParameterManager::new();
        
        let temp_desc = manager.get_parameter_description("temperature");
        assert!(temp_desc.contains("randomness"));
        assert!(temp_desc.contains("deterministic"));
        assert!(temp_desc.contains("creative"));
        
        let tokens_desc = manager.get_parameter_description("max_tokens");
        assert!(tokens_desc.contains("Maximum"));
        assert!(tokens_desc.contains("tokens"));
        
        let top_p_desc = manager.get_parameter_description("top_p");
        assert!(top_p_desc.contains("Nucleus"));
        assert!(top_p_desc.contains("sampling"));
        
        let freq_desc = manager.get_parameter_description("frequency_penalty");
        assert!(freq_desc.contains("repetition"));
        assert!(freq_desc.contains("frequency"));
        
        let pres_desc = manager.get_parameter_description("presence_penalty");
        assert!(pres_desc.contains("repetition"));
        assert!(pres_desc.contains("appear"));
        
        let unknown_desc = manager.get_parameter_description("unknown_param");
        assert_eq!(unknown_desc, "Unknown parameter");
    }

    #[test]
    fn test_use_case_display() {
        assert_eq!(UseCase::Creative.to_string(), "Creative");
        assert_eq!(UseCase::Analytical.to_string(), "Analytical");
        assert_eq!(UseCase::Conversational.to_string(), "Conversational");
        assert_eq!(UseCase::CodeGeneration.to_string(), "Code Generation");
        assert_eq!(UseCase::Summarization.to_string(), "Summarization");
        assert_eq!(UseCase::Translation.to_string(), "Translation");
        assert_eq!(UseCase::Custom.to_string(), "Custom");
    }

    #[test]
    fn test_builtin_presets_validity() {
        let manager = ParameterManager::new();
        
        // All built-in presets should have valid parameters
        for preset in manager.get_presets() {
            let result = manager.validate_parameters(
                preset.temperature,
                preset.max_tokens,
                preset.top_p,
                preset.frequency_penalty,
                preset.presence_penalty,
            );
            assert!(result.is_ok(), "Preset '{}' has invalid parameters: {:?}", preset.name, result);
        }
    }

    #[test]
    fn test_preset_characteristics() {
        let manager = ParameterManager::new();
        
        // Creative presets should have higher temperature
        let creative_presets = manager.get_presets_by_use_case(&UseCase::Creative);
        for preset in creative_presets {
            assert!(preset.temperature >= 0.8, "Creative preset '{}' should have high temperature", preset.name);
        }
        
        // Analytical presets should have lower temperature
        let analytical_presets = manager.get_presets_by_use_case(&UseCase::Analytical);
        for preset in analytical_presets {
            assert!(preset.temperature <= 0.3, "Analytical preset '{}' should have low temperature", preset.name);
        }
        
        // Code generation presets should have very low temperature
        let code_presets = manager.get_presets_by_use_case(&UseCase::CodeGeneration);
        for preset in code_presets {
            assert!(preset.temperature <= 0.2, "Code generation preset '{}' should have very low temperature", preset.name);
        }
    }

    #[test]
    fn test_parameter_ranges() {
        let manager = ParameterManager::new();
        let ranges = manager.get_ranges();
        
        // Check that ranges are sensible
        assert_eq!(ranges.temperature, (0.0, 2.0));
        assert_eq!(ranges.max_tokens, (1, 32768));
        assert_eq!(ranges.top_p, (0.0, 1.0));
        assert_eq!(ranges.frequency_penalty, (-2.0, 2.0));
        assert_eq!(ranges.presence_penalty, (-2.0, 2.0));
    }
}