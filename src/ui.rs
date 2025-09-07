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
use crate::{
    chat::{ChatSession, Message, MessageRole},
    config::Config,
    models::{AIModel, ModelRegistry},
    RuffError,
};

pub struct UI {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    input_buffer: String,
    cursor_position: usize,
    scroll_offset: usize,
    show_model_selector: bool,
    selected_model_index: usize,
    model_registry: ModelRegistry,
}

// Standalone render functions
fn render_header(f: &mut Frame, area: Rect, session: &ChatSession, current_model: &AIModel) {
    let title = format!("🦀 Ruff - {} | Model: {} | Session: {}", 
        current_model.name, 
        current_model.provider,
        session.title.chars().take(30).collect::<String>()
    );
    
    let header = Paragraph::new(title)
        .style(Style::default().fg(RatatuiColor::Red).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
        
    f.render_widget(header, area);
}

fn render_messages(f: &mut Frame, area: Rect, session: &ChatSession, scroll_offset: usize) {
    let messages: Vec<ListItem> = session
        .messages
        .iter()
        .skip(scroll_offset)
        .map(|msg| format_message(msg))
        .collect();
    
    let messages_list = List::new(messages)
        .block(Block::default().borders(Borders::ALL).title("Chat"))
        .style(Style::default().fg(RatatuiColor::White));
        
    f.render_widget(messages_list, area);
}

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

fn render_status(f: &mut Frame, area: Rect, session: &ChatSession) {
    let session_id_short = &session.id.to_string()[..8];
    let status_text = format!(
        "Session: {} | Messages: {} | Tokens: {} | Commands: Enter=Send, Ctrl+M=Models, Ctrl+C=Quit",
        session_id_short,
        session.messages.len(),
        session.total_tokens_used.total_tokens,
    );
    
    let status = Paragraph::new(status_text)
        .style(Style::default().fg(RatatuiColor::DarkGray))
        .wrap(Wrap { trim: true });
        
    f.render_widget(status, area);
}

fn render_model_selector(f: &mut Frame, model_registry: &ModelRegistry, selected_model_index: usize) {
    let area = centered_rect(60, 70, f.area());
    
    // Clear the area
    f.render_widget(Clear, area);
    
    let models: Vec<ListItem> = model_registry
        .list_models()
        .into_iter()
        .enumerate()
        .map(|(i, (_key, model))| {
            let style = if i == selected_model_index {
                Style::default().bg(RatatuiColor::Red).fg(RatatuiColor::White)
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
                Span::styled(format!("{:<15}", model.provider), Style::default().fg(provider_color)),
                Span::styled(format!("{:<25}", model.name), Style::default().fg(RatatuiColor::White)),
                Span::styled(format!("{}K tokens", model.max_tokens / 1000), Style::default().fg(RatatuiColor::Gray)),
            ])).style(style)
        })
        .collect();
    
    let model_list = List::new(models)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Select AI Model (↑↓ to navigate, Enter to select, Esc to cancel)")
            .title_style(Style::default().fg(RatatuiColor::Red)))
        .highlight_style(Style::default().bg(RatatuiColor::Red));
        
    f.render_widget(model_list, area);
}

fn format_message(message: &Message) -> ListItem {
    let timestamp = message.timestamp.format("%H:%M:%S");
    let (role_color, role_symbol) = match message.role {
        MessageRole::User => (RatatuiColor::Cyan, "👤"),
        MessageRole::Assistant => (RatatuiColor::Green, "🤖"),
        MessageRole::System => (RatatuiColor::Yellow, "⚙️"),
    };
    
    let role_line = Line::from(vec![
        Span::styled(format!("{} ", role_symbol), Style::default().fg(role_color)),
        Span::styled(format!("[{}] ", timestamp), Style::default().fg(RatatuiColor::Gray)),
        Span::styled(message.role.to_string(), Style::default().fg(role_color).add_modifier(Modifier::BOLD)),
    ]);
    
    let content_lines: Vec<Line> = message
        .content
        .lines()
        .map(|line| Line::from(Span::raw(format!("  {}", line))))
        .collect();
    
    let mut lines = vec![role_line];
    lines.extend(content_lines);
    
    if let Some(usage) = &message.token_usage {
        if usage.total_tokens > 0 || usage.input_tokens > 0 || usage.output_tokens > 0 {
            lines.push(Line::from(Span::styled(
                format!("  📊 Tokens - In: {} | Out: {} | Total: {}", 
                    usage.input_tokens, usage.output_tokens, usage.total_tokens),
                Style::default().fg(RatatuiColor::DarkGray)
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
    pub fn new(_config: Config) -> Result<Self, RuffError> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        Ok(Self {
            terminal,
            input_buffer: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            show_model_selector: false,
            selected_model_index: 0,
            model_registry: ModelRegistry::new(),
        })
    }
    
    pub fn cleanup(&mut self) -> Result<(), RuffError> {
        terminal::disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            cursor::Show,
            terminal::LeaveAlternateScreen
        )?;
        Ok(())
    }
    
    pub fn render(&mut self, session: &ChatSession, current_model: &AIModel) -> Result<(), RuffError> {
    // Extract all needed data before the closure
    let show_model_selector = self.show_model_selector;
    let input_buffer = self.input_buffer.clone();
    let scroll_offset = self.scroll_offset;
    let selected_model_index = self.selected_model_index;
    let model_registry = self.model_registry.clone();
    
    self.terminal.draw(move |f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(5),     // Messages
                Constraint::Length(4),  // Input
                Constraint::Length(2),  // Status
            ].as_ref())
            .split(f.area());
        
        // Header
        render_header(f, chunks[0], session, current_model);
        
        // Messages
        render_messages(f, chunks[1], session, scroll_offset);
        
        // Input
        render_input(f, chunks[2], &input_buffer);
        
        // Status bar
        render_status(f, chunks[3], session);
        
        // Model selector overlay
        if show_model_selector {
            render_model_selector(f, &model_registry, selected_model_index);
        }
    })?;
    
    Ok(())
}

    pub fn handle_key_event(&mut self, key: KeyEvent) -> UIAction {
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
    
    pub fn show_error(&mut self, error: &str) -> Result<(), RuffError> {
        // Simple error display - in a real implementation, you might want a popup
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Red),
            Print(format!("\n❌ Error: {}\n", error)),
            ResetColor
        )?;
        Ok(())
    }
    
    pub fn show_loading(&mut self, message: &str) -> Result<(), RuffError> {
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
    Quit,
}

impl Drop for UI {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}