use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    events::{AppEvent, EventBus},
    EnhancedError,
};

pub type TemplateId = Uuid;

/// Represents a conversation template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTemplate {
    pub id: TemplateId,
    pub name: String,
    pub description: String,
    pub category: TemplateCategory,
    pub system_prompt: Option<String>,
    pub initial_messages: Vec<TemplateMessage>,
    pub parameters: TemplateParameters,
    pub tags: Vec<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub usage_count: u32,
    pub is_favorite: bool,
    pub is_shared: bool,
    pub author: Option<String>,
}

/// Template message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMessage {
    pub role: MessageRole,
    pub content: String,
    pub variables: Vec<String>, // Variables that can be substituted
}

/// Message roles for templates
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Template categories for organization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TemplateCategory {
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

/// Template parameters for AI model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParameters {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub suggested_model: Option<String>,
}

/// Template search filters
#[derive(Debug, Clone)]
pub struct TemplateSearchFilters {
    pub category: Option<TemplateCategory>,
    pub tags: Option<Vec<String>>,
    pub favorites_only: bool,
    pub shared_only: bool,
    pub author: Option<String>,
}

/// Template search result
#[derive(Debug, Clone)]
pub struct TemplateSearchResult {
    pub template: ConversationTemplate,
    pub relevance_score: f32,
}

/// Template variable substitution
#[derive(Debug, Clone)]
pub struct TemplateVariable {
    pub name: String,
    pub value: String,
    pub description: Option<String>,
}

/// Manages conversation templates
pub struct TemplateManager {
    templates: Arc<RwLock<HashMap<TemplateId, ConversationTemplate>>>,
    templates_dir: PathBuf,
    event_bus: Arc<EventBus>,
}

impl TemplateManager {
    /// Create a new template manager
    pub fn new(templates_dir: PathBuf, event_bus: Arc<EventBus>) -> Result<Self, EnhancedError> {
        // Ensure templates directory exists
        if !templates_dir.exists() {
            fs::create_dir_all(&templates_dir)?;
        }

        let manager = Self {
            templates: Arc::new(RwLock::new(HashMap::new())),
            templates_dir,
            event_bus,
        };

        Ok(manager)
    }

    /// Load templates from disk
    pub async fn load_templates(&self) -> Result<(), EnhancedError> {
        let mut templates = self.templates.write().await;
        templates.clear();

        // Load built-in templates
        self.load_builtin_templates(&mut templates).await?;

        // Load user templates from disk
        if self.templates_dir.exists() {
            for entry in fs::read_dir(&self.templates_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    match self.load_template_from_file(&path).await {
                        Ok(template) => {
                            templates.insert(template.id, template);
                        }
                        Err(e) => {
                            eprintln!("Failed to load template from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        self.event_bus
            .publish(AppEvent::ConfigurationChanged)
            .await
            .map_err(|e| EnhancedError::api(format!("Event bus error: {}", e)))?;

        Ok(())
    }

    /// Create a new template
    pub async fn create_template(
        &self,
        mut template: ConversationTemplate,
    ) -> Result<TemplateId, EnhancedError> {
        template.id = Uuid::new_v4();
        template.created_at = Local::now();
        template.updated_at = Local::now();
        template.usage_count = 0;

        // Save to disk
        self.save_template_to_file(&template).await?;

        // Add to memory
        let template_id = template.id;
        {
            let mut templates = self.templates.write().await;
            templates.insert(template_id, template);
        }

        self.event_bus
            .publish(AppEvent::ConfigurationChanged)
            .await
            .map_err(|e| EnhancedError::api(format!("Event bus error: {}", e)))?;

        Ok(template_id)
    }

    /// Update an existing template
    pub async fn update_template(
        &self,
        template_id: TemplateId,
        mut template: ConversationTemplate,
    ) -> Result<(), EnhancedError> {
        template.id = template_id;
        template.updated_at = Local::now();

        // Save to disk
        self.save_template_to_file(&template).await?;

        // Update in memory
        {
            let mut templates = self.templates.write().await;
            templates.insert(template_id, template);
        }

        self.event_bus
            .publish(AppEvent::ConfigurationChanged)
            .await
            .map_err(|e| EnhancedError::api(format!("Event bus error: {}", e)))?;

        Ok(())
    }

    /// Delete a template
    pub async fn delete_template(&self, template_id: TemplateId) -> Result<(), EnhancedError> {
        // Remove from memory
        let template = {
            let mut templates = self.templates.write().await;
            templates.remove(&template_id)
        };

        if let Some(template) = template {
            // Remove from disk
            let file_path = self.get_template_file_path(&template);
            if file_path.exists() {
                fs::remove_file(file_path)?;
            }

            self.event_bus
                .publish(AppEvent::ConfigurationChanged)
                .await
                .map_err(|e| EnhancedError::api(format!("Event bus error: {}", e)))?;
        }

        Ok(())
    }

    /// Get a template by ID
    pub async fn get_template(&self, template_id: TemplateId) -> Option<ConversationTemplate> {
        let templates = self.templates.read().await;
        templates.get(&template_id).cloned()
    }

    /// List all templates
    pub async fn list_templates(&self) -> Vec<ConversationTemplate> {
        let templates = self.templates.read().await;
        templates.values().cloned().collect()
    }

    /// Search templates
    pub async fn search_templates(
        &self,
        query: &str,
        filters: Option<TemplateSearchFilters>,
    ) -> Vec<TemplateSearchResult> {
        let templates = self.templates.read().await;
        let mut results = Vec::new();

        for template in templates.values() {
            // Apply filters
            if let Some(ref filters) = filters {
                if let Some(ref category) = filters.category {
                    if &template.category != category {
                        continue;
                    }
                }

                if filters.favorites_only && !template.is_favorite {
                    continue;
                }

                if filters.shared_only && !template.is_shared {
                    continue;
                }

                if let Some(ref author) = filters.author {
                    if template.author.as_ref() != Some(author) {
                        continue;
                    }
                }

                if let Some(ref filter_tags) = filters.tags {
                    if !filter_tags.iter().any(|tag| template.tags.contains(tag)) {
                        continue;
                    }
                }
            }

            // Calculate relevance score
            let relevance_score = self.calculate_relevance_score(template, query);

            if relevance_score > 0.0 {
                results.push(TemplateSearchResult {
                    template: template.clone(),
                    relevance_score,
                });
            }
        }

        // Sort by relevance score (descending)
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Get templates by category
    pub async fn get_templates_by_category(
        &self,
        category: TemplateCategory,
    ) -> Vec<ConversationTemplate> {
        let templates = self.templates.read().await;
        templates
            .values()
            .filter(|template| template.category == category)
            .cloned()
            .collect()
    }

    /// Get favorite templates
    pub async fn get_favorite_templates(&self) -> Vec<ConversationTemplate> {
        let templates = self.templates.read().await;
        templates
            .values()
            .filter(|template| template.is_favorite)
            .cloned()
            .collect()
    }

    /// Toggle template favorite status
    pub async fn toggle_favorite(&self, template_id: TemplateId) -> Result<bool, EnhancedError> {
        let mut templates = self.templates.write().await;

        if let Some(template) = templates.get_mut(&template_id) {
            template.is_favorite = !template.is_favorite;
            template.updated_at = Local::now();

            // Save to disk
            let template_clone = template.clone();
            drop(templates); // Release the lock before async operation
            self.save_template_to_file(&template_clone).await?;

            Ok(template_clone.is_favorite)
        } else {
            Err(EnhancedError::api("Template not found".to_string()))
        }
    }

    /// Increment template usage count
    pub async fn increment_usage(&self, template_id: TemplateId) -> Result<(), EnhancedError> {
        let mut templates = self.templates.write().await;

        if let Some(template) = templates.get_mut(&template_id) {
            template.usage_count += 1;
            template.updated_at = Local::now();

            // Save to disk
            let template_clone = template.clone();
            drop(templates); // Release the lock before async operation
            self.save_template_to_file(&template_clone).await?;
        }

        Ok(())
    }

    /// Apply template with variable substitution
    pub async fn apply_template(
        &self,
        template_id: TemplateId,
        variables: Vec<TemplateVariable>,
    ) -> Result<AppliedTemplate, EnhancedError> {
        let template = self
            .get_template(template_id)
            .await
            .ok_or_else(|| EnhancedError::api("Template not found".to_string()))?;

        // Increment usage count
        self.increment_usage(template_id).await?;

        // Create variable substitution map
        let var_map: HashMap<String, String> = variables
            .into_iter()
            .map(|var| (var.name, var.value))
            .collect();

        // Apply substitutions
        let system_prompt = template
            .system_prompt
            .as_ref()
            .map(|prompt| self.substitute_variables(prompt, &var_map));

        let initial_messages: Vec<_> = template
            .initial_messages
            .iter()
            .map(|msg| AppliedTemplateMessage {
                role: msg.role.clone(),
                content: self.substitute_variables(&msg.content, &var_map),
            })
            .collect();

        Ok(AppliedTemplate {
            template_id,
            name: template.name,
            system_prompt,
            initial_messages,
            parameters: template.parameters,
        })
    }

    /// Export templates to a file
    pub async fn export_templates(
        &self,
        file_path: &Path,
        template_ids: Option<Vec<TemplateId>>,
    ) -> Result<(), EnhancedError> {
        let templates = self.templates.read().await;

        let templates_to_export: Vec<_> = if let Some(ids) = template_ids {
            ids.into_iter()
                .filter_map(|id| templates.get(&id).cloned())
                .collect()
        } else {
            templates.values().cloned().collect()
        };

        let json = serde_json::to_string_pretty(&templates_to_export)?;
        fs::write(file_path, json)?;

        Ok(())
    }

    /// Import templates from a file
    pub async fn import_templates(
        &self,
        file_path: &Path,
        overwrite_existing: bool,
    ) -> Result<Vec<TemplateId>, EnhancedError> {
        let content = fs::read_to_string(file_path)?;
        let imported_templates: Vec<ConversationTemplate> = serde_json::from_str(&content)?;

        let mut imported_ids = Vec::new();

        for mut template in imported_templates {
            let original_id = template.id;

            // Check if template already exists
            let exists = {
                let templates = self.templates.read().await;
                templates.contains_key(&original_id)
            };

            if exists && !overwrite_existing {
                // Generate new ID to avoid conflicts
                template.id = Uuid::new_v4();
            }

            template.created_at = Local::now();
            template.updated_at = Local::now();

            let template_id = self.create_template(template).await?;
            imported_ids.push(template_id);
        }

        Ok(imported_ids)
    }

    /// Get template statistics
    pub async fn get_template_stats(&self) -> TemplateStats {
        let templates = self.templates.read().await;

        let total_templates = templates.len();
        let favorite_count = templates.values().filter(|t| t.is_favorite).count();
        let shared_count = templates.values().filter(|t| t.is_shared).count();

        let mut category_counts = HashMap::new();
        for template in templates.values() {
            *category_counts
                .entry(template.category.clone())
                .or_insert(0) += 1;
        }

        let most_used = templates
            .values()
            .max_by_key(|t| t.usage_count)
            .map(|t| (t.id, t.usage_count));

        TemplateStats {
            total_templates,
            favorite_count,
            shared_count,
            category_counts,
            most_used,
        }
    }

    // Private helper methods

    async fn load_builtin_templates(
        &self,
        templates: &mut HashMap<TemplateId, ConversationTemplate>,
    ) -> Result<(), EnhancedError> {
        let builtin_templates = vec![
            self.create_code_review_template(),
            self.create_writing_assistant_template(),
            self.create_brainstorming_template(),
            self.create_explain_code_template(),
            self.create_debug_helper_template(),
        ];

        for template in builtin_templates {
            templates.insert(template.id, template);
        }

        Ok(())
    }

    fn create_code_review_template(&self) -> ConversationTemplate {
        ConversationTemplate {
            id: Uuid::new_v4(),
            name: "Code Review Assistant".to_string(),
            description: "Help review code for best practices, bugs, and improvements".to_string(),
            category: TemplateCategory::Development,
            system_prompt: Some("You are an expert code reviewer. Analyze the provided code for potential issues, suggest improvements, and highlight best practices. Focus on readability, performance, security, and maintainability.".to_string()),
            initial_messages: vec![
                TemplateMessage {
                    role: MessageRole::User,
                    content: "Please review this code:\n\n```{{language}}\n{{code}}\n```\n\nFocus on: {{focus_areas}}".to_string(),
                    variables: vec!["language".to_string(), "code".to_string(), "focus_areas".to_string()],
                }
            ],
            parameters: TemplateParameters {
                temperature: Some(0.3),
                max_tokens: Some(2000),
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                suggested_model: Some("gpt-4".to_string()),
            },
            tags: vec!["code".to_string(), "review".to_string(), "development".to_string()],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            is_shared: true,
            author: Some("Ruff Built-in".to_string()),
        }
    }

    fn create_writing_assistant_template(&self) -> ConversationTemplate {
        ConversationTemplate {
            id: Uuid::new_v4(),
            name: "Writing Assistant".to_string(),
            description: "Help improve writing style, grammar, and clarity".to_string(),
            category: TemplateCategory::Writing,
            system_prompt: Some("You are a professional writing assistant. Help improve text by correcting grammar, enhancing clarity, improving flow, and suggesting better word choices while maintaining the original tone and intent.".to_string()),
            initial_messages: vec![
                TemplateMessage {
                    role: MessageRole::User,
                    content: "Please help me improve this {{document_type}}:\n\n{{text}}\n\nFocus on: {{improvement_areas}}".to_string(),
                    variables: vec!["document_type".to_string(), "text".to_string(), "improvement_areas".to_string()],
                }
            ],
            parameters: TemplateParameters {
                temperature: Some(0.4),
                max_tokens: Some(1500),
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                suggested_model: Some("gpt-3.5-turbo".to_string()),
            },
            tags: vec!["writing".to_string(), "grammar".to_string(), "editing".to_string()],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            is_shared: true,
            author: Some("Ruff Built-in".to_string()),
        }
    }

    fn create_brainstorming_template(&self) -> ConversationTemplate {
        ConversationTemplate {
            id: Uuid::new_v4(),
            name: "Brainstorming Session".to_string(),
            description: "Generate creative ideas and solutions for any topic".to_string(),
            category: TemplateCategory::Creative,
            system_prompt: Some("You are a creative brainstorming partner. Generate diverse, innovative ideas and help explore different perspectives. Be encouraging and build upon ideas to create even better solutions.".to_string()),
            initial_messages: vec![
                TemplateMessage {
                    role: MessageRole::User,
                    content: "I need help brainstorming ideas for: {{topic}}\n\nContext: {{context}}\n\nConstraints: {{constraints}}\n\nPlease generate {{number}} creative ideas.".to_string(),
                    variables: vec!["topic".to_string(), "context".to_string(), "constraints".to_string(), "number".to_string()],
                }
            ],
            parameters: TemplateParameters {
                temperature: Some(0.8),
                max_tokens: Some(1000),
                top_p: None,
                frequency_penalty: Some(0.3),
                presence_penalty: Some(0.3),
                suggested_model: Some("gpt-4".to_string()),
            },
            tags: vec!["creative".to_string(), "brainstorming".to_string(), "ideas".to_string()],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            is_shared: true,
            author: Some("Ruff Built-in".to_string()),
        }
    }

    fn create_explain_code_template(&self) -> ConversationTemplate {
        ConversationTemplate {
            id: Uuid::new_v4(),
            name: "Code Explainer".to_string(),
            description: "Explain how code works in simple terms".to_string(),
            category: TemplateCategory::Education,
            system_prompt: Some("You are a patient programming teacher. Explain code clearly and simply, breaking down complex concepts into understandable parts. Use analogies when helpful and provide context about why the code works the way it does.".to_string()),
            initial_messages: vec![
                TemplateMessage {
                    role: MessageRole::User,
                    content: "Please explain this {{language}} code to me:\n\n```{{language}}\n{{code}}\n```\n\nMy experience level: {{experience_level}}".to_string(),
                    variables: vec!["language".to_string(), "code".to_string(), "experience_level".to_string()],
                }
            ],
            parameters: TemplateParameters {
                temperature: Some(0.5),
                max_tokens: Some(1500),
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                suggested_model: Some("gpt-4".to_string()),
            },
            tags: vec!["education".to_string(), "code".to_string(), "explanation".to_string()],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            is_shared: true,
            author: Some("Ruff Built-in".to_string()),
        }
    }

    fn create_debug_helper_template(&self) -> ConversationTemplate {
        ConversationTemplate {
            id: Uuid::new_v4(),
            name: "Debug Helper".to_string(),
            description: "Help debug code issues and errors".to_string(),
            category: TemplateCategory::Development,
            system_prompt: Some("You are an expert debugger. Help identify the root cause of bugs, suggest fixes, and explain why the issue occurred. Provide step-by-step debugging approaches when appropriate.".to_string()),
            initial_messages: vec![
                TemplateMessage {
                    role: MessageRole::User,
                    content: "I'm having trouble with this {{language}} code:\n\n```{{language}}\n{{code}}\n```\n\nError message: {{error_message}}\n\nExpected behavior: {{expected_behavior}}\nActual behavior: {{actual_behavior}}".to_string(),
                    variables: vec!["language".to_string(), "code".to_string(), "error_message".to_string(), "expected_behavior".to_string(), "actual_behavior".to_string()],
                }
            ],
            parameters: TemplateParameters {
                temperature: Some(0.2),
                max_tokens: Some(2000),
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                suggested_model: Some("gpt-4".to_string()),
            },
            tags: vec!["debugging".to_string(), "code".to_string(), "troubleshooting".to_string()],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            is_shared: true,
            author: Some("Ruff Built-in".to_string()),
        }
    }

    async fn load_template_from_file(
        &self,
        file_path: &Path,
    ) -> Result<ConversationTemplate, EnhancedError> {
        let content = fs::read_to_string(file_path)?;
        let template: ConversationTemplate = serde_json::from_str(&content)?;
        Ok(template)
    }

    async fn save_template_to_file(
        &self,
        template: &ConversationTemplate,
    ) -> Result<(), EnhancedError> {
        let file_path = self.get_template_file_path(template);
        let json = serde_json::to_string_pretty(template)?;
        fs::write(file_path, json)?;
        Ok(())
    }

    fn get_template_file_path(&self, template: &ConversationTemplate) -> PathBuf {
        self.templates_dir.join(format!("{}.json", template.id))
    }

    fn calculate_relevance_score(&self, template: &ConversationTemplate, query: &str) -> f32 {
        if query.is_empty() {
            return 1.0; // Return all templates if no query
        }

        let query_lower = query.to_lowercase();
        let mut score = 0.0;

        // Name match (highest weight)
        if template.name.to_lowercase().contains(&query_lower) {
            score += 3.0;
        }

        // Description match
        if template.description.to_lowercase().contains(&query_lower) {
            score += 2.0;
        }

        // Tag match
        for tag in &template.tags {
            if tag.to_lowercase().contains(&query_lower) {
                score += 1.5;
            }
        }

        // Category match
        let category_str = match &template.category {
            TemplateCategory::Development => "development",
            TemplateCategory::Writing => "writing",
            TemplateCategory::Analysis => "analysis",
            TemplateCategory::Creative => "creative",
            TemplateCategory::Business => "business",
            TemplateCategory::Education => "education",
            TemplateCategory::Research => "research",
            TemplateCategory::Personal => "personal",
            TemplateCategory::Custom(name) => name,
        };

        if category_str.to_lowercase().contains(&query_lower) {
            score += 1.0;
        }

        // System prompt match (lower weight)
        if let Some(ref system_prompt) = template.system_prompt {
            if system_prompt.to_lowercase().contains(&query_lower) {
                score += 0.5;
            }
        }

        // Boost score for favorites and frequently used templates
        if template.is_favorite {
            score *= 1.2;
        }

        if template.usage_count > 0 {
            score *= 1.0 + (template.usage_count as f32 * 0.1);
        }

        score
    }

    fn substitute_variables(&self, text: &str, variables: &HashMap<String, String>) -> String {
        let mut result = text.to_string();

        for (name, value) in variables {
            let placeholder = format!("{{{{{}}}}}", name);
            result = result.replace(&placeholder, value);
        }

        result
    }
}

/// Applied template with variable substitutions
#[derive(Debug, Clone)]
pub struct AppliedTemplate {
    pub template_id: TemplateId,
    pub name: String,
    pub system_prompt: Option<String>,
    pub initial_messages: Vec<AppliedTemplateMessage>,
    pub parameters: TemplateParameters,
}

/// Applied template message
#[derive(Debug, Clone)]
pub struct AppliedTemplateMessage {
    pub role: MessageRole,
    pub content: String,
}

/// Template statistics
#[derive(Debug, Clone)]
pub struct TemplateStats {
    pub total_templates: usize,
    pub favorite_count: usize,
    pub shared_count: usize,
    pub category_counts: HashMap<TemplateCategory, usize>,
    pub most_used: Option<(TemplateId, u32)>,
}

impl Default for TemplateParameters {
    fn default() -> Self {
        Self {
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            suggested_model: None,
        }
    }
}

impl Default for TemplateSearchFilters {
    fn default() -> Self {
        Self {
            category: None,
            tags: None,
            favorites_only: false,
            shared_only: false,
            author: None,
        }
    }
}

impl std::fmt::Display for TemplateCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateCategory::Development => write!(f, "Development"),
            TemplateCategory::Writing => write!(f, "Writing"),
            TemplateCategory::Analysis => write!(f, "Analysis"),
            TemplateCategory::Creative => write!(f, "Creative"),
            TemplateCategory::Business => write!(f, "Business"),
            TemplateCategory::Education => write!(f, "Education"),
            TemplateCategory::Research => write!(f, "Research"),
            TemplateCategory::Personal => write!(f, "Personal"),
            TemplateCategory::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_template_manager() -> (TemplateManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let event_bus = Arc::new(EventBus::new());
        let manager = TemplateManager::new(temp_dir.path().to_path_buf(), event_bus).unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_template_manager_creation() {
        let (manager, _temp_dir) = create_test_template_manager().await;

        // Load templates should work
        manager.load_templates().await.unwrap();

        // Should have built-in templates
        let templates = manager.list_templates().await;
        assert!(!templates.is_empty());
    }

    #[tokio::test]
    async fn test_create_and_get_template() {
        let (manager, _temp_dir) = create_test_template_manager().await;

        let template = ConversationTemplate {
            id: Uuid::new_v4(), // Will be overridden
            name: "Test Template".to_string(),
            description: "A test template".to_string(),
            category: TemplateCategory::Personal,
            system_prompt: Some("You are a test assistant".to_string()),
            initial_messages: vec![TemplateMessage {
                role: MessageRole::User,
                content: "Hello {{name}}!".to_string(),
                variables: vec!["name".to_string()],
            }],
            parameters: TemplateParameters::default(),
            tags: vec!["test".to_string()],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            is_shared: false,
            author: Some("Test Author".to_string()),
        };

        let template_id = manager.create_template(template.clone()).await.unwrap();

        let retrieved = manager.get_template(template_id).await.unwrap();
        assert_eq!(retrieved.name, "Test Template");
        assert_eq!(retrieved.description, "A test template");
    }

    #[tokio::test]
    async fn test_search_templates() {
        let (manager, _temp_dir) = create_test_template_manager().await;
        manager.load_templates().await.unwrap();

        // Search for code-related templates
        let results = manager.search_templates("code", None).await;
        assert!(!results.is_empty());

        // All results should have relevance scores
        for result in &results {
            assert!(result.relevance_score > 0.0);
        }
    }

    #[tokio::test]
    async fn test_template_categories() {
        let (manager, _temp_dir) = create_test_template_manager().await;
        manager.load_templates().await.unwrap();

        let dev_templates = manager
            .get_templates_by_category(TemplateCategory::Development)
            .await;
        assert!(!dev_templates.is_empty());

        for template in &dev_templates {
            assert_eq!(template.category, TemplateCategory::Development);
        }
    }

    #[tokio::test]
    async fn test_toggle_favorite() {
        let (manager, _temp_dir) = create_test_template_manager().await;
        manager.load_templates().await.unwrap();

        let templates = manager.list_templates().await;
        let template_id = templates[0].id;

        // Initially not favorite
        assert!(!templates[0].is_favorite);

        // Toggle to favorite
        let is_favorite = manager.toggle_favorite(template_id).await.unwrap();
        assert!(is_favorite);

        // Verify it's now favorite
        let updated_template = manager.get_template(template_id).await.unwrap();
        assert!(updated_template.is_favorite);
    }

    #[tokio::test]
    async fn test_apply_template_with_variables() {
        let (manager, _temp_dir) = create_test_template_manager().await;

        let template = ConversationTemplate {
            id: Uuid::new_v4(),
            name: "Variable Test".to_string(),
            description: "Test variable substitution".to_string(),
            category: TemplateCategory::Personal,
            system_prompt: Some("You are helping {{user_name}}".to_string()),
            initial_messages: vec![TemplateMessage {
                role: MessageRole::User,
                content: "Hello, my name is {{user_name}} and I work in {{industry}}".to_string(),
                variables: vec!["user_name".to_string(), "industry".to_string()],
            }],
            parameters: TemplateParameters::default(),
            tags: vec![],
            created_at: Local::now(),
            updated_at: Local::now(),
            usage_count: 0,
            is_favorite: false,
            is_shared: false,
            author: None,
        };

        let template_id = manager.create_template(template).await.unwrap();

        let variables = vec![
            TemplateVariable {
                name: "user_name".to_string(),
                value: "Alice".to_string(),
                description: None,
            },
            TemplateVariable {
                name: "industry".to_string(),
                value: "technology".to_string(),
                description: None,
            },
        ];

        let applied = manager
            .apply_template(template_id, variables)
            .await
            .unwrap();

        assert_eq!(
            applied.system_prompt,
            Some("You are helping Alice".to_string())
        );
        assert_eq!(
            applied.initial_messages[0].content,
            "Hello, my name is Alice and I work in technology"
        );
    }

    #[tokio::test]
    async fn test_template_stats() {
        let (manager, _temp_dir) = create_test_template_manager().await;
        manager.load_templates().await.unwrap();

        let stats = manager.get_template_stats().await;

        assert!(stats.total_templates > 0);
        assert!(!stats.category_counts.is_empty());
    }

    #[tokio::test]
    async fn test_export_import_templates() {
        let (manager, temp_dir) = create_test_template_manager().await;
        manager.load_templates().await.unwrap();

        let export_path = temp_dir.path().join("exported_templates.json");

        // Export all templates
        manager.export_templates(&export_path, None).await.unwrap();
        assert!(export_path.exists());

        // Create a new manager and import
        let (manager2, _temp_dir2) = create_test_template_manager().await;
        let imported_ids = manager2
            .import_templates(&export_path, false)
            .await
            .unwrap();

        assert!(!imported_ids.is_empty());

        let imported_templates = manager2.list_templates().await;
        assert_eq!(imported_templates.len(), imported_ids.len());
    }
}
