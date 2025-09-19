use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Local};
use crate::RuffError;

pub type SystemPromptId = Uuid;

/// System prompt template for reuse across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptTemplate {
    pub id: SystemPromptId,
    pub name: String,
    pub description: String,
    pub content: String,
    pub category: SystemPromptCategory,
    pub tags: Vec<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub usage_count: u32,
    pub is_favorite: bool,
    pub variables: Vec<String>, // Variables that can be substituted like {{variable}}
}

/// Categories for organizing system prompts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SystemPromptCategory {
    Development,
    Writing,
    Analysis,
    Creative,
    Business,
    Education,
    Research,
    Personal,
    Custom(String),
}

/// System prompt manager for handling templates and session prompts
pub struct SystemPromptManager {
    templates: HashMap<SystemPromptId, SystemPromptTemplate>,
}

impl SystemPromptManager {
    /// Create a new system prompt manager
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }
    
    /// Initialize with built-in templates
    pub fn initialize(&mut self) -> Result<(), RuffError> {
        self.load_builtin_templates();
        Ok(())
    }
    
    /// Create a new system prompt template
    pub fn create_template(&mut self, mut template: SystemPromptTemplate) -> SystemPromptId {
        template.id = Uuid::new_v4();
        template.created_at = Local::now();
        template.updated_at = Local::now();
        template.usage_count = 0;
        
        let id = template.id;
        self.templates.insert(id, template);
        id
    }
    
    /// Update an existing template
    pub fn update_template(&mut self, id: SystemPromptId, mut template: SystemPromptTemplate) -> Result<(), RuffError> {
        if !self.templates.contains_key(&id) {
            return Err(RuffError::App(format!("System prompt template {} not found", id)));
        }
        
        template.id = id;
        template.updated_at = Local::now();
        self.templates.insert(id, template);
        Ok(())
    }
    
    /// Delete a template
    pub fn delete_template(&mut self, id: SystemPromptId) -> Result<(), RuffError> {
        if self.templates.remove(&id).is_none() {
            return Err(RuffError::App(format!("System prompt template {} not found", id)));
        }
        Ok(())
    }
    
    /// Get a template by ID
    pub fn get_template(&self, id: SystemPromptId) -> Option<&SystemPromptTemplate> {
        self.templates.get(&id)
    }
    
    /// Get all templates
    pub fn get_all_templates(&self) -> Vec<&SystemPromptTemplate> {
        self.templates.values().collect()
    }
    
    /// Get templates by category
    pub fn get_templates_by_category(&self, category: &SystemPromptCategory) -> Vec<&SystemPromptTemplate> {
        self.templates
            .values()
            .filter(|template| &template.category == category)
            .collect()
    }
    
    /// Get favorite templates
    pub fn get_favorite_templates(&self) -> Vec<&SystemPromptTemplate> {
        self.templates
            .values()
            .filter(|template| template.is_favorite)
            .collect()
    }
    
    /// Search templates by name or content
    pub fn search_templates(&self, query: &str) -> Vec<&SystemPromptTemplate> {
        let query_lower = query.to_lowercase();
        self.templates
            .values()
            .filter(|template| {
                template.name.to_lowercase().contains(&query_lower) ||
                template.description.to_lowercase().contains(&query_lower) ||
                template.content.to_lowercase().contains(&query_lower) ||
                template.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
    
    /// Toggle favorite status of a template
    pub fn toggle_favorite(&mut self, id: SystemPromptId) -> Result<bool, RuffError> {
        let template = self.templates.get_mut(&id)
            .ok_or_else(|| RuffError::App(format!("System prompt template {} not found", id)))?;
        
        template.is_favorite = !template.is_favorite;
        template.updated_at = Local::now();
        Ok(template.is_favorite)
    }
    
    /// Increment usage count for a template
    pub fn increment_usage(&mut self, id: SystemPromptId) -> Result<(), RuffError> {
        let template = self.templates.get_mut(&id)
            .ok_or_else(|| RuffError::App(format!("System prompt template {} not found", id)))?;
        
        template.usage_count += 1;
        template.updated_at = Local::now();
        Ok(())
    }
    
    /// Apply template with variable substitution
    pub fn apply_template(&mut self, id: SystemPromptId, variables: &HashMap<String, String>) -> Result<String, RuffError> {
        let content = {
            let template = self.templates.get(&id)
                .ok_or_else(|| RuffError::App(format!("System prompt template {} not found", id)))?;
            template.content.clone()
        };
        
        // Increment usage count
        self.increment_usage(id)?;
        
        // Apply variable substitution
        let mut result = content;
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        
        Ok(result)
    }
    
    /// Validate system prompt content
    pub fn validate_prompt(&self, content: &str) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        if content.trim().is_empty() {
            errors.push("System prompt cannot be empty".to_string());
        }
        
        if content.len() > 8000 {
            errors.push("System prompt is too long (max 8000 characters)".to_string());
        }
        
        // Check for unclosed variable placeholders
        let mut open_braces = 0;
        for ch in content.chars() {
            match ch {
                '{' => open_braces += 1,
                '}' => {
                    if open_braces > 0 {
                        open_braces -= 1;
                    }
                }
                _ => {}
            }
        }
        
        if open_braces > 0 {
            errors.push("Unclosed variable placeholders found".to_string());
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Extract variables from prompt content
    pub fn extract_variables(&self, content: &str) -> Vec<String> {
        let mut variables = Vec::new();
        let mut chars = content.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '{' && chars.peek() == Some(&'{') {
                chars.next(); // consume second '{'
                let mut var_name = String::new();
                
                while let Some(ch) = chars.next() {
                    if ch == '}' && chars.peek() == Some(&'}') {
                        chars.next(); // consume second '}'
                        if !var_name.is_empty() && !variables.contains(&var_name) {
                            variables.push(var_name);
                        }
                        break;
                    } else {
                        var_name.push(ch);
                    }
                }
            }
        }
        
        variables
    }
    
    /// Load built-in system prompt templates
    fn load_builtin_templates(&mut self) {
        let builtin_templates = vec![
            SystemPromptTemplate {
                id: Uuid::new_v4(),
                name: "Code Assistant".to_string(),
                description: "Helpful coding assistant with best practices focus".to_string(),
                content: "You are an expert software developer and coding assistant. Help write clean, efficient, and well-documented code. Focus on best practices, security, and maintainability. Explain your reasoning and suggest improvements when appropriate.".to_string(),
                category: SystemPromptCategory::Development,
                tags: vec!["coding".to_string(), "development".to_string(), "best-practices".to_string()],
                created_at: Local::now(),
                updated_at: Local::now(),
                usage_count: 0,
                is_favorite: false,
                variables: vec![],
            },
            SystemPromptTemplate {
                id: Uuid::new_v4(),
                name: "Writing Assistant".to_string(),
                description: "Professional writing and editing assistant".to_string(),
                content: "You are a professional writing assistant. Help improve writing clarity, grammar, style, and flow. Maintain the original tone and intent while making suggestions for better readability and impact.".to_string(),
                category: SystemPromptCategory::Writing,
                tags: vec!["writing".to_string(), "editing".to_string(), "grammar".to_string()],
                created_at: Local::now(),
                updated_at: Local::now(),
                usage_count: 0,
                is_favorite: false,
                variables: vec![],
            },
            SystemPromptTemplate {
                id: Uuid::new_v4(),
                name: "Research Assistant".to_string(),
                description: "Analytical research and fact-checking assistant".to_string(),
                content: "You are a thorough research assistant. Provide accurate, well-sourced information and help analyze complex topics. Always cite sources when possible and distinguish between facts and opinions.".to_string(),
                category: SystemPromptCategory::Research,
                tags: vec!["research".to_string(), "analysis".to_string(), "facts".to_string()],
                created_at: Local::now(),
                updated_at: Local::now(),
                usage_count: 0,
                is_favorite: false,
                variables: vec![],
            },
            SystemPromptTemplate {
                id: Uuid::new_v4(),
                name: "Creative Brainstorming".to_string(),
                description: "Creative ideation and brainstorming partner".to_string(),
                content: "You are a creative brainstorming partner. Generate innovative ideas, think outside the box, and help explore different perspectives. Be encouraging and build upon ideas to create even better solutions.".to_string(),
                category: SystemPromptCategory::Creative,
                tags: vec!["creative".to_string(), "brainstorming".to_string(), "innovation".to_string()],
                created_at: Local::now(),
                updated_at: Local::now(),
                usage_count: 0,
                is_favorite: false,
                variables: vec![],
            },
            SystemPromptTemplate {
                id: Uuid::new_v4(),
                name: "Business Analyst".to_string(),
                description: "Strategic business analysis and planning assistant".to_string(),
                content: "You are a strategic business analyst. Help analyze business problems, identify opportunities, and develop actionable solutions. Focus on data-driven insights and practical recommendations.".to_string(),
                category: SystemPromptCategory::Business,
                tags: vec!["business".to_string(), "strategy".to_string(), "analysis".to_string()],
                created_at: Local::now(),
                updated_at: Local::now(),
                usage_count: 0,
                is_favorite: false,
                variables: vec![],
            },
            SystemPromptTemplate {
                id: Uuid::new_v4(),
                name: "Custom Role".to_string(),
                description: "Customizable role-based assistant".to_string(),
                content: "You are a {{role}} with expertise in {{domain}}. Your primary goal is to {{objective}}. Always consider {{constraints}} when providing assistance.".to_string(),
                category: SystemPromptCategory::Personal,
                tags: vec!["custom".to_string(), "flexible".to_string(), "role-based".to_string()],
                created_at: Local::now(),
                updated_at: Local::now(),
                usage_count: 0,
                is_favorite: false,
                variables: vec!["role".to_string(), "domain".to_string(), "objective".to_string(), "constraints".to_string()],
            },
        ];
        
        for template in builtin_templates {
            self.templates.insert(template.id, template);
        }
    }
}

impl Default for SystemPromptManager {
    fn default() -> Self {
        let mut manager = Self::new();
        manager.initialize().unwrap();
        manager
    }
}

impl std::fmt::Display for SystemPromptCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemPromptCategory::Development => write!(f, "Development"),
            SystemPromptCategory::Writing => write!(f, "Writing"),
            SystemPromptCategory::Analysis => write!(f, "Analysis"),
            SystemPromptCategory::Creative => write!(f, "Creative"),
            SystemPromptCategory::Business => write!(f, "Business"),
            SystemPromptCategory::Education => write!(f, "Education"),
            SystemPromptCategory::Research => write!(f, "Research"),
            SystemPromptCategory::Personal => write!(f, "Personal"),
            SystemPromptCategory::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn test_search_templates() {
        let mut manager = SystemPromptManager::new();
        manager.initialize().unwrap();
        
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
    }

    #[test]
    fn test_variable_extraction() {
        let manager = SystemPromptManager::new();
        
        let content = "You are a {{role}} with {{experience}} years of experience in {{domain}}.";
        let variables = manager.extract_variables(content);
        
        assert_eq!(variables.len(), 3);
        assert!(variables.contains(&"role".to_string()));
        assert!(variables.contains(&"experience".to_string()));
        assert!(variables.contains(&"domain".to_string()));
    }

    #[test]
    fn test_apply_template_with_variables() {
        let mut manager = SystemPromptManager::new();
        
        let template = SystemPromptTemplate {
            id: Uuid::new_v4(),
            name: "Variable Test".to_string(),
            description: "Test template with variables".to_string(),
            content: "You are a {{role}} specializing in {{field}}.".to_string(),
            category: SystemPromptCategory::Personal,
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            variables: vec!["role".to_string(), "field".to_string()],
        };
        
        let id = manager.create_template(template);
        
        let mut variables = HashMap::new();
        variables.insert("role".to_string(), "expert developer".to_string());
        variables.insert("field".to_string(), "Rust programming".to_string());
        
        let result = manager.apply_template(id, &variables).unwrap();
        assert_eq!(result, "You are a expert developer specializing in Rust programming.");
    }

    #[test]
    fn test_validate_prompt() {
        let manager = SystemPromptManager::new();
        
        // Valid prompt
        assert!(manager.validate_prompt("You are a helpful assistant.").is_ok());
        
        // Empty prompt
        assert!(manager.validate_prompt("").is_err());
        assert!(manager.validate_prompt("   ").is_err());
        
        // Too long prompt
        let long_prompt = "a".repeat(8001);
        assert!(manager.validate_prompt(&long_prompt).is_err());
        
        // Unclosed braces
        assert!(manager.validate_prompt("You are a {{role assistant.").is_err());
    }
}