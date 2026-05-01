type Result<T> = std::result::Result<T, EnhancedError>;
use crossterm::event::{self, Event};
use futures_util::StreamExt;
use std::time::{Duration, Instant};

use colored::Colorize;
use std::sync::Arc;

use crate::{
    api::{APIClient, ChatMessage},
    config::service::ConfigurationService,
    config::Config,
    events::{EventBus, SessionId},
    export::service::ExportService,
    message::manager::MessageManager,
    models::{ModelRegistry, TokenUsage},
    plugin::PluginManager,
    search::TantivyMessageSearchIndex,
    session::manager::SessionManager,
    templates::TemplateManager,
    ui::enhanced::{
        AccessibilityManager, ClipboardManager, CommandPalette, HelpSystem, LayoutManager,
        MarkdownRenderer, NavigationManager, SyntaxHighlighter, ThemeService,
    },
    ui::{UIAction, UI},
    EnhancedError,
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
    #[allow(dead_code)] // Future functionality
    clipboard_manager: ClipboardManager,
    navigation_manager: NavigationManager,
    #[allow(dead_code)] // Future functionality
    accessibility_manager: AccessibilityManager,

    // Search and indexing
    search_index: Arc<TantivyMessageSearchIndex>,

    // Export/import and templates
    export_service: ExportService,
    template_manager: TemplateManager,

    // Plugin system
    plugin_manager: Option<PluginManager>,

    // Event system
    event_bus: Arc<EventBus>,
}

impl App {
    pub async fn new() -> Result<Self> {
        let config = Config::load()?;

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
            .ok_or_else(|| EnhancedError::storage("Could not find data directory"))?
            .join("ruff");

        let sessions_dir = app_data_dir.join("sessions");
        let messages_dir = app_data_dir.join("messages");
        let exports_dir = app_data_dir.join("exports");
        let templates_dir = app_data_dir.join("templates");
        let plugins_dir = dirs::config_dir()
            .ok_or_else(|| EnhancedError::config("Could not find config directory"))?
            .join("ruff")
            .join("plugins");

        // Create directories
        std::fs::create_dir_all(&sessions_dir)?;
        std::fs::create_dir_all(&messages_dir)?;
        std::fs::create_dir_all(&exports_dir)?;
        std::fs::create_dir_all(&templates_dir)?;
        std::fs::create_dir_all(&plugins_dir)?;

        // Initialize session and message managers
        let session_event_bus = EventBus::new();
        let message_event_bus = EventBus::new();

        let mut session_manager = SessionManager::new(session_event_bus, sessions_dir);
        session_manager.initialize().await?;

        let message_manager = MessageManager::new_with_storage(message_event_bus, messages_dir)?;

        // Initialize enhanced UI components
        let layout_manager = LayoutManager::new();
        let command_palette = CommandPalette::new();
        let help_system = HelpSystem::new();
        let theme_service = ThemeService::new();
        let markdown_renderer = MarkdownRenderer::new();
        let syntax_highlighter = SyntaxHighlighter::new();
        let clipboard_manager = match ClipboardManager::new() {
            Ok(manager) => manager,
            Err(e) => {
                eprintln!("Warning: Clipboard support unavailable: {}", e);
                ClipboardManager::default()
            }
        };
        let navigation_manager = NavigationManager::new();
        let accessibility_manager = AccessibilityManager::new();

        // Initialize Tantivy search backend
        let search_dir = app_data_dir.join("search_index");
        std::fs::create_dir_all(&search_dir)?;
        let search_index = Arc::new(TantivyMessageSearchIndex::new(search_dir)?);

        // Index existing messages on startup
        for session in session_manager.get_all_sessions() {
            let session_messages = message_manager.get_session_messages_owned(session.id);
            for message in session_messages {
                search_index.index_message(session.id, &message).await?;
            }
        }
        search_index.commit().await?;

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
                first_key.to_string()
            } else {
                return Err(EnhancedError::config("No models available"));
            }
        };

        // Get or create the active session
        let current_session_id = if session_manager.session_count() > 0 {
            // Get the most recently active session
            let sessions = session_manager.get_all_sessions();
            let most_recent = sessions
                .iter()
                .max_by_key(|s| s.last_activity)
                .map(|s| s.id);

            if let Some(session_id) = most_recent {
                session_manager.switch_session(session_id).await?;
                Some(session_id)
            } else {
                None
            }
        } else {
            // Create a new session
            let session_id = session_manager
                .create_session(Some("New Chat".to_string()))
                .await?;
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
            export_service,
            template_manager,
            plugin_manager: Some(plugin_manager),
            event_bus,
        })
    }

    pub fn cleanup(&mut self) -> Result<()> {
        self.ui.cleanup()
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut last_render = Instant::now();

        loop {
            // Render UI at 60 FPS max
            if last_render.elapsed() >= Duration::from_millis(16) {
                let current_model = self
                    .model_registry
                    .get_model(&self.current_model_key)
                    .ok_or_else(|| {
                        EnhancedError::config(format!(
                            "Unsupported model: {}",
                            self.current_model_key
                        ))
                    })?;

                // Get current session for rendering
                let current_session = if let Some(session_id) = self.current_session_id {
                    self.session_manager.get_session(session_id)
                } else {
                    None
                };

                // Render with enhanced components
                if let Some(session) = current_session {
                    let messages = self.message_manager.get_session_messages_owned(session.id);
                    self.ui.render_enhanced(
                        session,
                        &messages,
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
                    if let Err(e) = self.ui.render_empty_state(current_model) {
                        return Err(e);
                    }
                }

                last_render = Instant::now();
            }

            // Handle events
            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_plugin_input(key.clone()).await? {
                        continue;
                    }

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
                        UIAction::Quit => {
                            break;
                        }
                        UIAction::SendMessage(message) => {
                            if let Err(e) = self.handle_send_message(message).await {
                                self.ui
                                    .show_error(&format!("Failed to send message: {}", e))?;
                            }
                        }
                        UIAction::SelectModel(model_key) => {
                            if let Err(e) = self.handle_select_model(model_key).await {
                                self.ui
                                    .show_error(&format!("Failed to select model: {}", e))?;
                            }
                        }
                        UIAction::FontSizeChanged(_size) => {
                            // Font size change is already handled in the UI
                            continue;
                        }
                        UIAction::IncreaseFontSize => {
                            if let Err(e) = self.ui.increase_font_size() {
                                self.ui
                                    .show_error(&format!("Failed to increase font size: {}", e))?;
                            }
                        }
                        UIAction::DecreaseFontSize => {
                            if let Err(e) = self.ui.decrease_font_size() {
                                self.ui
                                    .show_error(&format!("Failed to decrease font size: {}", e))?;
                            }
                        }
                        UIAction::ResetFontSize => {
                            if let Err(e) = self.ui.reset_font_size() {
                                self.ui
                                    .show_error(&format!("Failed to reset font size: {}", e))?;
                            }
                        }
                        UIAction::ToggleTheme => {
                            if let Err(e) = self.ui.toggle_theme() {
                                self.ui
                                    .show_error(&format!("Failed to toggle theme: {}", e))?;
                            }
                        }
                        UIAction::ScrollToTop => {
                            if let Err(e) = self.ui.scroll_to_top() {
                                self.ui.show_error(&format!("Failed to scroll: {}", e))?;
                            }
                        }
                        UIAction::ScrollToBottom => {
                            if let Err(e) = self.ui.scroll_to_bottom() {
                                self.ui.show_error(&format!("Failed to scroll: {}", e))?;
                            }
                        }
                        UIAction::PreviousSession => {
                            if let Err(e) = self
                                .handle_navigation_action(
                                    crate::ui::enhanced::NavigationAction::PreviousSession,
                                )
                                .await
                            {
                                self.ui
                                    .show_error(&format!("Failed to switch session: {}", e))?;
                            }
                        }
                        UIAction::NextSession => {
                            if let Err(e) = self
                                .handle_navigation_action(
                                    crate::ui::enhanced::NavigationAction::NextSession,
                                )
                                .await
                            {
                                self.ui
                                    .show_error(&format!("Failed to switch session: {}", e))?;
                            }
                        }
                        UIAction::ShowCommandPalette => {
                            self.command_palette.show();
                        }
                        UIAction::ShowHelp => {
                            self.help_system.show();
                        }
                        UIAction::CreateNewSession => {
                            if let Err(e) = self.handle_create_new_session().await {
                                self.ui
                                    .show_error(&format!("Failed to create new session: {}", e))?;
                            }
                        }
                        UIAction::SwitchSession(session_id) => {
                            if let Err(e) = self.handle_switch_session(session_id).await {
                                self.ui
                                    .show_error(&format!("Failed to switch session: {}", e))?;
                            }
                        }
                        UIAction::ExportSession(format) => {
                            if let Err(e) = self.handle_export_session(format).await {
                                self.ui
                                    .show_error(&format!("Failed to export session: {}", e))?;
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

    async fn handle_plugin_input(&self, key: crossterm::event::KeyEvent) -> Result<bool> {
        let Some(extension_manager) = self.ui.get_ui_extension_manager().as_ref().cloned() else {
            return Ok(false);
        };

        extension_manager.handle_input(&Event::Key(key)).await
    }

    async fn handle_send_message(&mut self, message: String) -> Result<()> {
        let session_id = self
            .current_session_id
            .ok_or_else(|| EnhancedError::session("No active session"))?;

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
        let user_message_id = self
            .message_manager
            .add_message(session_id, user_message.clone())
            .await?;

        // Index the user message
        self.search_index
            .index_message(session_id, &user_message)
            .await?;

        // Get current model
        let current_model = self
            .model_registry
            .get_model(&self.current_model_key)
            .ok_or_else(|| {
                EnhancedError::config(format!("Unsupported model: {}", self.current_model_key))
            })?;

        // Get API key from configuration service
        let api_key = self
            .configuration_service
            .get_api_key(&current_model.provider)?;

        // Get model configuration
        let model_config = self
            .configuration_service
            .get_model_config(&self.current_model_key)
            .unwrap_or_default();

        // Prepare messages for API
        let mut api_messages = Vec::new();

        // Add system message if model supports it and session has system prompt
        if current_model.supports_system {
            let session = self
                .session_manager
                .get_session(session_id)
                .ok_or_else(|| {
                    EnhancedError::session(format!("Session not found: {}", session_id))
                        .with_session(session_id)
                })?;

            let system_prompt = session.system_prompt.as_deref().unwrap_or(
                "You are a helpful AI assistant. Provide clear, accurate, and helpful responses.",
            );

            api_messages.push(ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            });
        }

        // Get conversation history from message manager
        let session_messages = self.message_manager.get_session_messages(session_id);
        let conversation_messages: Vec<_> = session_messages
            .into_iter()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
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
            return Err(EnhancedError::performance(format!(
                "Token limit exceeded. Input: {}, Output: 0, Max: {}",
                estimated_input_tokens, current_model.max_tokens
            )));
        }

        // Check rate limits
        if !self
            .configuration_service
            .can_make_request(&current_model.provider, Some(estimated_input_tokens))
        {
            self.configuration_service
                .wait_for_request(&current_model.provider, Some(estimated_input_tokens))
                .await?;
        }

        // Show loading indicator
        self.ui.show_loading("Sending message...")?;

        // Send request
        let start_time = Instant::now();
        let response_result = if self.should_stream_response(&current_model.provider) {
            self.collect_streaming_response(
                current_model,
                api_messages.clone(),
                &api_key,
                model_config
                    .max_tokens
                    .min(current_model.max_tokens - estimated_input_tokens),
                model_config.temperature,
                estimated_input_tokens,
            )
            .await
        } else {
            self.api_client
                .send_message(
                    current_model,
                    api_messages,
                    &api_key,
                    model_config
                        .max_tokens
                        .min(current_model.max_tokens - estimated_input_tokens),
                    model_config.temperature,
                )
                .await
        };

        match response_result {
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
                self.message_manager
                    .add_message(session_id, assistant_message.clone())
                    .await?;

                // Index the assistant message
                self.search_index
                    .index_message(session_id, &assistant_message)
                    .await?;
                self.search_index.commit().await?;

                // Sync messages from MessageStore to Session before updating metadata
                // Update session metadata (MessageManager is the source of truth)
                self.session_manager
                    .update_session_metadata(session_id, &self.message_manager)
                    .await?;

                // Update search index
                self.session_manager.update_session_index(session_id);

                // Show success info
                println!(
                    "{}",
                    format!(
                        "✅ Response received in {:.2}s | Tokens: In:{} Out:{} Total:{}",
                        duration.as_secs_f64(),
                        token_usage.input_tokens,
                        token_usage.output_tokens,
                        token_usage.total_tokens
                    )
                    .bright_green()
                );
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

                self.message_manager
                    .add_message(session_id, error_message.clone())
                    .await?;

                // Index the error message
                self.search_index
                    .index_message(session_id, &error_message)
                    .await?;
                return Err(e);
            }
        }

        Ok(())
    }

    fn should_stream_response(&self, provider: &str) -> bool {
        let streaming_enabled = self
            .configuration_service
            .get_global_config()
            .ui_config
            .enable_streaming;

        streaming_enabled && matches!(provider, "openai" | "anthropic" | "groq" | "together")
    }

    async fn collect_streaming_response(
        &self,
        model: &crate::models::AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
        estimated_input_tokens: u32,
    ) -> Result<(String, TokenUsage)> {
        let mut stream = self
            .api_client
            .send_streaming_message(model, messages, api_key, max_tokens, temperature)
            .await?;
        let mut response = String::new();
        let mut token_usage = TokenUsage::default();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            response.push_str(&chunk.content);

            if let Some(usage) = chunk.token_usage {
                token_usage = usage;
            }

            if chunk.is_complete {
                break;
            }
        }

        if token_usage.total_tokens == 0 {
            let output_tokens = (response.len() / 4).max(1) as u32;
            token_usage = TokenUsage {
                input_tokens: estimated_input_tokens,
                output_tokens,
                total_tokens: estimated_input_tokens + output_tokens,
            };
        }

        Ok((response, token_usage))
    }

    async fn handle_select_model(&mut self, model_key: String) -> Result<()> {
        // Validate model exists
        let model = self
            .model_registry
            .get_model(&model_key)
            .ok_or_else(|| EnhancedError::config(format!("Unsupported model: {}", model_key)))?;

        // Check if API key is configured
        if let Err(_) = self.configuration_service.get_api_key(&model.provider) {
            return Err(EnhancedError::auth(format!(
                "Invalid API key for model: {}",
                model.provider
            )));
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

            self.message_manager
                .add_message(session_id, system_message.clone())
                .await?;

            // Index the system message
            self.search_index
                .index_message(session_id, &system_message)
                .await?;

            self.session_manager
                .update_session_metadata(session_id, &self.message_manager)
                .await?;
        }

        println!(
            "{}",
            format!("🔄 Switched to model: {} ({})", model.name, model.provider).bright_blue()
        );

        Ok(())
    }

    async fn handle_create_new_session(&mut self) -> Result<()> {
        let session_id = self
            .session_manager
            .create_session(Some("New Chat".to_string()))
            .await?;
        self.session_manager.switch_session(session_id).await?;
        self.current_session_id = Some(session_id);

        println!("{}", "📝 Created new chat session".bright_green());
        Ok(())
    }

    async fn handle_switch_session(&mut self, session_id: crate::events::SessionId) -> Result<()> {
        self.session_manager.switch_session(session_id).await?;
        self.current_session_id = Some(session_id);

        if let Some(session) = self.session_manager.get_session(session_id) {
            println!(
                "{}",
                format!("🔄 Switched to session: {}", session.title).bright_blue()
            );
        }

        Ok(())
    }

    async fn handle_export_session(
        &mut self,
        format: crate::export::formats::ExportFormat,
    ) -> Result<()> {
        let session_id = self
            .current_session_id
            .ok_or_else(|| EnhancedError::session("No active session to export"))?;

        let session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| {
                EnhancedError::session(format!("Session not found: {}", session_id))
                    .with_session(session_id)
            })?;

        let request = crate::export::service::SessionExportRequest {
            session_id,
            format,
            options: crate::export::formats::ExportOptions::default(),
            output_path: None,
        };

        let result = self
            .export_service
            .export_session(session, request, &self.message_manager)?;

        println!(
            "{}",
            format!("📄 Session exported to: {}", result.file_path).bright_green()
        );
        Ok(())
    }

    async fn handle_navigation_action(
        &mut self,
        action: crate::ui::enhanced::NavigationAction,
    ) -> Result<()> {
        use crate::ui::enhanced::NavigationAction;

        match action {
            NavigationAction::NextSession => {
                let sessions = self.session_manager.get_session_tabs();
                if let Some(current_id) = self.current_session_id {
                    if let Some(current_index) =
                        sessions.iter().position(|tab| tab.id == current_id)
                    {
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
                    if let Some(current_index) =
                        sessions.iter().position(|tab| tab.id == current_id)
                    {
                        let prev_index = if current_index == 0 {
                            sessions.len() - 1
                        } else {
                            current_index - 1
                        };
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
        let total_chars: usize = messages
            .iter()
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
    pub fn get_search_index(&self) -> &Arc<TantivyMessageSearchIndex> {
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
        self.current_session_id
            .and_then(|id| self.session_manager.get_session(id))
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Cleanup will be handled by UI's Drop implementation
    }
}

#[cfg(any())]
mod app_integration_tests;
