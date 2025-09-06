use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::models::TokenUsage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Local>,
    pub messages: Vec<Message>,
    pub model: String,
    pub total_tokens_used: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Local>,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl ChatSession {
    pub fn new(title: String, model: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            created_at: Local::now(),
            messages: Vec::new(),
            model,
            total_tokens_used: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
        }
    }
    
    pub fn add_message(&mut self, role: MessageRole, content: String, token_usage: Option<TokenUsage>) {
        let message = Message {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Local::now(),
            token_usage: token_usage.clone(),
        };
        
        // Update total token usage
        if let Some(usage) = &token_usage {
            self.total_tokens_used.input_tokens += usage.input_tokens;
            self.total_tokens_used.output_tokens += usage.output_tokens;
            self.total_tokens_used.total_tokens += usage.total_tokens;
        }
        
        self.messages.push(message);
    }
    
    pub fn get_conversation_history(&self) -> Vec<(String, String)> {
        self.messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::System => "system".to_string(),
                };
                (role, msg.content.clone())
            })
            .collect()
    }
    
    pub fn get_last_n_messages(&self, n: usize) -> Vec<&Message> {
        let start = if self.messages.len() > n {
            self.messages.len() - n
        } else {
            0
        };
        self.messages[start..].iter().collect()
    }
    
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.total_tokens_used = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
        };
    }
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant", 
            MessageRole::System => "system",
        }
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}