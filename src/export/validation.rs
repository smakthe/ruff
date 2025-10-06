//! Data validation functionality

use serde::{Deserialize, Serialize};
use crate::session::manager::{ChatSession, ChatSessionWithMessages, Message, MessageRole};

/// Data validator for import/export operations
pub struct DataValidator {
    max_title_length: usize,
    max_message_length: usize,
    max_messages_per_session: usize,
}

/// Validation result containing errors and warnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub session_count: usize,
    pub message_count: usize,
}

/// Validation rules for import data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    pub max_title_length: usize,
    pub max_message_length: usize,
    pub max_messages_per_session: usize,
    pub require_non_empty_content: bool,
    pub allow_unknown_roles: bool,
    pub require_timestamps: bool,
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self {
            max_title_length: 200,
            max_message_length: 100_000, // 100KB
            max_messages_per_session: 10_000,
            require_non_empty_content: true,
            allow_unknown_roles: false,
            require_timestamps: false,
        }
    }
}

impl DataValidator {
    /// Create a new data validator with default rules
    pub fn new() -> Self {
        let rules = ValidationRules::default();
        Self {
            max_title_length: rules.max_title_length,
            max_message_length: rules.max_message_length,
            max_messages_per_session: rules.max_messages_per_session,
        }
    }

    /// Create a new data validator with custom rules
    pub fn with_rules(rules: ValidationRules) -> Self {
        Self {
            max_title_length: rules.max_title_length,
            max_message_length: rules.max_message_length,
            max_messages_per_session: rules.max_messages_per_session,
        }
    }

    /// Validate a collection of sessions with embedded messages
    pub fn validate_sessions_with_messages(&self, sessions: &[ChatSessionWithMessages]) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut total_messages = 0;

        if sessions.is_empty() {
            errors.push("No sessions found to validate".to_string());
            return ValidationResult {
                is_valid: false,
                errors,
                warnings,
                session_count: 0,
                message_count: 0,
            };
        }

        for (session_index, session) in sessions.iter().enumerate() {
            let session_prefix = format!("Session {} ('{}')", session_index + 1, session.title);

            // Validate session structure
            self.validate_session_structure_with_messages(session, &session_prefix, &mut errors, &mut warnings);

            // Validate messages
            self.validate_session_messages(session, &session_prefix, &mut errors, &mut warnings);

            total_messages += session.messages.len();
        }

        // Global validations
        if total_messages == 0 {
            warnings.push("No messages found across all sessions".to_string());
        }

        // Check for duplicate session titles
        let mut title_counts = std::collections::HashMap::new();
        for session in sessions {
            *title_counts.entry(&session.title).or_insert(0) += 1;
        }

        for (title, count) in title_counts {
            if count > 1 {
                warnings.push(format!("Duplicate session title found: '{}' ({} times)", title, count));
            }
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            session_count: sessions.len(),
            message_count: total_messages,
        }
    }

    /// Validate a collection of sessions (without accessing messages)
    pub fn validate_sessions(&self, sessions: &[ChatSession]) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if sessions.is_empty() {
            errors.push("No sessions found to validate".to_string());
            return ValidationResult {
                is_valid: false,
                errors,
                warnings,
                session_count: 0,
                message_count: 0,
            };
        }

        let mut total_message_count = 0;
        for (session_index, session) in sessions.iter().enumerate() {
            let session_prefix = format!("Session {} ('{}')", session_index + 1, session.title);

            // Validate session structure (without messages access)
            self.validate_session_structure_basic(session, &session_prefix, &mut errors, &mut warnings);

            total_message_count += session.message_count as usize;
        }

        // Global validations
        if total_message_count == 0 {
            warnings.push("No messages found across all sessions".to_string());
        }

        // Check for duplicate session titles
        let mut title_counts = std::collections::HashMap::new();
        for session in sessions {
            *title_counts.entry(&session.title).or_insert(0) += 1;
        }

        for (title, count) in title_counts {
            if count > 1 {
                warnings.push(format!("Duplicate session title found: '{}' ({} times)", title, count));
            }
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            session_count: sessions.len(),
            message_count: total_message_count,
        }
    }

    /// Validate a single session structure (basic, without message access)
    fn validate_session_structure_basic(&self, session: &ChatSession, prefix: &str, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
        // Validate title
        if session.title.trim().is_empty() {
            errors.push(format!("{}: Session title is empty", prefix));
        } else if session.title.len() > self.max_title_length {
            errors.push(format!("{}: Session title exceeds maximum length ({} > {})",
                prefix, session.title.len(), self.max_title_length));
        }

        // Validate timestamps
        if session.created_at > session.updated_at {
            warnings.push(format!("{}: Created date is after updated date", prefix));
        }

        if session.created_at > session.last_activity {
            warnings.push(format!("{}: Created date is after last activity", prefix));
        }

        // Validate model
        if session.model.trim().is_empty() {
            warnings.push(format!("{}: Model is not specified", prefix));
        }

        // Check for too many messages (based on metadata)
        if session.message_count as usize > self.max_messages_per_session {
            warnings.push(format!("{}: Session has {} messages, which exceeds recommended limit of {}",
                prefix, session.message_count, self.max_messages_per_session));
        }
    }

    /// Validate a single session structure with messages
    fn validate_session_structure_with_messages(&self, session: &ChatSessionWithMessages, prefix: &str, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
        // Validate title
        if session.title.trim().is_empty() {
            errors.push(format!("{}: Session title is empty", prefix));
        } else if session.title.len() > self.max_title_length {
            errors.push(format!("{}: Session title exceeds maximum length ({} > {})", 
                prefix, session.title.len(), self.max_title_length));
        }

        // Validate timestamps
        if session.created_at > session.updated_at {
            warnings.push(format!("{}: Created date is after updated date", prefix));
        }

        if session.created_at > session.last_activity {
            warnings.push(format!("{}: Created date is after last activity", prefix));
        }

        // Validate message count consistency
        if session.message_count as usize != session.messages.len() {
            warnings.push(format!("{}: Message count mismatch (metadata: {}, actual: {})", 
                prefix, session.message_count, session.messages.len()));
        }

        // Validate model
        if session.model.trim().is_empty() {
            warnings.push(format!("{}: Model is not specified", prefix));
        }

        // Check for too many messages
        if session.messages.len() > self.max_messages_per_session {
            warnings.push(format!("{}: Session has {} messages, which exceeds recommended limit of {}", 
                prefix, session.messages.len(), self.max_messages_per_session));
        }

        // Validate token usage consistency
        let calculated_tokens: u32 = session.messages.iter()
            .filter_map(|msg| msg.token_usage.as_ref())
            .map(|usage| usage.total_tokens)
            .sum();
        
        if calculated_tokens > 0 && session.total_tokens_used.total_tokens != calculated_tokens {
            warnings.push(format!("{}: Total token usage mismatch (session: {}, calculated: {})", 
                prefix, session.total_tokens_used.total_tokens, calculated_tokens));
        }
    }

    /// Validate messages within a session
    fn validate_session_messages(&self, session: &ChatSessionWithMessages, prefix: &str, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
        let mut message_ids = std::collections::HashSet::new();
        let mut conversation_flow_issues = Vec::new();
        let mut last_timestamp = None;

        for (msg_index, message) in session.messages.iter().enumerate() {
            let msg_prefix = format!("{}, Message {}", prefix, msg_index + 1);
            
            // Check for duplicate message IDs
            if !message_ids.insert(message.id) {
                errors.push(format!("{}: Duplicate message ID found", msg_prefix));
            }

            // Validate message content
            if message.content.trim().is_empty() {
                warnings.push(format!("{}: Message has empty content", msg_prefix));
            } else if message.content.len() > self.max_message_length {
                errors.push(format!("{}: Message content exceeds maximum length ({} > {})", 
                    msg_prefix, message.content.len(), self.max_message_length));
            }

            // Validate message role
            match message.role {
                MessageRole::User | MessageRole::Assistant | MessageRole::System => {
                    // Valid roles
                }
            }

            // Check timestamp ordering (warning only, as some imports may have out-of-order timestamps)
            if let Some(last_ts) = last_timestamp {
                if message.timestamp < last_ts {
                    warnings.push(format!("{}: Message timestamp is earlier than previous message", msg_prefix));
                }
            }
            last_timestamp = Some(message.timestamp);

            // Validate edit timestamp
            if let Some(edited_at) = message.edited_at {
                if edited_at < message.timestamp {
                    warnings.push(format!("{}: Edit timestamp is before creation timestamp", msg_prefix));
                }
            }

            // Validate token usage
            if let Some(token_usage) = &message.token_usage {
                if token_usage.total_tokens != token_usage.input_tokens + token_usage.output_tokens {
                    warnings.push(format!("{}: Token usage calculation is incorrect", msg_prefix));
                }
                
                if token_usage.total_tokens == 0 {
                    warnings.push(format!("{}: Token usage is zero", msg_prefix));
                }
            }

            // Validate metadata
            if message.metadata.model_used.trim().is_empty() {
                warnings.push(format!("{}: Model used is not specified", msg_prefix));
            }

            if message.metadata.temperature < 0.0 || message.metadata.temperature > 2.0 {
                warnings.push(format!("{}: Temperature value is outside normal range ({})", 
                    msg_prefix, message.metadata.temperature));
            }

            // Check conversation flow
            if msg_index > 0 {
                let prev_message = &session.messages[msg_index - 1];
                if message.role == prev_message.role && message.role != MessageRole::System {
                    conversation_flow_issues.push(format!("Messages {} and {} have the same role ({})", 
                        msg_index, msg_index + 1, format!("{:?}", message.role)));
                }
            }

            // Validate threading relationships
            if let Some(parent_id) = message.parent_id {
                let parent_exists = session.messages.iter().any(|m| m.id == parent_id);
                if !parent_exists {
                    errors.push(format!("{}: References non-existent parent message", msg_prefix));
                }
            }

            // Check for circular references in children
            if !message.children.is_empty() {
                for child_id in &message.children {
                    if *child_id == message.id {
                        errors.push(format!("{}: Message references itself as a child", msg_prefix));
                    }
                }
            }
        }

        // Report conversation flow issues as warnings
        if !conversation_flow_issues.is_empty() {
            warnings.push(format!("{}: Conversation flow issues: {}", 
                prefix, conversation_flow_issues.join("; ")));
        }

        // Check for orphaned messages (messages with parent_id but parent doesn't list them as children)
        for message in &session.messages {
            if let Some(parent_id) = message.parent_id {
                if let Some(parent) = session.messages.iter().find(|m| m.id == parent_id) {
                    if !parent.children.contains(&message.id) {
                        warnings.push(format!("{}: Message {} is not listed as child of its parent", 
                            prefix, message.id));
                    }
                }
            }
        }
    }

    /// Validate a single message
    pub fn validate_message(&self, message: &Message) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if message.content.trim().is_empty() {
            warnings.push("Message content is empty".to_string());
        }

        if message.content.len() > self.max_message_length {
            errors.push(format!("Message content exceeds maximum length ({} > {})", 
                message.content.len(), self.max_message_length));
        }

        if message.metadata.model_used.trim().is_empty() {
            warnings.push("Model used is not specified".to_string());
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            session_count: 0,
            message_count: 1,
        }
    }

    /// Check if content appears to be valid text
    pub fn validate_content_quality(&self, content: &str) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check for suspicious patterns
        if content.chars().filter(|c| c.is_control() && *c != '\n' && *c != '\t').count() > 0 {
            warnings.push("Content contains control characters".to_string());
        }

        // Check for very long lines (might indicate formatting issues)
        if content.lines().any(|line| line.len() > 1000) {
            warnings.push("Content contains very long lines".to_string());
        }

        // Check for repeated characters (might indicate corruption)
        let mut char_counts = std::collections::HashMap::new();
        for ch in content.chars() {
            *char_counts.entry(ch).or_insert(0) += 1;
        }
        
        let total_chars = content.len();
        for (ch, count) in char_counts {
            if count > total_chars / 2 && total_chars > 100 {
                warnings.push(format!("Content is dominated by repeated character: '{}'", ch));
                break;
            }
        }

        warnings
    }

    /// Validate import file before processing
    pub fn validate_import_file(&self, file_path: &std::path::Path) -> Result<(), crate::EnhancedError> {
        if !file_path.exists() {
            return Err(crate::EnhancedError::storage("Import file does not exist".to_string()));
        }

        if !file_path.is_file() {
            return Err(crate::EnhancedError::storage("Import path is not a file".to_string()));
        }

        // Check file size (warn if very large)
        if let Ok(metadata) = std::fs::metadata(file_path) {
            let size_mb = metadata.len() / (1024 * 1024);
            if size_mb > 100 {
                return Err(crate::EnhancedError::storage(format!("Import file is very large ({} MB). Consider splitting into smaller files.", size_mb)));
            }
        }

        Ok(())
    }
}

impl Default for DataValidator {
    fn default() -> Self {
        Self::new()
    }
}