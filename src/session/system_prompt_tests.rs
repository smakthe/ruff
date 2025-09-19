#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_system_prompt_manager_creation() {
        let manager = SystemPromptManager::new();
        assert_eq!(manager.templates.len(), 0);
    }

    #[test]
    fn test_initialize_builtin_templates() {
        let mut manager = SystemPromptManager::new();
        manager.initialize().unwrap();
        
        assert!(!manager.templates.is_empty());
        
        // Check that we have templates from different categories
        let categories: std::collections::HashSet<_> = manager.templates
            .values()
            .map(|t| &t.category)
            .collect();
        
        assert!(categories.len() > 1);
        
        // Check for specific built-in templates
        let template_names: Vec<_> = manager.templates
            .values()
            .map(|t| &t.name)
            .collect();
        
        assert!(template_names.iter().any(|name| name.contains("Code Assistant")));
        assert!(template_names.iter().any(|name| name.contains("Writing Assistant")));
    }

    #[test]
    fn test_create_template() {
        let mut manager = SystemPromptManager::new();
        
        let template = SystemPromptTemplate {
            id: Uuid::new_v4(), // Will be overwritten
            name: "Test Template".to_string(),
            description: "A test template".to_string(),
            content: "You are a test assistant.".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec!["test".to_string()],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec![],
        };
        
        let id = manager.create_template(template.clone());
        
        let retrieved = manager.get_template(id).unwrap();
        assert_eq!(retrieved.name, "Test Template");
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.usage_count, 0);
    }

    #[test]
    fn test_update_template() {
        let mut manager = SystemPromptManager::new();
        
        let template = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Original Name".to_string(),
            description: "Original description".to_string(),
            content: "Original content".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec![],
        };
        
        let id = manager.create_template(template);
        
        let mut updated_template = manager.get_template(id).unwrap().clone();
        updated_template.name = "Updated Name".to_string();
        updated_template.description = "Updated description".to_string();
        
        manager.update_template(id, updated_template).unwrap();
        
        let retrieved = manager.get_template(id).unwrap();
        assert_eq!(retrieved.name, "Updated Name");
        assert_eq!(retrieved.description, "Updated description");
    }

    #[test]
    fn test_delete_template() {
        let mut manager = SystemPromptManager::new();
        
        let template = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "To Delete".to_string(),
            description: "Will be deleted".to_string(),
            content: "Delete me".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec![],
        };
        
        let id = manager.create_template(template);
        assert!(manager.get_template(id).is_some());
        
        manager.delete_template(id).unwrap();
        assert!(manager.get_template(id).is_none());
    }

    #[test]
    fn test_search_templates() {
        let mut manager = SystemPromptManager::new();
        manager.initialize().unwrap();
        
        // Search for "code" should find code-related templates
        let results = manager.search_templates("code");
        assert!(!results.is_empty());
        
        // Should find templates with "code" in name, description, content, or tags
        for template in results {
            let content_lower = format!("{} {} {} {}", 
                template.name, 
                template.description, 
                template.content,
                template.tags.join(" ")
            ).to_lowercase();
            assert!(content_lower.contains("code"));
        }
        
        // Search for non-existent term should return empty
        let empty_results = manager.search_templates("nonexistentterm12345");
        assert!(empty_results.is_empty());
    }

    #[test]
    fn test_get_templates_by_category() {
        let mut manager = SystemPromptManager::new();
        manager.initialize().unwrap();
        
        let dev_templates = manager.get_templates_by_category(&SystemPromptCategory::Development);
        assert!(!dev_templates.is_empty());
        
        for template in dev_templates {
            assert_eq!(template.category, SystemPromptCategory::Development);
        }
    }

    #[test]
    fn test_toggle_favorite() {
        let mut manager = SystemPromptManager::new();
        
        let template = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Favorite Test".to_string(),
            description: "Test favorite functionality".to_string(),
            content: "Test content".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec![],
        };
        
        let id = manager.create_template(template);
        
        // Initially not favorite
        assert!(!manager.get_template(id).unwrap().is_favorite);
        
        // Toggle to favorite
        let is_favorite = manager.toggle_favorite(id).unwrap();
        assert!(is_favorite);
        assert!(manager.get_template(id).unwrap().is_favorite);
        
        // Toggle back to not favorite
        let is_favorite = manager.toggle_favorite(id).unwrap();
        assert!(!is_favorite);
        assert!(!manager.get_template(id).unwrap().is_favorite);
    }

    #[test]
    fn test_variable_extraction() {
        let manager = SystemPromptManager::new();
        
        // Test simple variables
        let content = "You are a {{role}} with {{experience}} years of experience in {{domain}}.";
        let variables = manager.extract_variables(content);
        
        assert_eq!(variables.len(), 3);
        assert!(variables.contains(&"role".to_string()));
        assert!(variables.contains(&"experience".to_string()));
        assert!(variables.contains(&"domain".to_string()));
        
        // Test no variables
        let content_no_vars = "You are a helpful assistant.";
        let no_variables = manager.extract_variables(content_no_vars);
        assert!(no_variables.is_empty());
        
        // Test malformed variables (should be ignored)
        let content_malformed = "You are a {role} with {{incomplete and {{valid}} variable.";
        let malformed_variables = manager.extract_variables(content_malformed);
        assert_eq!(malformed_variables.len(), 1);
        assert!(malformed_variables.contains(&"valid".to_string()));
        
        // Test duplicate variables (should only appear once)
        let content_duplicates = "{{role}} is a {{role}} with {{role}} experience.";
        let duplicate_variables = manager.extract_variables(content_duplicates);
        assert_eq!(duplicate_variables.len(), 1);
        assert!(duplicate_variables.contains(&"role".to_string()));
    }

    #[test]
    fn test_apply_template_with_variables() {
        let mut manager = SystemPromptManager::new();
        
        let template = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Variable Test".to_string(),
            description: "Test template with variables".to_string(),
            content: "You are a {{role}} specializing in {{field}}. Your experience level is {{level}}.".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec!["role".to_string(), "field".to_string(), "level".to_string()],
        };
        
        let id = manager.create_template(template);
        
        let mut variables = HashMap::new();
        variables.insert("role".to_string(), "expert developer".to_string());
        variables.insert("field".to_string(), "Rust programming".to_string());
        variables.insert("level".to_string(), "senior".to_string());
        
        let result = manager.apply_template(id, &variables).unwrap();
        assert_eq!(result, "You are a expert developer specializing in Rust programming. Your experience level is senior.");
        
        // Check that usage count was incremented
        let template_after = manager.get_template(id).unwrap();
        assert_eq!(template_after.usage_count, 1);
    }

    #[test]
    fn test_apply_template_partial_variables() {
        let mut manager = SystemPromptManager::new();
        
        let template = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Partial Variable Test".to_string(),
            description: "Test template with partial variable substitution".to_string(),
            content: "You are a {{role}} in {{field}}. Your {{missing}} is not provided.".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec!["role".to_string(), "field".to_string(), "missing".to_string()],
        };
        
        let id = manager.create_template(template);
        
        let mut variables = HashMap::new();
        variables.insert("role".to_string(), "developer".to_string());
        variables.insert("field".to_string(), "AI".to_string());
        // Note: "missing" variable is not provided
        
        let result = manager.apply_template(id, &variables).unwrap();
        assert_eq!(result, "You are a developer in AI. Your {{missing}} is not provided.");
    }

    #[test]
    fn test_validate_prompt() {
        let manager = SystemPromptManager::new();
        
        // Valid prompt
        assert!(manager.validate_prompt("You are a helpful assistant.").is_ok());
        
        // Empty prompt should fail
        assert!(manager.validate_prompt("").is_err());
        assert!(manager.validate_prompt("   ").is_err());
        
        // Too long prompt should fail
        let long_prompt = "a".repeat(8001);
        let result = manager.validate_prompt(&long_prompt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("too long")));
        
        // Unclosed braces should fail
        let unclosed_result = manager.validate_prompt("You are a {{role assistant.");
        assert!(unclosed_result.is_err());
        let unclosed_errors = unclosed_result.unwrap_err();
        assert!(unclosed_errors.iter().any(|e| e.contains("Unclosed")));
        
        // Valid prompt with variables
        assert!(manager.validate_prompt("You are a {{role}} with {{experience}}.").is_ok());
    }

    #[test]
    fn test_increment_usage() {
        let mut manager = SystemPromptManager::new();
        
        let template = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Usage Test".to_string(),
            description: "Test usage counting".to_string(),
            content: "Test content".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec![],
        };
        
        let id = manager.create_template(template);
        
        // Initial usage count should be 0
        assert_eq!(manager.get_template(id).unwrap().usage_count, 0);
        
        // Increment usage
        manager.increment_usage(id).unwrap();
        assert_eq!(manager.get_template(id).unwrap().usage_count, 1);
        
        // Increment again
        manager.increment_usage(id).unwrap();
        assert_eq!(manager.get_template(id).unwrap().usage_count, 2);
    }

    #[test]
    fn test_get_favorite_templates() {
        let mut manager = SystemPromptManager::new();
        
        // Create some templates, some favorite, some not
        let template1 = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Favorite 1".to_string(),
            description: "First favorite".to_string(),
            content: "Content 1".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: true,
            variables: vec![],
        };
        
        let template2 = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Not Favorite".to_string(),
            description: "Not a favorite".to_string(),
            content: "Content 2".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec![],
        };
        
        let template3 = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Favorite 2".to_string(),
            description: "Second favorite".to_string(),
            content: "Content 3".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: true,
            variables: vec![],
        };
        
        manager.create_template(template1);
        manager.create_template(template2);
        manager.create_template(template3);
        
        let favorites = manager.get_favorite_templates();
        assert_eq!(favorites.len(), 2);
        
        for template in favorites {
            assert!(template.is_favorite);
            assert!(template.name.contains("Favorite"));
        }
    }

    #[test]
    fn test_system_prompt_category_display() {
        assert_eq!(SystemPromptCategory::Development.to_string(), "Development");
        assert_eq!(SystemPromptCategory::Writing.to_string(), "Writing");
        assert_eq!(SystemPromptCategory::Custom("MyCategory".to_string()).to_string(), "MyCategory");
    }
}