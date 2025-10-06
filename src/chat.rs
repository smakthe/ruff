use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::models::TokenUsage;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use directories::ProjectDirs;
use crate::EnhancedError;

// Helper function to get the sessions directory
fn get_sessions_dir() -> Result<PathBuf, EnhancedError> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "ruff", "ruff") {
        let data_dir = proj_dirs.data_dir();
        let sessions_dir = data_dir.join("sessions");
        fs::create_dir_all(&sessions_dir)?;
        Ok(sessions_dir)
    } else {
        Err(EnhancedError::unknown("Could not find project directories".to_string()))
    }
}

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

    pub fn save(&self) -> Result<(), EnhancedError> {
        let sessions_dir = get_sessions_dir()?;
        let file_path = sessions_dir.join(format!("{}.json", self.id));
        let mut file = File::create(file_path)?;
        let json = serde_json::to_string_pretty(self)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load(session_id: Uuid) -> Result<Self, EnhancedError> {
        let sessions_dir = get_sessions_dir()?;
        let file_path = sessions_dir.join(format!("{}.json", session_id));
        let mut file = File::open(file_path)?;
        let mut json = String::new();
        file.read_to_string(&mut json)?;
        let session = serde_json::from_str(&json)?;
        Ok(session)
    }

    pub fn list_sessions() -> Result<Vec<ChatSession>, EnhancedError> {
        let sessions_dir = get_sessions_dir()?;
        let mut sessions = Vec::new();
        for entry in fs::read_dir(sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = Uuid::parse_str(stem) {
                        if let Ok(session) = Self::load(id) {
                            sessions.push(session);
                        }
                    }
                }
            }
        }
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    pub fn delete(session_id: Uuid) -> Result<(), EnhancedError> {
        let sessions_dir = get_sessions_dir()?;
        let file_path = sessions_dir.join(format!("{}.json", session_id));
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        Ok(())
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