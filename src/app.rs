use anyhow::Result;
use crossterm::event::{self, Event};
use std::time::{Duration, Instant};

use std::sync::Arc;
use colored::Colorize;

use crate::{
    api::{APIClient, ChatMessage},

    config::Config,
    models::ModelRegistry,
    ui::{UIAction, UI},
    plugin::PluginManager,
    events::{EventBus, SessionId},
    session::manager::SessionManager,
    message::manager::MessageManager,
    export::service::ExportService,
    config::service::ConfigurationService,
    search::{SearchIndex, IndexManager},
    templates::TemplateManager,
    ui::enhanced::{
        LayoutManager, CommandPalette, HelpSystem, ThemeService,
        MarkdownRenderer, SyntaxHighlighter, ClipboardManager,
        NavigationManager, AccessibilityManager
    },
    RuffError,
};

pub struct App {
    // Core UI and rendering
    ui: UI,
    layout_manager: LayoutManager,
    
    // Session and message management
    session_manager: SessionManager,
    message_manager: MessageManager,
    current_session_id: Option<SessionId>,
    
    // Configuration and services
    config: Config,
    configuration_service: ConfigurationService,
    model_registry: ModelRegistry,
    api_client: APIClient,
    current_model_key: String,
    
    // Enhanced UI components
    command_palette: CommandPalette,
    help_system: HelpSystem,
    theme_service: ThemeService,
    markdown_renderer: MarkdownRenderer,
    syntax_highlighter: SyntaxHighlighter,
    clipboard_manager: ClipboardManager,
    navigation_manager: NavigationManager,
    accessibility_manager: AccessibilityManager,
    
    // Search and indexing
    search_index: SearchIndex,
    index_manager: IndexManager,
    
    // Export/import and templates
    export_service: ExportService,
    template_manager: TemplateManager,
    
    // Plugin system
    plugin_manager: Option<PluginManager>,
    
    // Event system
    event_bus: Arc<EventBus>,
}

impl App {
    pub async fn new() -> Result<Self, RuffError> {
        let config = Config::load().map_err(|e| {
            eprintln!("{}", "⚠️  Configuration not found. Run 'ruff --init' to initialize.".bright_yellow());
            e
        })?;
        
        // Initialize event bus first (needed by many components)
        let event_bus = Arc::new(EventBus::new());
        
        // Initialize configuration service
        let configuration_service = ConfigurationService::new()?;
        configuration_service.initialize().await?;
        
        // Initialize core services
        let model_registry = ModelRegistry::new();
        let api_client = APIClient::new();
        
        // Initialize storage paths
        let app_data_dir = dirs::data_dir()
            .ok_or_else(|| RuffError::App("Could not find data directory".to_string()))?
            .join("ruff");
        
        let sessions_dir = app_data_dir.join("sessions");
        let exports_dir = app_data_dir.join("exports");
        let templates_dir = app_data_dir.join("templates");
        let plugins_dir = dirs::config_dir()
            .ok_or_else(|| RuffError::App("Could not find config directory".to_string()))?
            .join("ruff")
            .join("plugins");
        
        // Create directories
        std::fs::create_dir_all(&sessions_dir)?;
        std::fs::create_dir_all(&exports_dir)?;
        std::fs::create_dir_all(&templates_dir)?;
        std::fs::create_dir_all(&plugins_dir)?;
        
        // Initialize session and message managers
        let session_event_bus = EventBus::new();
        let message_event_bus = EventBus::new();
        
        let mut session_manager = SessionManager::new(session_event_bus, sessions_dir);
        session_manager.initialize().await?;
        
        let message_manager = MessageManager::new(message_event_bus);
        
        // Initialize enhanced UI components
        let layout_manager = LayoutManager::new();
        let command_palette = CommandPalette::new();
        let help_system = HelpSystem::new();
        let theme_service = ThemeService::new();
        let markdown_renderer = MarkdownRenderer::new();
        let syntax_highlighter = SyntaxHighlighter::new();
        let clipboard_manager = ClipboardManager::new().map_err(|e| RuffError::App(format!("Failed to initialize clipboard: {}", e)))?;
        let navigation_manager = NavigationManager::new();
        let accessibility_manager = AccessibilityManager::new();
        
        // Initialize search components
        let search_index = SearchIndex::new();
        let index_manager = IndexManager::new();
        
        // Initialize export service
        let export_service = ExportService::new(exports_dir);
        export_service.initialize()?;
        
        // Initialize template manager
        let template_manager = TemplateManager::new(templates_dir, event_bus.clone())?;
        template_manager.load_templates().await?;
        
        // Initialize plugin manager
        let plugin_manager = PluginManager::new(event_bus.clone(), plugins_dir);
        
        // Initialize main UI with enhanced components
        let mut ui = UI::new(config.clone())?;
        
        // Set up UI extension manager from plugin system
        let ui_extension_manager = plugin_manager.get_ui_extension_manager();
        ui.set_ui_extension_manager(ui_extension_manager);
        
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
        
        // Get or create the active session
        let current_session_id = if session_manager.session_count() > 0 {
            // Get the most recently active session
            let sessions = session_manager.get_all_sessions();
            let most_recent = sessions.iter()
                .max_by_key(|s| s.last_activity)
                .map(|s| s.id);
            
            if let Some(session_id) = most_recent {
                session_manager.switch_session(session_id).await?;
                println!("{}", format!("🔄 Resuming last chat session").bright_blue());
                Some(session_id)
            } else {
                None
            }
        } else {
            // Create a new session
            let session_id = session_manager.create_session(Some("New Chat".to_string())).await?;
            session_manager.switch_session(session_id).await?;
            Some(session_id)
        };

        Ok(Self {
            ui,
            layout_manager,
            session_manager,
            message_manager,
            current_session_id,
            config,
            configuration_service,
            model_registry,
            api_client,
            current_model_key,
            command_palette,
            help_system,
            theme_service,
            markdown_renderer,
            syntax_highlighter,
            clipboard_manager,
            navigation_manager,
            accessibility_manager,
            search_index,
            index_manager,
            export_service,
            template_manager,
            plugin_manager: Some(plugin_manager),
            event_bus,
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        println!("{}", "🦀 Welcome to Ruff - Enhanced AI Chat Terminal".bright_red());
        println!("{}", "Press F1 for help, Ctrl+Shift+P for command palette, Ctrl+C to quit".bright_cyan());
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
                
                // Get current session for rendering
                let current_session = if let Some(session_id) = self.current_session_id {
                    self.session_manager.get_session(session_id)
                } else {
                    None
                };
                
                // Render with enhanced components
                if let Some(session) = current_session {
                    self.ui.render_enhanced(
                        session,
                        current_model,
                        &self.layout_manager,
                        &self.theme_service,
                        &self.markdown_renderer,
                        &self.syntax_highlighter,
                        &self.command_palette,
                        &self.help_system,
                    )?;
                } else {
                    // Render empty state
                    self.ui.render_empty_state(current_model)?;
                }
                
                last_render = Instant::now();
            }
            
            // Handle events
            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    // First, let enhanced components handle the key event
                    let action = if self.command_palette.is_visible() {
                        self.command_palette.handle_key_event(key)
                    } else if self.help_system.is_visible() {
                        self.help_system.handle_key_event(key)
                    } else {
                        // Handle navigation shortcuts
                        match self.navigation_manager.handle_key_event(key) {
                            Some(nav_action) => {
                                self.handle_navigation_action(nav_action).await?;
                                continue;
                            }
                            None => {
                                // Let the main UI handle the event
                                self.ui.handle_key_event(key)
                            }
                        }
                    };
                    
                    match action {
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
                        UIAction::FontSizeChanged(_size) => {
                            // Font size change is already handled in the UI
                            continue;
                        }
                        UIAction::ShowCommandPalette => {
                            self.command_palette.show();
                        }
                        UIAction::ShowHelp => {
                            self.help_system.show();
                        }
                        UIAction::CreateNewSession => {
                            if let Err(e) = self.handle_create_new_session().await {
                                self.ui.show_error(&format!("Failed to create new session: {}", e))?;
                            }
                        }
                        UIAction::SwitchSession(session_id) => {
                            if let Err(e) = self.handle_switch_session(session_id).await {
                                self.ui.show_error(&format!("Failed to switch session: {}", e))?;
                            }
                        }
                        UIAction::ExportSession(format) => {
                            if let Err(e) = self.handle_export_session(format).await {
                                self.ui.show_error(&format!("Failed to export session: {}", e))?;
                            }
                        }
                    }
                }
            }
        }
        
        // Save all sessions before exiting
        if let Err(e) = self.session_manager.save_all_sessions().await {
            eprintln!("Warning: Failed to save sessions: {}", e);
        }
        
        println!("{}", "\n💾 Sessions saved.".bright_yellow());
        println!("{}", "\n👋 Thanks for using Ruff!".bright_green());
        Ok(())
    }
    
    async fn handle_send_message(&mut self, message: String) -> Result<(), RuffError> {
        let session_id = self.current_session_id
            .ok_or_else(|| RuffError::App("No active session".to_string()))?;
        
        // Create user message
        let user_message = crate::session::manager::Message {
            id: uuid::Uuid::new_v4(),
            role: crate::session::manager::MessageRole::User,
            content: message.clone(),
            timestamp: chrono::Local::now(),
            edited_at: None,
            token_usage: None,
            parent_id: None,
            children: Vec::new(),
            metadata: crate::session::manager::MessageMetadata {
                model_used: self.current_model_key.clone(),
                temperature: self.config.temperature,
                response_time_ms: 0,
                is_regenerated: false,
                regeneration_count: 0,
            },
        };
        
        // Add user message to session
        let user_message_id = self.message_manager.add_message(session_id, user_message).await?;
        
        // Get current model
        let current_model = self.model_registry
            .get_model(&self.current_model_key)
            .ok_or_else(|| RuffError::UnsupportedModel { 
                model: self.current_model_key.clone() 
            })?;
        
        // Get API key from configuration service
        let api_key = self.configuration_service.get_api_key(&current_model.provider)?;
        
        // Get model configuration
        let model_config = self.configuration_service
            .get_model_config(&self.current_model_key)
            .unwrap_or_default();
        
        // Prepare messages for API
        let mut api_messages = Vec::new();
        
        // Add system message if model supports it and session has system prompt
        if current_model.supports_system {
            let session = self.session_manager.get_session(session_id)
                .ok_or_else(|| RuffError::App("Session not found".to_string()))?;
            
            let system_prompt = session.system_prompt.as_deref()
                .unwrap_or("You are a helpful AI assistant. Provide clear, accurate, and helpful responses.");
            
            api_messages.push(ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            });
        }
        
        // Get conversation history from message manager
        let session_messages = self.message_manager.get_session_messages(session_id);
        let conversation_messages: Vec<_> = session_messages.into_iter()
            .rev().take(10).collect::<Vec<_>>().into_iter().rev()
            .map(|msg| ChatMessage {
                role: match msg.role {
                    crate::session::manager::MessageRole::User => "user".to_string(),
                    crate::session::manager::MessageRole::Assistant => "assistant".to_string(),
                    crate::session::manager::MessageRole::System => "system".to_string(),
                },
                content: msg.content.clone(),
            })
            .collect();
        
        api_messages.extend(conversation_messages);
        
        // Check token limits (rough estimation)
        let estimated_input_tokens = self.estimate_tokens(&api_messages);
        if estimated_input_tokens > current_model.max_tokens {
            return Err(RuffError::TokenLimit {
                input_tokens: estimated_input_tokens,
                output_tokens: 0,
                max_tokens: current_model.max_tokens,
            });
        }
        
        // Check rate limits
        if !self.configuration_service.can_make_request(&current_model.provider, Some(estimated_input_tokens)) {
            self.configuration_service.wait_for_request(&current_model.provider, Some(estimated_input_tokens)).await?;
        }
        
        // Show loading indicator
        self.ui.show_loading("Sending message...")?;
        
        // Send request
        let start_time = Instant::now();
        match self.api_client.send_message(
            current_model,
            api_messages,
            &api_key,
            model_config.max_tokens.min(current_model.max_tokens - estimated_input_tokens),
            model_config.temperature,
        ).await {
            Ok((response, token_usage)) => {
                let duration = start_time.elapsed();
                
                // Create assistant message
                let assistant_message = crate::session::manager::Message {
                    id: uuid::Uuid::new_v4(),
                    role: crate::session::manager::MessageRole::Assistant,
                    content: response,
                    timestamp: chrono::Local::now(),
                    edited_at: None,
                    token_usage: Some(token_usage.clone()),
                    parent_id: Some(user_message_id),
                    children: Vec::new(),
                    metadata: crate::session::manager::MessageMetadata {
                        model_used: self.current_model_key.clone(),
                        temperature: model_config.temperature,
                        response_time_ms: duration.as_millis() as u64,
                        is_regenerated: false,
                        regeneration_count: 0,
                    },
                };
                
                // Add assistant response
                self.message_manager.add_message(session_id, assistant_message).await?;
                
                // Update session metadata
                self.session_manager.update_session_metadata(session_id).await?;
                
                // Update search index
                self.session_manager.update_session_index(session_id);
                
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
                let error_message = crate::session::manager::Message {
                    id: uuid::Uuid::new_v4(),
                    role: crate::session::manager::MessageRole::System,
                    content: format!("Error: {}", e),
                    timestamp: chrono::Local::now(),
                    edited_at: None,
                    token_usage: None,
                    parent_id: None,
                    children: Vec::new(),
                    metadata: crate::session::manager::MessageMetadata {
                        model_used: self.current_model_key.clone(),
                        temperature: 0.0,
                        response_time_ms: 0,
                        is_regenerated: false,
                        regeneration_count: 0,
                    },
                };
                
                self.message_manager.add_message(session_id, error_message).await?;
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
        if let Err(_) = self.configuration_service.get_api_key(&model.provider) {
            return Err(RuffError::InvalidApiKey { 
                model: model.provider.clone() 
            });
        }
        
        // Switch model
        self.current_model_key = model_key.clone();
        
        // Update current session's model if there is one
        if let Some(session_id) = self.current_session_id {
            if let Some(session) = self.session_manager.get_session_mut(session_id) {
                session.model = model_key.clone();
                session.updated_at = chrono::Local::now();
            }
            
            // Add system message about model switch
            let system_message = crate::session::manager::Message {
                id: uuid::Uuid::new_v4(),
                role: crate::session::manager::MessageRole::System,
                content: format!("Switched to model: {} ({})", model.name, model.provider),
                timestamp: chrono::Local::now(),
                edited_at: None,
                token_usage: None,
                parent_id: None,
                children: Vec::new(),
                metadata: crate::session::manager::MessageMetadata {
                    model_used: model_key.clone(),
                    temperature: 0.0,
                    response_time_ms: 0,
                    is_regenerated: false,
                    regeneration_count: 0,
                },
            };
            
            self.message_manager.add_message(session_id, system_message).await?;
            
            // Save the session
            self.session_manager.save_session(session_id).await?;
        }
        
        println!("{}", format!("🔄 Switched to model: {} ({})", model.name, model.provider).bright_blue());
        
        Ok(())
    }
    
    async fn handle_create_new_session(&mut self) -> Result<(), RuffError> {
        let session_id = self.session_manager.create_session(Some("New Chat".to_string())).await?;
        self.session_manager.switch_session(session_id).await?;
        self.current_session_id = Some(session_id);
        
        println!("{}", "📝 Created new chat session".bright_green());
        Ok(())
    }
    
    async fn handle_switch_session(&mut self, session_id: crate::events::SessionId) -> Result<(), RuffError> {
        self.session_manager.switch_session(session_id).await?;
        self.current_session_id = Some(session_id);
        
        if let Some(session) = self.session_manager.get_session(session_id) {
            println!("{}", format!("🔄 Switched to session: {}", session.title).bright_blue());
        }
        
        Ok(())
    }
    
    async fn handle_export_session(&mut self, format: crate::export::formats::ExportFormat) -> Result<(), RuffError> {
        let session_id = self.current_session_id
            .ok_or_else(|| RuffError::App("No active session to export".to_string()))?;
        
        let session = self.session_manager.get_session(session_id)
            .ok_or_else(|| RuffError::App("Session not found".to_string()))?;
        
        let request = crate::export::service::SessionExportRequest {
            session_id,
            format,
            options: crate::export::formats::ExportOptions::default(),
            output_path: None,
        };
        
        let result = self.export_service.export_session(session, request)?;
        
        println!("{}", format!("📄 Session exported to: {}", result.file_path).bright_green());
        Ok(())
    }
    
    async fn handle_navigation_action(&mut self, action: crate::ui::enhanced::NavigationAction) -> Result<(), RuffError> {
        use crate::ui::enhanced::NavigationAction;
        
        match action {
            NavigationAction::NextSession => {
                let sessions = self.session_manager.get_session_tabs();
                if let Some(current_id) = self.current_session_id {
                    if let Some(current_index) = sessions.iter().position(|tab| tab.id == current_id) {
                        let next_index = (current_index + 1) % sessions.len();
                        if let Some(next_session) = sessions.get(next_index) {
                            self.handle_switch_session(next_session.id).await?;
                        }
                    }
                }
            }
            NavigationAction::PreviousSession => {
                let sessions = self.session_manager.get_session_tabs();
                if let Some(current_id) = self.current_session_id {
                    if let Some(current_index) = sessions.iter().position(|tab| tab.id == current_id) {
                        let prev_index = if current_index == 0 { sessions.len() - 1 } else { current_index - 1 };
                        if let Some(prev_session) = sessions.get(prev_index) {
                            self.handle_switch_session(prev_session.id).await?;
                        }
                    }
                }
            }
            NavigationAction::GoToMessage(message_number) => {
                // This would be handled by the UI layer to scroll to a specific message
                self.ui.scroll_to_message(message_number)?;
            }
            NavigationAction::ScrollToTop => {
                self.ui.scroll_to_top()?;
            }
            NavigationAction::ScrollToBottom => {
                self.ui.scroll_to_bottom()?;
            }
            NavigationAction::SessionChanged(_) => {
                // This is handled by the session manager
            }
            NavigationAction::MessageJumped(_, _) => {
                // This is handled by the UI layer
            }
            NavigationAction::ScrollChanged(_) => {
                // This is handled by the UI layer
            }
            NavigationAction::FirstMessage(_) => {
                // This is handled by the UI layer
            }
            NavigationAction::LastMessage(_) => {
                // This is handled by the UI layer
            }
        }
        
        Ok(())
    }
    
    fn estimate_tokens(&self, messages: &[ChatMessage]) -> u32 {
        // Rough estimation: ~4 characters per token
        let total_chars: usize = messages.iter()
            .map(|msg| msg.content.len() + msg.role.len())
            .sum();
        (total_chars / 4).max(1) as u32
    }

    /// Get the plugin manager
    pub fn get_plugin_manager(&self) -> Option<&PluginManager> {
        self.plugin_manager.as_ref()
    }

    /// Get the event bus
    pub fn get_event_bus(&self) -> &Arc<EventBus> {
        &self.event_bus
    }
    
    /// Get the session manager
    pub fn get_session_manager(&self) -> &SessionManager {
        &self.session_manager
    }
    
    /// Get the session manager mutably
    pub fn get_session_manager_mut(&mut self) -> &mut SessionManager {
        &mut self.session_manager
    }
    
    /// Get the message manager
    pub fn get_message_manager(&self) -> &MessageManager {
        &self.message_manager
    }
    
    /// Get the message manager mutably
    pub fn get_message_manager_mut(&mut self) -> &mut MessageManager {
        &mut self.message_manager
    }
    
    /// Get the configuration service
    pub fn get_configuration_service(&self) -> &ConfigurationService {
        &self.configuration_service
    }
    
    /// Get the export service
    pub fn get_export_service(&self) -> &ExportService {
        &self.export_service
    }
    
    /// Get the template manager
    pub fn get_template_manager(&self) -> &TemplateManager {
        &self.template_manager
    }
    
    /// Get the search index
    pub fn get_search_index(&self) -> &SearchIndex {
        &self.search_index
    }
    
    /// Get the theme service
    pub fn get_theme_service(&self) -> &ThemeService {
        &self.theme_service
    }
    
    /// Get the current session ID
    pub fn get_current_session_id(&self) -> Option<SessionId> {
        self.current_session_id
    }
    
    /// Get the current session
    pub fn get_current_session(&self) -> Option<&crate::session::manager::ChatSession> {
        self.current_session_id.and_then(|id| self.session_manager.get_session(id))
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Cleanup will be handled by UI's Drop implementation
    }
}

#[cfg(test)]
mod app_integration_tests;