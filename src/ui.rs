use crate::{
    config::Config,
    models::{AIModel, ModelRegistry},
    plugin::{UIExtensionConfig, UIExtensionManager},
    ui::enhanced::{
        layout::{PaneConfig, ResizeDirection},
        CommandPalette, HelpSystem, LayoutManager, MarkdownRenderer, SyntaxHighlighter,
        ThemeService,
    },
    EnhancedError,
};
use crossterm::{
    cursor,
    event::{self, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color as RatatuiColor, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, Stdout};

pub mod enhanced;

#[cfg(any())]
mod font_size_tests;

#[cfg(any())]
mod enhanced_integration_tests;

pub struct UI {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    input_buffer: String,
    cursor_position: usize,
    scroll_offset: usize,
    show_model_selector: bool,
    selected_model_index: usize,
    model_registry: ModelRegistry,
    layout_manager: LayoutManager,
    ui_extension_manager: Option<std::sync::Arc<UIExtensionManager>>,
    ui_extension_config: UIExtensionConfig,

    // Enhanced UI components
    markdown_renderer: MarkdownRenderer,
    syntax_highlighter: SyntaxHighlighter,
    theme_service: ThemeService,
    command_palette: CommandPalette,
    help_system: HelpSystem,
}

// Standalone render functions
#[allow(dead_code)] // Legacy function kept for reference
fn render_header(
    f: &mut Frame,
    area: Rect,
    session: &crate::session::manager::ChatSession,
    current_model: &AIModel,
) {
    render_header_with_font_size(f, area, session, current_model, 14);
}

#[allow(dead_code)] // Legacy function kept for reference
fn render_header_with_font_size(
    f: &mut Frame,
    area: Rect,
    session: &crate::session::manager::ChatSession,
    current_model: &AIModel,
    font_size: u16,
) {
    let title = format!(
        "🦀 Ruff - {} | Model: {} | Session: {} | Font: {}pt",
        current_model.name,
        current_model.provider,
        session.title.chars().take(25).collect::<String>(),
        font_size
    );

    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(RatatuiColor::Red)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(header, area);
}

fn render_header_with_theme(
    f: &mut Frame,
    area: Rect,
    session: &crate::session::manager::ChatSession,
    current_model: &AIModel,
    font_size: u16,
    theme_colors: &crate::ui::enhanced::ThemeColors,
) {
    let title = format!(
        "🦀 Ruff - {} | Model: {} | Session: {} | Font: {}pt",
        current_model.name,
        current_model.provider,
        session.title.chars().take(25).collect::<String>(),
        font_size
    );

    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(theme_colors.primary)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme_colors.outline)),
        )
        .alignment(Alignment::Center);

    f.render_widget(header, area);
}

fn render_messages(
    f: &mut Frame,
    area: Rect,
    _session: &crate::session::manager::ChatSession,
    messages: &[crate::session::manager::Message],
    scroll_offset: usize,
    markdown_renderer: &MarkdownRenderer,
) {
    let message_items: Vec<ListItem> = messages
        .iter()
        .skip(scroll_offset)
        .map(|msg| format_message(msg, markdown_renderer))
        .collect();

    let messages_list = List::new(message_items)
        .block(Block::default().borders(Borders::ALL).title("Chat"))
        .style(Style::default().fg(RatatuiColor::White));

    f.render_widget(messages_list, area);
}

#[allow(dead_code)] // Legacy function kept for reference
fn render_input(f: &mut Frame, area: Rect, input_buffer: &str) {
    let input_text = if input_buffer.is_empty() {
        "Type your message... (Ctrl+M for models, Ctrl+C to quit)".to_string()
    } else {
        input_buffer.to_string()
    };

    let input = Paragraph::new(input_text)
        .style(Style::default().fg(if input_buffer.is_empty() {
            RatatuiColor::DarkGray
        } else {
            RatatuiColor::White
        }))
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(Wrap { trim: false });

    f.render_widget(input, area);
}

fn render_input_with_theme(
    f: &mut Frame,
    area: Rect,
    input_buffer: &str,
    theme_colors: &crate::ui::enhanced::ThemeColors,
) {
    let input_text = if input_buffer.is_empty() {
        "Type your message... (Ctrl+Shift+P commands, F1 help, Ctrl+N new session, Ctrl+C quit)"
            .to_string()
    } else {
        input_buffer.to_string()
    };

    let input = Paragraph::new(input_text)
        .style(Style::default().fg(if input_buffer.is_empty() {
            theme_colors.on_surface_variant
        } else {
            theme_colors.on_surface
        }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input")
                .border_style(Style::default().fg(theme_colors.outline))
                .title_style(Style::default().fg(theme_colors.primary)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(input, area);
}

#[allow(dead_code)] // Legacy function kept for reference
fn render_status(f: &mut Frame, area: Rect, session: &crate::session::manager::ChatSession) {
    render_status_with_font_size(f, area, session, 14);
}

#[allow(dead_code)] // Legacy function kept for reference
fn render_status_with_font_size(
    f: &mut Frame,
    area: Rect,
    session: &crate::session::manager::ChatSession,
    font_size: u16,
) {
    let session_id_short = &session.id.to_string()[..8];
    let status_text = format!(
        "Session: {} | Messages: {} | Tokens: {} | Font: {}pt | Ctrl+/- Zoom, Ctrl+0 Reset, Ctrl+M Models, Ctrl+C Quit",
        session_id_short,
        session.message_count,
        session.total_tokens_used.total_tokens,
        font_size
    );

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(RatatuiColor::DarkGray))
        .wrap(Wrap { trim: true });

    f.render_widget(status, area);
}

fn render_status_with_theme(
    f: &mut Frame,
    area: Rect,
    session: &crate::session::manager::ChatSession,
    font_size: u16,
    theme_colors: &crate::ui::enhanced::ThemeColors,
) {
    let session_id_short = &session.id.to_string()[..8];
    let status_text = format!(
        "Session: {} | Messages: {} | Tokens: {} | Font: {}pt | Ctrl+Shift+P Commands, F1 Help, Ctrl+N New, Ctrl+S Export",
        session_id_short,
        session.message_count,
        session.total_tokens_used.total_tokens,
        font_size
    );

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(theme_colors.on_surface_variant))
        .wrap(Wrap { trim: true });

    f.render_widget(status, area);
}

#[allow(dead_code)] // Legacy function kept for reference
fn render_model_selector(
    f: &mut Frame,
    model_registry: &ModelRegistry,
    selected_model_index: usize,
) {
    let area = centered_rect(60, 70, f.area());

    // Clear the area
    f.render_widget(Clear, area);

    let models: Vec<ListItem> = model_registry
        .list_models()
        .into_iter()
        .enumerate()
        .map(|(i, (_key, model))| {
            let style = if i == selected_model_index {
                Style::default()
                    .bg(RatatuiColor::Red)
                    .fg(RatatuiColor::White)
            } else {
                Style::default()
            };

            let provider_color = match model.provider.as_str() {
                "openai" => RatatuiColor::Green,
                "anthropic" => RatatuiColor::Blue,
                "cohere" => RatatuiColor::Magenta,
                "together" => RatatuiColor::Cyan,
                "groq" => RatatuiColor::Yellow,
                "huggingface" => RatatuiColor::Red,
                _ => RatatuiColor::White,
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<15}", model.provider),
                    Style::default().fg(provider_color),
                ),
                Span::styled(
                    format!("{:<25}", model.name),
                    Style::default().fg(RatatuiColor::White),
                ),
                Span::styled(
                    format!("{}K tokens", model.max_tokens / 1000),
                    Style::default().fg(RatatuiColor::Gray),
                ),
            ]))
            .style(style)
        })
        .collect();

    let model_list = List::new(models)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select AI Model (↑↓ to navigate, Enter to select, Esc to cancel)")
                .title_style(Style::default().fg(RatatuiColor::Red)),
        )
        .highlight_style(Style::default().bg(RatatuiColor::Red));

    f.render_widget(model_list, area);
}

fn render_model_selector_with_theme(
    f: &mut Frame,
    model_registry: &ModelRegistry,
    selected_model_index: usize,
    theme_colors: &crate::ui::enhanced::ThemeColors,
) {
    let area = centered_rect(60, 70, f.area());

    // Clear the area
    f.render_widget(Clear, area);

    let models: Vec<ListItem> = model_registry
        .list_models()
        .into_iter()
        .enumerate()
        .map(|(i, (_key, model))| {
            let style = if i == selected_model_index {
                Style::default()
                    .bg(theme_colors.primary)
                    .fg(theme_colors.on_primary)
            } else {
                Style::default().fg(theme_colors.on_surface)
            };

            let provider_color = match model.provider.as_str() {
                "openai" => theme_colors.success,
                "anthropic" => theme_colors.info,
                "cohere" => theme_colors.secondary,
                "together" => theme_colors.primary,
                "groq" => theme_colors.warning,
                "huggingface" => theme_colors.error,
                _ => theme_colors.on_surface,
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<15}", model.provider),
                    Style::default().fg(provider_color),
                ),
                Span::styled(
                    format!("{:<25}", model.name),
                    Style::default().fg(theme_colors.on_surface),
                ),
                Span::styled(
                    format!("{}K tokens", model.max_tokens / 1000),
                    Style::default().fg(theme_colors.on_surface_variant),
                ),
            ]))
            .style(style)
        })
        .collect();

    let model_list = List::new(models)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select AI Model (↑↓ to navigate, Enter to select, Esc to cancel)")
                .title_style(Style::default().fg(theme_colors.primary))
                .border_style(Style::default().fg(theme_colors.outline)),
        )
        .highlight_style(
            Style::default()
                .bg(theme_colors.primary_container)
                .fg(theme_colors.on_primary_container),
        );

    f.render_widget(model_list, area);
}

fn format_message<'a>(
    message: &'a crate::session::manager::Message,
    markdown_renderer: &'a MarkdownRenderer,
) -> ListItem<'a> {
    let timestamp = message.timestamp.format("%H:%M:%S");
    let (role_color, role_symbol) = match message.role {
        crate::session::manager::MessageRole::User => (RatatuiColor::Cyan, "👤"),
        crate::session::manager::MessageRole::Assistant => (RatatuiColor::Green, "🤖"),
        crate::session::manager::MessageRole::System => (RatatuiColor::Yellow, "⚙️"),
    };

    let role_line = Line::from(vec![
        Span::styled(format!("{} ", role_symbol), Style::default().fg(role_color)),
        Span::styled(
            format!("[{}] ", timestamp),
            Style::default().fg(RatatuiColor::Gray),
        ),
        Span::styled(
            message.role.to_string(),
            Style::default().fg(role_color).add_modifier(Modifier::BOLD),
        ),
    ]);

    // Render message content with markdown if it's an assistant message
    let content_lines: Vec<Line> =
        if message.role == crate::session::manager::MessageRole::Assistant {
            // Use markdown rendering for AI responses
            let rendered = markdown_renderer.render(&message.content);
            rendered
                .text
                .lines
                .into_iter()
                .map(|line| {
                    // Add indentation to each line
                    let mut spans = vec![Span::raw("  ")];
                    spans.extend(line.spans);
                    Line::from(spans)
                })
                .collect()
        } else {
            // Use plain text for user messages
            message
                .content
                .lines()
                .map(|line| Line::from(Span::raw(format!("  {}", line))))
                .collect()
        };

    let mut lines = vec![role_line];
    lines.extend(content_lines);

    if let Some(usage) = &message.token_usage {
        if usage.total_tokens > 0 || usage.input_tokens > 0 || usage.output_tokens > 0 {
            lines.push(Line::from(Span::styled(
                format!(
                    "  📊 Tokens - In: {} | Out: {} | Total: {}",
                    usage.input_tokens, usage.output_tokens, usage.total_tokens
                ),
                Style::default().fg(RatatuiColor::DarkGray),
            )));
        }
    }

    lines.push(Line::from(Span::raw(""))); // Empty line separator

    ListItem::new(Text::from(lines))
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

impl UI {
    pub fn new(_config: Config) -> Result<Self, EnhancedError> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Initialize layout manager with config file path
        let config_dir = dirs::config_dir()
            .ok_or_else(|| EnhancedError::unknown("Could not find config directory".to_string()))?
            .join("ruff");

        std::fs::create_dir_all(&config_dir)?;
        let layout_config_path = config_dir.join("layout.json");

        let mut layout_manager = LayoutManager::new().with_config_file(layout_config_path);

        // Set up default panes
        let header_pane = PaneConfig::new("header".to_string())
            .with_min_size(80, 3)
            .with_preferred_size(80, 3)
            .non_resizable();

        let messages_pane = PaneConfig::new("messages".to_string())
            .with_min_size(80, 10)
            .with_preferred_size(80, 20)
            .with_resize_direction(ResizeDirection::Vertical);

        let input_pane = PaneConfig::new("input".to_string())
            .with_min_size(80, 4)
            .with_preferred_size(80, 4)
            .with_resize_direction(ResizeDirection::Vertical);

        let status_pane = PaneConfig::new("status".to_string())
            .with_min_size(80, 2)
            .with_preferred_size(80, 2)
            .non_resizable();

        layout_manager.add_pane(header_pane)?;
        layout_manager.add_pane(messages_pane)?;
        layout_manager.add_pane(input_pane)?;
        layout_manager.add_pane(status_pane)?;

        // Try to load existing layout configuration
        let _ = layout_manager.load_layout();

        Ok(Self {
            terminal,
            input_buffer: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            show_model_selector: false,
            selected_model_index: 0,
            model_registry: ModelRegistry::new(),
            layout_manager,
            ui_extension_manager: None,
            ui_extension_config: UIExtensionConfig::default(),

            // Initialize enhanced UI components
            markdown_renderer: MarkdownRenderer::new(),
            syntax_highlighter: SyntaxHighlighter::new(),
            theme_service: ThemeService::new(),
            command_palette: CommandPalette::new(),
            help_system: HelpSystem::new(),
        })
    }

    pub fn cleanup(&mut self) -> Result<(), EnhancedError> {
        terminal::disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            cursor::Show,
            terminal::LeaveAlternateScreen
        )?;
        Ok(())
    }

    pub fn render(
        &mut self,
        session: &crate::session::manager::ChatSession,
        messages: &[crate::session::manager::Message],
        current_model: &AIModel,
    ) -> Result<(), EnhancedError> {
        // Calculate layout using the layout manager
        let terminal_size = self.terminal.size()?;
        let total_area = Rect {
            x: 0,
            y: 0,
            width: terminal_size.width,
            height: terminal_size.height,
        };

        // Calculate plugin UI extension layout first
        // NOTE: We skip async plugin extensions when inside a Tokio runtime because
        // block_on cannot be called from within an async runtime context.
        // This is fine since plugins are optional and rarely used.
        let extension_layout: Option<crate::plugin::ui_extensions::UIExtensionLayout> = None;

        // Use main content area from extension layout or full area
        let main_content_area = extension_layout
            .as_ref()
            .map(|layout| layout.main_content_area)
            .unwrap_or(total_area);

        let layout_result = self.layout_manager.calculate_layout(main_content_area)?;

        // Extract all needed data before the closure
        let show_model_selector = self.show_model_selector;
        let input_buffer = self.input_buffer.clone();
        let scroll_offset = self.scroll_offset;
        let selected_model_index = self.selected_model_index;
        let model_registry = self.model_registry.clone();
        let font_size = self.layout_manager.get_font_size();
        let pane_areas = layout_result.pane_areas.clone();
        let extension_manager = self.ui_extension_manager.clone();

        // Get current theme
        let current_theme = self.theme_service.get_current_theme();
        let theme_colors = current_theme.colors.clone();

        // Clone components for use in closure
        let markdown_renderer = self.markdown_renderer.clone();
        let command_palette_visible = self.command_palette.is_visible();
        let command_palette = self.command_palette.clone();
        let help_system_visible = self.help_system.is_visible();
        let help_system = self.help_system.clone();
        let messages_vec = messages.to_vec(); // Clone messages for closure

        self.terminal.draw(move |f| {
            // NOTE: Plugin UI extensions are currently disabled to avoid block_on issues
            // when running inside an async runtime. The extension_layout is always None.
            let _ = (&extension_layout, &extension_manager); // Suppress unused variable warnings

            // Use layout manager calculated areas or fall back to default layout
            let header_area = pane_areas.get("header").copied().unwrap_or_else(|| Rect {
                x: main_content_area.x,
                y: main_content_area.y,
                width: main_content_area.width,
                height: 3,
            });

            let messages_area = pane_areas.get("messages").copied().unwrap_or_else(|| Rect {
                x: main_content_area.x,
                y: main_content_area.y + 3,
                width: main_content_area.width,
                height: main_content_area.height.saturating_sub(9),
            });

            let input_area = pane_areas.get("input").copied().unwrap_or_else(|| Rect {
                x: main_content_area.x,
                y: main_content_area.y + main_content_area.height.saturating_sub(6),
                width: main_content_area.width,
                height: 4,
            });

            let status_area = pane_areas.get("status").copied().unwrap_or_else(|| Rect {
                x: main_content_area.x,
                y: main_content_area.y + main_content_area.height.saturating_sub(2),
                width: main_content_area.width,
                height: 2,
            });

            // Render components with enhanced styling and markdown support
            render_header_with_theme(
                f,
                header_area,
                session,
                current_model,
                font_size,
                &theme_colors,
            );
            render_messages(
                f,
                messages_area,
                session,
                &messages_vec,
                scroll_offset,
                &markdown_renderer,
            );
            render_input_with_theme(f, input_area, &input_buffer, &theme_colors);
            render_status_with_theme(f, status_area, session, font_size, &theme_colors);

            // Render overlays
            if show_model_selector {
                render_model_selector_with_theme(
                    f,
                    &model_registry,
                    selected_model_index,
                    &theme_colors,
                );
            }

            if command_palette_visible {
                render_command_palette(f, &command_palette, &theme_colors);
            }

            if help_system_visible {
                render_help_system(f, &help_system, &theme_colors);
            }
        })?;

        Ok(())
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> UIAction {
        // Plugin UI extension input is handled by App before entering this synchronous path.

        // Handle enhanced UI components first
        if self.command_palette.is_visible() {
            return self.command_palette.handle_key_event(key);
        }

        if self.help_system.is_visible() {
            return self.help_system.handle_key_event(key);
        }

        if self.show_model_selector {
            match key.code {
                KeyCode::Esc => {
                    self.show_model_selector = false;
                    UIAction::None
                }
                KeyCode::Up => {
                    if self.selected_model_index > 0 {
                        self.selected_model_index -= 1;
                    }
                    UIAction::None
                }
                KeyCode::Down => {
                    let model_count = self.model_registry.list_models().len();
                    if self.selected_model_index < model_count.saturating_sub(1) {
                        self.selected_model_index += 1;
                    }
                    UIAction::None
                }
                KeyCode::Enter => {
                    let models = self.model_registry.list_models();
                    if let Some((model_key, _)) = models.get(self.selected_model_index) {
                        let result = UIAction::SelectModel(model_key.to_string());
                        self.show_model_selector = false;
                        result
                    } else {
                        UIAction::None
                    }
                }
                _ => UIAction::None,
            }
        } else {
            match key.code {
                KeyCode::Enter => {
                    if !self.input_buffer.trim().is_empty() {
                        let message = self.input_buffer.clone();
                        self.input_buffer.clear();
                        self.cursor_position = 0;
                        UIAction::SendMessage(message)
                    } else {
                        UIAction::None
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    UIAction::Quit
                }
                KeyCode::Char('m') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    self.show_model_selector = true;
                    UIAction::None
                }
                KeyCode::Char('p')
                    if key
                        .modifiers
                        .contains(event::KeyModifiers::CONTROL | event::KeyModifiers::SHIFT) =>
                {
                    self.command_palette.show();
                    UIAction::ShowCommandPalette
                }
                KeyCode::F(1) => {
                    self.help_system.show();
                    UIAction::ShowHelp
                }
                KeyCode::Char('?') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    self.help_system.show();
                    UIAction::ShowHelp
                }
                KeyCode::Char('n') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    UIAction::CreateNewSession
                }
                KeyCode::Char('o') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    // Open session browser - for now just show command palette
                    self.command_palette.show();
                    UIAction::ShowCommandPalette
                }
                KeyCode::F(2) => {
                    // Rename session - for now just show command palette
                    self.command_palette.show();
                    UIAction::ShowCommandPalette
                }
                KeyCode::Char('f')
                    if key
                        .modifiers
                        .contains(event::KeyModifiers::CONTROL | event::KeyModifiers::SHIFT) =>
                {
                    // Global search - for now just show command palette
                    self.command_palette.show();
                    UIAction::ShowCommandPalette
                }
                KeyCode::Char('f') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    // Search messages in current session - for now just show command palette
                    self.command_palette.show();
                    UIAction::ShowCommandPalette
                }
                KeyCode::Char('s') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    // Export conversation
                    UIAction::ExportSession(crate::export::formats::ExportFormat::Markdown)
                }
                KeyCode::Left if key.modifiers.contains(event::KeyModifiers::ALT) => {
                    UIAction::PreviousSession
                }
                KeyCode::Right if key.modifiers.contains(event::KeyModifiers::ALT) => {
                    UIAction::NextSession
                }
                KeyCode::Char('+') | KeyCode::Char('=')
                    if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                {
                    self.increase_font_size()
                        .map(UIAction::FontSizeChanged)
                        .unwrap_or(UIAction::None)
                }
                KeyCode::Char('-') if key.modifiers.contains(event::KeyModifiers::CONTROL) => self
                    .decrease_font_size()
                    .map(UIAction::FontSizeChanged)
                    .unwrap_or(UIAction::None),
                KeyCode::Char('0') if key.modifiers.contains(event::KeyModifiers::CONTROL) => self
                    .set_font_size(14)
                    .map(UIAction::FontSizeChanged)
                    .unwrap_or(UIAction::None),
                KeyCode::Char('g') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    // Go to message - this would open a dialog in a full implementation
                    UIAction::None
                }
                KeyCode::Home if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    // Go to first message
                    self.scroll_offset = 0;
                    UIAction::None
                }
                KeyCode::End if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    // Go to last message
                    self.scroll_offset = usize::MAX;
                    UIAction::None
                }
                KeyCode::Backspace => {
                    if self.cursor_position > 0 {
                        self.input_buffer.remove(self.cursor_position - 1);
                        self.cursor_position -= 1;
                    }
                    UIAction::None
                }
                KeyCode::Delete => {
                    if self.cursor_position < self.input_buffer.len() {
                        self.input_buffer.remove(self.cursor_position);
                    }
                    UIAction::None
                }
                KeyCode::Left => {
                    if self.cursor_position > 0 {
                        self.cursor_position -= 1;
                    }
                    UIAction::None
                }
                KeyCode::Right => {
                    if self.cursor_position < self.input_buffer.len() {
                        self.cursor_position += 1;
                    }
                    UIAction::None
                }
                KeyCode::Home => {
                    self.cursor_position = 0;
                    UIAction::None
                }
                KeyCode::End => {
                    self.cursor_position = self.input_buffer.len();
                    UIAction::None
                }
                KeyCode::PageUp => {
                    if self.scroll_offset > 0 {
                        self.scroll_offset = self.scroll_offset.saturating_sub(10);
                    }
                    UIAction::None
                }
                KeyCode::PageDown => {
                    self.scroll_offset += 10;
                    UIAction::None
                }
                KeyCode::Char(c) => {
                    self.input_buffer.insert(self.cursor_position, c);
                    self.cursor_position += 1;
                    UIAction::None
                }
                _ => UIAction::None,
            }
        }
    }

    pub fn show_error(&mut self, _error: &str) -> Result<(), EnhancedError> {
        // Simple error display - in a real implementation, you might want a proper error dialog
        // For now, we'll just store the error message to be displayed in the status bar
        Ok(())
    }

    // Getter methods for enhanced components (needed for tests)
    pub fn get_markdown_renderer(&self) -> &MarkdownRenderer {
        &self.markdown_renderer
    }

    pub fn get_syntax_highlighter(&self) -> &SyntaxHighlighter {
        &self.syntax_highlighter
    }

    pub fn get_theme_service(&self) -> &ThemeService {
        &self.theme_service
    }

    pub fn get_command_palette(&self) -> &CommandPalette {
        &self.command_palette
    }

    pub fn get_command_palette_mut(&mut self) -> &mut CommandPalette {
        &mut self.command_palette
    }

    pub fn get_help_system(&self) -> &HelpSystem {
        &self.help_system
    }

    pub fn get_help_system_mut(&mut self) -> &mut HelpSystem {
        &mut self.help_system
    }

    pub fn get_layout_manager(&self) -> &LayoutManager {
        &self.layout_manager
    }

    pub fn get_ui_extension_manager(
        &self,
    ) -> &Option<std::sync::Arc<crate::plugin::UIExtensionManager>> {
        &self.ui_extension_manager
    }

    pub fn get_ui_extension_config(&self) -> &crate::plugin::UIExtensionConfig {
        &self.ui_extension_config
    }

    pub fn get_font_size(&self) -> u16 {
        self.layout_manager.get_font_size()
    }

    pub fn set_font_size(&mut self, size: u16) -> Result<u16, EnhancedError> {
        self.layout_manager.set_font_size(size)?;
        let _ = self.layout_manager.save_layout();
        Ok(size)
    }

    pub fn increase_font_size(&mut self) -> Result<u16, EnhancedError> {
        let current_size = self.layout_manager.get_font_size();
        self.set_font_size(current_size + 1)
    }

    pub fn decrease_font_size(&mut self) -> Result<u16, EnhancedError> {
        let current_size = self.layout_manager.get_font_size();
        if current_size > 8 {
            self.set_font_size(current_size - 1)
        } else {
            Err(EnhancedError::unknown(
                "Font size cannot be smaller than 8".to_string(),
            ))
        }
    }

    pub fn reset_font_size(&mut self) -> Result<(), EnhancedError> {
        self.set_font_size(14)?;
        Ok(())
    }

    pub fn set_theme(&mut self, theme_name: &str) -> Result<(), EnhancedError> {
        self.theme_service
            .set_theme(theme_name)
            .map_err(|e| EnhancedError::unknown(format!("Failed to set theme: {}", e)))
    }

    pub fn toggle_theme(&mut self) -> Result<(), EnhancedError> {
        self.theme_service
            .toggle_theme()
            .map_err(|e| EnhancedError::unknown(format!("Failed to toggle theme: {}", e)))
    }

    pub fn scroll_to_top(&mut self) -> Result<(), EnhancedError> {
        self.scroll_offset = 0;
        Ok(())
    }

    pub fn scroll_to_bottom(&mut self) -> Result<(), EnhancedError> {
        self.scroll_offset = usize::MAX;
        Ok(())
    }

    pub fn scroll_to_message(&mut self, message_index: usize) -> Result<(), EnhancedError> {
        self.scroll_offset = message_index;
        Ok(())
    }

    pub fn render_empty_state(&mut self, model: &AIModel) -> Result<(), EnhancedError> {
        let empty_session = crate::session::manager::ChatSession {
            id: uuid::Uuid::new_v4(),
            title: "New Session".to_string(),
            model: model.name.clone(),
            system_prompt: None,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
            total_tokens_used: crate::models::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
            model_config: crate::session::manager::SessionModelConfig::default(),
            tags: Vec::new(),
            is_archived: false,
            export_count: 0,
            message_count: 0,
            last_activity: chrono::Local::now(),
        };

        self.render(&empty_session, &[], model) // Empty messages for empty state
    }

    /// Set the UI extension manager
    pub fn set_ui_extension_manager(
        &mut self,
        manager: std::sync::Arc<crate::plugin::UIExtensionManager>,
    ) {
        self.ui_extension_manager = Some(manager);
    }

    /// Render with enhanced components
    pub fn render_enhanced(
        &mut self,
        session: &crate::session::manager::ChatSession,
        messages: &[crate::session::manager::Message],
        current_model: &AIModel,
        _layout_manager: &LayoutManager,
        _theme_service: &crate::ui::enhanced::ThemeService,
        _markdown_renderer: &crate::ui::enhanced::MarkdownRenderer,
        _syntax_highlighter: &crate::ui::enhanced::SyntaxHighlighter,
        _command_palette: &crate::ui::enhanced::CommandPalette,
        _help_system: &crate::ui::enhanced::HelpSystem,
    ) -> Result<(), EnhancedError> {
        // Use the main render method which now includes enhanced components
        self.render(session, messages, current_model)
    }

    pub fn show_loading(&mut self, message: &str) -> Result<(), EnhancedError> {
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Yellow),
            Print(format!("⏳ {}\n", message)),
            ResetColor
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum UIAction {
    None,
    SendMessage(String),
    SelectModel(String),
    FontSizeChanged(u16),
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ToggleTheme,
    ScrollToTop,
    ScrollToBottom,
    PreviousSession,
    NextSession,
    ShowCommandPalette,
    ShowHelp,
    CreateNewSession,
    SwitchSession(crate::events::SessionId),
    ExportSession(crate::export::formats::ExportFormat),
    Quit,
}

impl Drop for UI {
    fn drop(&mut self) {
        let _ = self.cleanup();
        // Save layout configuration on drop
        let _ = self.layout_manager.save_layout();
    }
}
fn render_command_palette(
    f: &mut Frame,
    command_palette: &CommandPalette,
    theme_colors: &crate::ui::enhanced::ThemeColors,
) {
    let area = centered_rect(80, 60, f.area());

    // Clear the area
    f.render_widget(Clear, area);

    // Split area for search input and results
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Min(0),    // Command list
        ])
        .split(area);

    // Render search input
    let search_input = Paragraph::new(command_palette.search_query())
        .style(Style::default().fg(theme_colors.on_surface))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Command Palette")
                .title_style(Style::default().fg(theme_colors.primary))
                .border_style(Style::default().fg(theme_colors.outline)),
        );

    f.render_widget(search_input, chunks[0]);

    // Render command list
    let commands: Vec<ListItem> = command_palette
        .filtered_commands()
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let style = if i == command_palette.selected_index() {
                Style::default()
                    .bg(theme_colors.primary_container)
                    .fg(theme_colors.on_primary_container)
            } else {
                Style::default().fg(theme_colors.on_surface)
            };

            let shortcut_text = result
                .command
                .shortcut
                .as_ref()
                .map(|s| format!(" ({})", s))
                .unwrap_or_default();

            let line = Line::from(vec![
                Span::styled(result.command.name.clone(), style),
                Span::styled(
                    shortcut_text,
                    Style::default().fg(theme_colors.on_surface_variant),
                ),
            ]);

            ListItem::new(vec![
                line,
                Line::from(Span::styled(
                    format!("  {}", result.command.description),
                    Style::default().fg(theme_colors.on_surface_variant),
                )),
            ])
            .style(style)
        })
        .collect();

    let command_list = List::new(commands)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Commands")
                .title_style(Style::default().fg(theme_colors.primary))
                .border_style(Style::default().fg(theme_colors.outline)),
        )
        .highlight_style(
            Style::default()
                .bg(theme_colors.primary)
                .fg(theme_colors.on_primary),
        );

    f.render_widget(command_list, chunks[1]);
}

fn render_help_system(
    f: &mut Frame,
    help_system: &HelpSystem,
    theme_colors: &crate::ui::enhanced::ThemeColors,
) {
    let area = centered_rect(90, 80, f.area());

    // Clear the area
    f.render_widget(Clear, area);

    // Split area for tabs and content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Min(0),    // Content
        ])
        .split(area);

    // Render tab bar
    let sections = crate::ui::enhanced::help::HelpSection::all();
    let tab_titles: Vec<Line> = sections
        .iter()
        .map(|section| {
            let style = if section == help_system.current_section() {
                Style::default()
                    .bg(theme_colors.primary)
                    .fg(theme_colors.on_primary)
            } else {
                Style::default().fg(theme_colors.on_surface_variant)
            };
            Line::from(Span::styled(section.display_name(), style))
        })
        .collect();

    let tabs = Paragraph::new(Text::from(tab_titles)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Help")
            .title_style(Style::default().fg(theme_colors.primary))
            .border_style(Style::default().fg(theme_colors.outline)),
    );

    f.render_widget(tabs, chunks[0]);

    // Render content based on current section
    match help_system.current_section() {
        crate::ui::enhanced::help::HelpSection::KeyboardShortcuts => {
            render_shortcuts_help(f, chunks[1], help_system, theme_colors);
        }
        _ => {
            render_general_help(f, chunks[1], help_system, theme_colors);
        }
    }
}

fn render_shortcuts_help(
    f: &mut Frame,
    area: Rect,
    help_system: &HelpSystem,
    theme_colors: &crate::ui::enhanced::ThemeColors,
) {
    // Split area for search and shortcuts
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Min(0),    // Shortcuts list
        ])
        .split(area);

    // Render search input
    let search_input = Paragraph::new(help_system.search_query())
        .style(Style::default().fg(theme_colors.on_surface))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search Shortcuts")
                .title_style(Style::default().fg(theme_colors.primary))
                .border_style(Style::default().fg(theme_colors.outline)),
        );

    f.render_widget(search_input, chunks[0]);

    // Render shortcuts list
    let shortcuts: Vec<ListItem> = help_system
        .filtered_shortcuts()
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let style = if i == help_system.selected_index() {
                Style::default()
                    .bg(theme_colors.primary_container)
                    .fg(theme_colors.on_primary_container)
            } else {
                Style::default().fg(theme_colors.on_surface)
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{:<20}", result.shortcut.key_combination),
                    Style::default().fg(theme_colors.primary),
                ),
                Span::styled(result.shortcut.description.clone(), style),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let shortcuts_list = List::new(shortcuts)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Keyboard Shortcuts")
                .title_style(Style::default().fg(theme_colors.primary))
                .border_style(Style::default().fg(theme_colors.outline)),
        )
        .highlight_style(
            Style::default()
                .bg(theme_colors.primary)
                .fg(theme_colors.on_primary),
        );

    f.render_widget(shortcuts_list, chunks[1]);
}

fn render_general_help(
    f: &mut Frame,
    area: Rect,
    help_system: &HelpSystem,
    theme_colors: &crate::ui::enhanced::ThemeColors,
) {
    let content = match help_system.get_help_content(help_system.current_section()) {
        Some(items) => {
            let mut lines = Vec::new();
            for item in items {
                lines.push(Line::from(Span::styled(
                    item.title.clone(),
                    Style::default()
                        .fg(theme_colors.primary)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(Span::raw("")));

                for line in item.content.lines() {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(theme_colors.on_surface),
                    )));
                }

                for subsection in &item.subsections {
                    lines.push(Line::from(Span::raw("")));
                    lines.push(Line::from(Span::styled(
                        subsection.title.clone(),
                        Style::default()
                            .fg(theme_colors.secondary)
                            .add_modifier(Modifier::BOLD),
                    )));

                    for line in subsection.content.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", line),
                            Style::default().fg(theme_colors.on_surface),
                        )));
                    }
                }

                lines.push(Line::from(Span::raw("")));
            }
            Text::from(lines)
        }
        None => Text::from("No help content available for this section."),
    };

    let help_content = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(help_system.current_section().display_name())
                .title_style(Style::default().fg(theme_colors.primary))
                .border_style(Style::default().fg(theme_colors.outline)),
        )
        .wrap(Wrap { trim: false })
        .scroll((0, 0));

    f.render_widget(help_content, area);
}
