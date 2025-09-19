use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;
use crate::{
    session::system_prompt::{SystemPromptManager, SystemPromptTemplate},
    RuffError,
};

/// System prompt editor UI state
pub struct SystemPromptEditor {
    pub is_visible: bool,
    pub mode: SystemPromptMode,
    pub selected_template_index: usize,
    pub templates: Vec<SystemPromptTemplate>,
    pub filtered_templates: Vec<usize>, // Indices into templates
    pub search_query: String,
    pub current_prompt: String,
    pub cursor_position: usize,
    pub scroll_offset: usize,
    pub template_scroll_offset: usize,
    pub variables: HashMap<String, String>,
    pub variable_input_index: usize,
    pub show_preview: bool,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
}

/// System prompt editor modes
#[derive(Debug, Clone, PartialEq)]
pub enum SystemPromptMode {
    TemplateList,
    TemplateEdit,
    CustomEdit,
    VariableInput,
    Preview,
}

impl SystemPromptEditor {
    /// Create a new system prompt editor
    pub fn new() -> Self {
        Self {
            is_visible: false,
            mode: SystemPromptMode::TemplateList,
            selected_template_index: 0,
            templates: Vec::new(),
            filtered_templates: Vec::new(),
            search_query: String::new(),
            current_prompt: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            template_scroll_offset: 0,
            variables: HashMap::new(),
            variable_input_index: 0,
            show_preview: false,
            error_message: None,
            success_message: None,
        }
    }
    
    /// Show the system prompt editor
    pub fn show(&mut self, manager: &SystemPromptManager) {
        self.is_visible = true;
        self.mode = SystemPromptMode::TemplateList;
        self.refresh_templates(manager);
        self.clear_messages();
    }
    
    /// Hide the system prompt editor
    pub fn hide(&mut self) {
        self.is_visible = false;
        self.clear_state();
    }
    
    /// Refresh templates from manager
    pub fn refresh_templates(&mut self, manager: &SystemPromptManager) {
        self.templates = manager.get_all_templates().into_iter().cloned().collect();
        self.filter_templates();
    }
    
    /// Filter templates based on search query
    pub fn filter_templates(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_templates = (0..self.templates.len()).collect();
        } else {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_templates = self.templates
                .iter()
                .enumerate()
                .filter(|(_, template)| {
                    template.name.to_lowercase().contains(&query_lower) ||
                    template.description.to_lowercase().contains(&query_lower) ||
                    template.content.to_lowercase().contains(&query_lower) ||
                    template.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
                })
                .map(|(i, _)| i)
                .collect();
        }
        
        // Reset selection if needed
        if self.selected_template_index >= self.filtered_templates.len() {
            self.selected_template_index = 0;
        }
    }
    
    /// Get currently selected template
    pub fn get_selected_template(&self) -> Option<&SystemPromptTemplate> {
        self.filtered_templates
            .get(self.selected_template_index)
            .and_then(|&index| self.templates.get(index))
    }
    
    /// Move selection up
    pub fn move_selection_up(&mut self) {
        if self.selected_template_index > 0 {
            self.selected_template_index -= 1;
        }
    }
    
    /// Move selection down
    pub fn move_selection_down(&mut self) {
        if self.selected_template_index + 1 < self.filtered_templates.len() {
            self.selected_template_index += 1;
        }
    }
    
    /// Select current template for editing
    pub fn select_template(&mut self) {
        if let Some(template) = self.get_selected_template().cloned() {
            self.current_prompt = template.content.clone();
            self.cursor_position = self.current_prompt.len();
            self.variables.clear();
            
            // Extract variables from template
            for var in &template.variables {
                self.variables.insert(var.clone(), String::new());
            }
            
            if template.variables.is_empty() {
                self.mode = SystemPromptMode::TemplateEdit;
            } else {
                self.mode = SystemPromptMode::VariableInput;
                self.variable_input_index = 0;
            }
        }
    }
    
    /// Start custom prompt editing
    pub fn start_custom_edit(&mut self) {
        self.mode = SystemPromptMode::CustomEdit;
        self.current_prompt.clear();
        self.cursor_position = 0;
        self.variables.clear();
    }
    
    /// Apply template with variables
    pub fn apply_template(&mut self, manager: &mut SystemPromptManager) -> Result<String, RuffError> {
        if let Some(template) = self.get_selected_template().cloned() {
            let result = manager.apply_template(template.id, &self.variables)?;
            self.success_message = Some("Template applied successfully".to_string());
            Ok(result)
        } else {
            Err(RuffError::App("No template selected".to_string()))
        }
    }
    
    /// Get the final prompt (either custom or applied template)
    pub fn get_final_prompt(&self) -> String {
        match self.mode {
            SystemPromptMode::CustomEdit => self.current_prompt.clone(),
            _ => {
                if let Some(template) = self.get_selected_template().cloned() {
                    let mut content = template.content.clone();
                    for (key, value) in &self.variables {
                        let placeholder = format!("{{{{{}}}}}", key);
                        content = content.replace(&placeholder, value);
                    }
                    content
                } else {
                    self.current_prompt.clone()
                }
            }
        }
    }
    
    /// Handle text input
    pub fn handle_input(&mut self, ch: char) {
        match self.mode {
            SystemPromptMode::TemplateList => {
                self.search_query.push(ch);
                self.filter_templates();
            }
            SystemPromptMode::CustomEdit | SystemPromptMode::TemplateEdit => {
                self.current_prompt.insert(self.cursor_position, ch);
                self.cursor_position += 1;
            }
            SystemPromptMode::VariableInput => {
                if let Some(var_name) = self.get_current_variable_name() {
                    if let Some(value) = self.variables.get_mut(&var_name) {
                        value.push(ch);
                    }
                }
            }
            _ => {}
        }
    }
    
    /// Handle backspace
    pub fn handle_backspace(&mut self) {
        match self.mode {
            SystemPromptMode::TemplateList => {
                if !self.search_query.is_empty() {
                    self.search_query.pop();
                    self.filter_templates();
                }
            }
            SystemPromptMode::CustomEdit | SystemPromptMode::TemplateEdit => {
                if self.cursor_position > 0 {
                    self.current_prompt.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            }
            SystemPromptMode::VariableInput => {
                if let Some(var_name) = self.get_current_variable_name() {
                    if let Some(value) = self.variables.get_mut(&var_name) {
                        if !value.is_empty() {
                            value.pop();
                        }
                    }
                }
            }
            _ => {}
        }
    }
    
    /// Move to next variable input
    pub fn next_variable(&mut self) {
        if self.mode == SystemPromptMode::VariableInput {
            let var_count = self.variables.len();
            if var_count > 0 {
                self.variable_input_index = (self.variable_input_index + 1) % var_count;
            }
        }
    }
    
    /// Move to previous variable input
    pub fn prev_variable(&mut self) {
        if self.mode == SystemPromptMode::VariableInput {
            let var_count = self.variables.len();
            if var_count > 0 {
                self.variable_input_index = if self.variable_input_index == 0 {
                    var_count - 1
                } else {
                    self.variable_input_index - 1
                };
            }
        }
    }
    
    /// Get current variable name for input
    fn get_current_variable_name(&self) -> Option<String> {
        let var_names: Vec<_> = self.variables.keys().cloned().collect();
        var_names.get(self.variable_input_index).cloned()
    }
    
    /// Toggle preview mode
    pub fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
    }
    
    /// Set error message
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.success_message = None;
    }
    
    /// Set success message
    pub fn set_success(&mut self, message: String) {
        self.success_message = Some(message);
        self.error_message = None;
    }
    
    /// Clear messages
    pub fn clear_messages(&mut self) {
        self.error_message = None;
        self.success_message = None;
    }
    
    /// Clear editor state
    fn clear_state(&mut self) {
        self.mode = SystemPromptMode::TemplateList;
        self.selected_template_index = 0;
        self.search_query.clear();
        self.current_prompt.clear();
        self.cursor_position = 0;
        self.scroll_offset = 0;
        self.template_scroll_offset = 0;
        self.variables.clear();
        self.variable_input_index = 0;
        self.show_preview = false;
        self.clear_messages();
    }
    
    /// Render the system prompt editor
    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.is_visible {
            return;
        }
        
        // Clear the area
        f.render_widget(Clear, area);
        
        // Main container
        let block = Block::default()
            .title("System Prompt Editor")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black));
        f.render_widget(block, area);
        
        let inner = area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 1 });
        
        match self.mode {
            SystemPromptMode::TemplateList => self.render_template_list(f, inner),
            SystemPromptMode::TemplateEdit | SystemPromptMode::CustomEdit => self.render_editor(f, inner),
            SystemPromptMode::VariableInput => self.render_variable_input(f, inner),
            SystemPromptMode::Preview => self.render_preview(f, inner),
        }
        
        // Render status messages
        self.render_messages(f, area);
    }
    
    /// Render template list
    fn render_template_list(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search
                Constraint::Min(5),    // Template list
                Constraint::Length(3), // Help
            ])
            .split(area);
        
        // Search box
        let search_text = if self.search_query.is_empty() {
            "Search templates...".to_string()
        } else {
            self.search_query.clone()
        };
        
        let search = Paragraph::new(search_text)
            .style(Style::default().fg(if self.search_query.is_empty() { 
                Color::DarkGray 
            } else { 
                Color::White 
            }))
            .block(Block::default().borders(Borders::ALL).title("Search"));
        f.render_widget(search, chunks[0]);
        
        // Template list
        let items: Vec<ListItem> = self.filtered_templates
            .iter()
            .enumerate()
            .map(|(i, &template_idx)| {
                let template = &self.templates[template_idx];
                let style = if i == self.selected_template_index {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                
                let content = vec![
                    Line::from(vec![
                        Span::styled(&template.name, Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(" - "),
                        Span::styled(template.category.to_string(), Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(Span::raw(&template.description)),
                ];
                
                ListItem::new(content).style(style)
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Templates"))
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_widget(list, chunks[1]);
        
        // Help text
        let help = Paragraph::new("↑/↓: Navigate | Enter: Select | C: Custom | Esc: Close")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Help"));
        f.render_widget(help, chunks[2]);
    }
    
    /// Render editor
    fn render_editor(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10), // Editor
                Constraint::Length(3), // Help
            ])
            .split(area);
        
        // Editor
        let title = match self.mode {
            SystemPromptMode::CustomEdit => "Custom System Prompt",
            _ => "Edit Template",
        };
        
        let editor = Paragraph::new(self.current_prompt.as_str())
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });
        f.render_widget(editor, chunks[0]);
        
        // Help text
        let help = Paragraph::new("Ctrl+S: Save | Ctrl+P: Preview | Esc: Back")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Help"));
        f.render_widget(help, chunks[1]);
    }
    
    /// Render variable input
    fn render_variable_input(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(5),    // Variables
                Constraint::Length(3), // Help
            ])
            .split(area);
        
        // Title
        let title = Paragraph::new("Fill in template variables")
            .style(Style::default().add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(title, chunks[0]);
        
        // Variables
        let var_names: Vec<_> = self.variables.keys().cloned().collect();
        let items: Vec<ListItem> = var_names
            .iter()
            .enumerate()
            .map(|(i, var_name)| {
                let empty_string = String::new();
                let value = self.variables.get(var_name).unwrap_or(&empty_string);
                let style = if i == self.variable_input_index {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                
                let content = format!("{}: {}", var_name, value);
                ListItem::new(content).style(style)
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Variables"));
        f.render_widget(list, chunks[1]);
        
        // Help text
        let help = Paragraph::new("Tab: Next variable | Enter: Apply | Esc: Back")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Help"));
        f.render_widget(help, chunks[2]);
    }
    
    /// Render preview
    fn render_preview(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10), // Preview
                Constraint::Length(3), // Help
            ])
            .split(area);
        
        let final_prompt = self.get_final_prompt();
        let preview = Paragraph::new(final_prompt)
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title("Preview"))
            .wrap(Wrap { trim: false });
        f.render_widget(preview, chunks[0]);
        
        // Help text
        let help = Paragraph::new("Enter: Apply | Esc: Back")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Help"));
        f.render_widget(help, chunks[1]);
    }
    
    /// Render status messages
    fn render_messages(&self, f: &mut Frame, area: Rect) {
        if let Some(ref error) = self.error_message {
            let error_area = Rect {
                x: area.x + 2,
                y: area.y + area.height - 4,
                width: area.width - 4,
                height: 3,
            };
            
            let error_widget = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red).bg(Color::Black))
                .block(Block::default().borders(Borders::ALL).title("Error"))
                .wrap(Wrap { trim: false });
            f.render_widget(Clear, error_area);
            f.render_widget(error_widget, error_area);
        } else if let Some(ref success) = self.success_message {
            let success_area = Rect {
                x: area.x + 2,
                y: area.y + area.height - 4,
                width: area.width - 4,
                height: 3,
            };
            
            let success_widget = Paragraph::new(success.as_str())
                .style(Style::default().fg(Color::Green).bg(Color::Black))
                .block(Block::default().borders(Borders::ALL).title("Success"))
                .wrap(Wrap { trim: false });
            f.render_widget(Clear, success_area);
            f.render_widget(success_widget, success_area);
        }
    }
}

impl Default for SystemPromptEditor {
    fn default() -> Self {
        Self::new()
    }
}