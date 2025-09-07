use anyhow::Result;
use crossterm::event::{self, Event};
use std::time::{Duration, Instant};
use colored::Colorize;

use crate::{
    api::{APIClient, ChatMessage},
    chat::{ChatSession, MessageRole},
    config::Config,
    models::ModelRegistry,
    ui::{UIAction, UI},
    RuffError,
};

pub struct App {
    ui: UI,
    session: ChatSession,
    config: Config,
    model_registry: ModelRegistry,
    api_client: APIClient,
    current_model_key: String,
}

impl App {
    pub async fn new() -> Result<Self, RuffError> {
        let config = Config::load().map_err(|e| {
            eprintln!("{}", "⚠️  Configuration not found. Run 'ruff --init' to initialize.".bright_yellow());
            e
        })?;
        
        let model_registry = ModelRegistry::new();
        let ui = UI::new(config.clone())?;
        let api_client = APIClient::new();
        
        // Validate default model exists
        let current_model_key = if model_registry.get_model(&config.default_model).is_some() {
            config.default_model.clone()
        } else {
            let available_models = model_registry.list_models();
            if let Some((first_key, _)) = available_models.first() {
                println!("{}", format!("⚠️  Default model '{}' not found. Using '{}'", 
                    config.default_model, first_key).bright_yellow());
                first_key.to_string()
            } else {
                return Err(RuffError::App("No models available".to_string()));
            }
        };
        
        // Load the last session or create a new one
        let sessions = ChatSession::list_sessions()?;
        let session = if let Some(last_session) = sessions.into_iter().next() {
            println!("{}", format!("🔄 Resuming last chat session: {}", last_session.title).bright_blue());
            last_session
        } else {
            ChatSession::new("New Chat".to_string(), current_model_key.clone())
        };

        Ok(Self {
            ui,
            session,
            config,
            model_registry,
            api_client,
            current_model_key,
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        println!("{}", "🦀 Welcome to Ruff - AI Chat Terminal".bright_red());
        println!("{}", "Press Ctrl+M to switch models, Ctrl+C to quit".bright_cyan());
        std::thread::sleep(Duration::from_millis(1000));
        
        let mut last_render = Instant::now();
        
        loop {
            // Render UI at 60 FPS max
            if last_render.elapsed() >= Duration::from_millis(16) {
                let current_model = self.model_registry
                    .get_model(&self.current_model_key)
                    .ok_or_else(|| RuffError::UnsupportedModel { 
                        model: self.current_model_key.clone() 
                    })?;
                    
                self.ui.render(&self.session, current_model)?;
                last_render = Instant::now();
            }
            
            // Handle events
            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    match self.ui.handle_key_event(key) {
                        UIAction::None => continue,
                        UIAction::Quit => break,
                        UIAction::SendMessage(message) => {
                            if let Err(e) = self.handle_send_message(message).await {
                                self.ui.show_error(&format!("Failed to send message: {}", e))?;
                            }
                        }
                        UIAction::SelectModel(model_key) => {
                            if let Err(e) = self.handle_select_model(model_key).await {
                                self.ui.show_error(&format!("Failed to select model: {}", e))?;
                            }
                        }
                    }
                }
            }
        }
        
        // Save the session before exiting
        self.session.save()?;
        println!("{}", "\n💾 Session saved.".bright_yellow());
        println!("{}", "\n👋 Thanks for using Ruff!".bright_green());
        Ok(())
    }
    
    async fn handle_send_message(&mut self, message: String) -> Result<(), RuffError> {
        // Add user message
        self.session.add_message(MessageRole::User, message.clone(), None);
        
        // Get current model
        let current_model = self.model_registry
            .get_model(&self.current_model_key)
            .ok_or_else(|| RuffError::UnsupportedModel { 
                model: self.current_model_key.clone() 
            })?;
        
        // Get API key
        let api_key = self.config.get_api_key(&current_model.provider)?;
        
        // Prepare messages for API
        let mut api_messages = Vec::new();
        
        // Add system message if model supports it
        if current_model.supports_system {
            api_messages.push(ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful AI assistant. Provide clear, accurate, and helpful responses.".to_string(),
            });
        }
        
        // Add conversation history (last 10 messages to stay within token limits)
        for (role, content) in self.session.get_conversation_history().into_iter().rev().take(10).collect::<Vec<_>>().into_iter().rev() {
            api_messages.push(ChatMessage { role, content });
        }
        
        // Check token limits (rough estimation)
        let estimated_input_tokens = self.estimate_tokens(&api_messages);
        if estimated_input_tokens > current_model.max_tokens {
            return Err(RuffError::TokenLimit {
                input_tokens: estimated_input_tokens,
                output_tokens: 0,
                max_tokens: current_model.max_tokens,
            });
        }
        
        // Show loading indicator
        self.ui.show_loading("Sending message...")?;
        
        // Send request
        let start_time = Instant::now();
        match self.api_client.send_message(
            current_model,
            api_messages,
            api_key,
            self.config.max_tokens.min(current_model.max_tokens - estimated_input_tokens),
            self.config.temperature,
        ).await {
            Ok((response, token_usage)) => {
                let duration = start_time.elapsed();
                
                // Add assistant response
                self.session.add_message(MessageRole::Assistant, response, Some(token_usage.clone()));
                
                // Show success info
                println!("{}", format!("✅ Response received in {:.2}s | Tokens: In:{} Out:{} Total:{}", 
                    duration.as_secs_f64(),
                    token_usage.input_tokens,
                    token_usage.output_tokens,
                    token_usage.total_tokens
                ).bright_green());
            }
            Err(e) => {
                // Add error message as system message
                let error_msg = format!("Error: {}", e);
                self.session.add_message(MessageRole::System, error_msg, None);
                return Err(e);
            }
        }
        
        Ok(())
    }
    
    async fn handle_select_model(&mut self, model_key: String) -> Result<(), RuffError> {
        // Validate model exists
        let model = self.model_registry
            .get_model(&model_key)
            .ok_or_else(|| RuffError::UnsupportedModel { 
                model: model_key.clone() 
            })?;
        
        // Check if API key is configured
        if let Err(_) = self.config.get_api_key(&model.provider) {
            return Err(RuffError::InvalidApiKey { 
                model: model.provider.clone() 
            });
        }
        
        // Switch model
        self.current_model_key = model_key.clone();
        self.session.model = model_key;
        
        // Add system message about model switch
        self.session.add_message(
            MessageRole::System, 
            format!("Switched to model: {} ({})", model.name, model.provider), 
            None
        );
        
        println!("{}", format!("🔄 Switched to model: {} ({})", model.name, model.provider).bright_blue());
        
        Ok(())
    }
    
    fn estimate_tokens(&self, messages: &[ChatMessage]) -> u32 {
        // Rough estimation: ~4 characters per token
        let total_chars: usize = messages.iter()
            .map(|msg| msg.content.len() + msg.role.len())
            .sum();
        (total_chars / 4).max(1) as u32
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Cleanup will be handled by UI's Drop implementation
    }
}