use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::{
    config::{ParameterManager, ParameterPreset, ParameterRanges},
    session::manager::ModelConfig,
    RuffError,
};

/// Parameter adjustment UI state
pub struct ParameterAdjustment {
    pub is_visible: bool,
    pub mode: ParameterMode,
    pub selected_parameter: usize,
    pub selected_preset: usize,
    pub current_config: ModelConfig,
    pub presets: Vec<ParameterPreset>,
    pub filtered_presets: Vec<usize>,
    pub parameter_names: Vec<String>,
    pub ranges: ParameterRanges,
    pub adjustment_step: f32,
    pub show_descriptions: bool,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
}

/// Parameter adjustment modes
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterMode {
    ParameterList,
    PresetList,
    ParameterEdit,
    PresetPreview,
}

impl ParameterAdjustment {
    /// Create a new parameter adjustment UI
    pub fn new() -> Self {
        Self {
            is_visible: false,
            mode: ParameterMode::ParameterList,
            selected_parameter: 0,
            selected_preset: 0,
            current_config: ModelConfig::default(),
            presets: Vec::new(),
            filtered_presets: Vec::new(),
            parameter_names: vec![
                "temperature".to_string(),
                "max_tokens".to_string(),
                "top_p".to_string(),
                "frequency_penalty".to_string(),
                "presence_penalty".to_string(),
            ],
            ranges: ParameterRanges {
                temperature: (0.0, 2.0),
                max_tokens: (1, 32768),
                top_p: (0.0, 1.0),
                frequency_penalty: (-2.0, 2.0),
                presence_penalty: (-2.0, 2.0),
            },
            adjustment_step: 0.1,
            show_descriptions: true,
            error_message: None,
            success_message: None,
        }
    }
    
    /// Show the parameter adjustment UI
    pub fn show(&mut self, config: ModelConfig, manager: &ParameterManager) {
        self.is_visible = true;
        self.mode = ParameterMode::ParameterList;
        self.current_config = config;
        self.ranges = manager.get_ranges().clone();
        self.refresh_presets(manager);
        self.clear_messages();
    }
    
    /// Hide the parameter adjustment UI
    pub fn hide(&mut self) {
        self.is_visible = false;
        self.clear_state();
    }
    
    /// Refresh presets from manager
    pub fn refresh_presets(&mut self, manager: &ParameterManager) {
        self.presets = manager.get_presets().into_iter().cloned().collect();
        self.filtered_presets = (0..self.presets.len()).collect();
    }
    
    /// Get current parameter value
    pub fn get_current_parameter_value(&self) -> f32 {
        match self.parameter_names.get(self.selected_parameter).map(|s| s.as_str()) {
            Some("temperature") => self.current_config.temperature,
            Some("max_tokens") => self.current_config.max_tokens as f32,
            Some("top_p") => self.current_config.top_p.unwrap_or(1.0),
            Some("frequency_penalty") => self.current_config.frequency_penalty.unwrap_or(0.0),
            Some("presence_penalty") => self.current_config.presence_penalty.unwrap_or(0.0),
            _ => 0.0,
        }
    }
    
    /// Set current parameter value
    pub fn set_current_parameter_value(&mut self, value: f32) -> Result<(), RuffError> {
        match self.parameter_names.get(self.selected_parameter).map(|s| s.as_str()) {
            Some("temperature") => {
                if value >= self.ranges.temperature.0 && value <= self.ranges.temperature.1 {
                    self.current_config.temperature = value;
                } else {
                    return Err(RuffError::App(format!(
                        "Temperature must be between {} and {}",
                        self.ranges.temperature.0, self.ranges.temperature.1
                    )));
                }
            }
            Some("max_tokens") => {
                let tokens = value as u32;
                if tokens >= self.ranges.max_tokens.0 && tokens <= self.ranges.max_tokens.1 {
                    self.current_config.max_tokens = tokens;
                } else {
                    return Err(RuffError::App(format!(
                        "Max tokens must be between {} and {}",
                        self.ranges.max_tokens.0, self.ranges.max_tokens.1
                    )));
                }
            }
            Some("top_p") => {
                if value >= self.ranges.top_p.0 && value <= self.ranges.top_p.1 {
                    self.current_config.top_p = Some(value);
                } else {
                    return Err(RuffError::App(format!(
                        "Top-p must be between {} and {}",
                        self.ranges.top_p.0, self.ranges.top_p.1
                    )));
                }
            }
            Some("frequency_penalty") => {
                if value >= self.ranges.frequency_penalty.0 && value <= self.ranges.frequency_penalty.1 {
                    self.current_config.frequency_penalty = Some(value);
                } else {
                    return Err(RuffError::App(format!(
                        "Frequency penalty must be between {} and {}",
                        self.ranges.frequency_penalty.0, self.ranges.frequency_penalty.1
                    )));
                }
            }
            Some("presence_penalty") => {
                if value >= self.ranges.presence_penalty.0 && value <= self.ranges.presence_penalty.1 {
                    self.current_config.presence_penalty = Some(value);
                } else {
                    return Err(RuffError::App(format!(
                        "Presence penalty must be between {} and {}",
                        self.ranges.presence_penalty.0, self.ranges.presence_penalty.1
                    )));
                }
            }
            _ => return Err(RuffError::App("Unknown parameter".to_string())),
        }
        Ok(())
    }
    
    /// Increase current parameter value
    pub fn increase_parameter(&mut self) -> Result<(), RuffError> {
        let current = self.get_current_parameter_value();
        let step = if self.parameter_names.get(self.selected_parameter).map(|s| s.as_str()) == Some("max_tokens") {
            100.0 // Larger step for token count
        } else {
            self.adjustment_step
        };
        self.set_current_parameter_value(current + step)
    }
    
    /// Decrease current parameter value
    pub fn decrease_parameter(&mut self) -> Result<(), RuffError> {
        let current = self.get_current_parameter_value();
        let step = if self.parameter_names.get(self.selected_parameter).map(|s| s.as_str()) == Some("max_tokens") {
            100.0 // Larger step for token count
        } else {
            self.adjustment_step
        };
        self.set_current_parameter_value(current - step)
    }
    
    /// Move parameter selection up
    pub fn move_parameter_up(&mut self) {
        if self.selected_parameter > 0 {
            self.selected_parameter -= 1;
        }
    }
    
    /// Move parameter selection down
    pub fn move_parameter_down(&mut self) {
        if self.selected_parameter + 1 < self.parameter_names.len() {
            self.selected_parameter += 1;
        }
    }
    
    /// Move preset selection up
    pub fn move_preset_up(&mut self) {
        if self.selected_preset > 0 {
            self.selected_preset -= 1;
        }
    }
    
    /// Move preset selection down
    pub fn move_preset_down(&mut self) {
        if self.selected_preset + 1 < self.filtered_presets.len() {
            self.selected_preset += 1;
        }
    }
    
    /// Apply selected preset
    pub fn apply_preset(&mut self) -> Result<(), RuffError> {
        if let Some(&preset_idx) = self.filtered_presets.get(self.selected_preset) {
            if let Some(preset) = self.presets.get(preset_idx) {
                self.current_config.temperature = preset.temperature;
                self.current_config.max_tokens = preset.max_tokens;
                self.current_config.top_p = preset.top_p;
                self.current_config.frequency_penalty = preset.frequency_penalty;
                self.current_config.presence_penalty = preset.presence_penalty;
                
                self.success_message = Some(format!("Applied preset: {}", preset.name));
                return Ok(());
            }
        }
        Err(RuffError::App("No preset selected".to_string()))
    }
    
    /// Get current configuration
    pub fn get_current_config(&self) -> &ModelConfig {
        &self.current_config
    }
    
    /// Toggle descriptions visibility
    pub fn toggle_descriptions(&mut self) {
        self.show_descriptions = !self.show_descriptions;
    }
    
    /// Get parameter description
    pub fn get_parameter_description(&self, parameter: &str) -> &'static str {
        match parameter {
            "temperature" => "Controls randomness: 0.0 = deterministic, 2.0 = very creative",
            "max_tokens" => "Maximum number of tokens to generate in the response",
            "top_p" => "Nucleus sampling: considers tokens with cumulative probability up to this value",
            "frequency_penalty" => "Reduces repetition of tokens based on their frequency in the text",
            "presence_penalty" => "Reduces repetition of tokens based on whether they appear in the text",
            _ => "Unknown parameter",
        }
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
    
    /// Clear UI state
    fn clear_state(&mut self) {
        self.mode = ParameterMode::ParameterList;
        self.selected_parameter = 0;
        self.selected_preset = 0;
        self.clear_messages();
    }
    
    /// Render the parameter adjustment UI
    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.is_visible {
            return;
        }
        
        // Clear the area
        f.render_widget(Clear, area);
        
        // Main container
        let block = Block::default()
            .title("Parameter Adjustment")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black));
        f.render_widget(block, area);
        
        let inner = area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 1 });
        
        match self.mode {
            ParameterMode::ParameterList => self.render_parameter_list(f, inner),
            ParameterMode::PresetList => self.render_preset_list(f, inner),
            ParameterMode::ParameterEdit => self.render_parameter_edit(f, inner),
            ParameterMode::PresetPreview => self.render_preset_preview(f, inner),
        }
        
        // Render status messages
        self.render_messages(f, area);
    }
    
    /// Render parameter list
    fn render_parameter_list(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),    // Parameter list
                Constraint::Length(3), // Help
            ])
            .split(area);
        
        // Parameter list
        let items: Vec<ListItem> = self.parameter_names
            .iter()
            .enumerate()
            .map(|(i, param_name)| {
                let value = self.get_parameter_display_value(param_name);
                let style = if i == self.selected_parameter {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                
                let mut content = vec![
                    Line::from(vec![
                        Span::styled(param_name, Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(": "),
                        Span::styled(value, Style::default().fg(Color::Green)),
                    ]),
                ];
                
                if self.show_descriptions {
                    content.push(Line::from(Span::styled(
                        self.get_parameter_description(param_name),
                        Style::default().fg(Color::Gray),
                    )));
                }
                
                ListItem::new(content).style(style)
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Parameters"))
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_widget(list, chunks[0]);
        
        // Help text
        let help = Paragraph::new("↑/↓: Navigate | ←/→: Adjust | P: Presets | D: Toggle descriptions | Esc: Close")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Help"));
        f.render_widget(help, chunks[1]);
    }
    
    /// Render preset list
    fn render_preset_list(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),    // Preset list
                Constraint::Length(3), // Help
            ])
            .split(area);
        
        // Preset list
        let items: Vec<ListItem> = self.filtered_presets
            .iter()
            .enumerate()
            .map(|(i, &preset_idx)| {
                let preset = &self.presets[preset_idx];
                let style = if i == self.selected_preset {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                
                let content = vec![
                    Line::from(vec![
                        Span::styled(&preset.name, Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(" - "),
                        Span::styled(preset.use_case.to_string(), Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(Span::raw(&preset.description)),
                    Line::from(Span::styled(
                        format!("T:{:.1} MT:{} TP:{:.1}", 
                            preset.temperature, 
                            preset.max_tokens,
                            preset.top_p.unwrap_or(1.0)
                        ),
                        Style::default().fg(Color::Gray),
                    )),
                ];
                
                ListItem::new(content).style(style)
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Presets"))
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_widget(list, chunks[0]);
        
        // Help text
        let help = Paragraph::new("↑/↓: Navigate | Enter: Apply | Esc: Back")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Help"));
        f.render_widget(help, chunks[1]);
    }
    
    /// Render parameter edit mode
    fn render_parameter_edit(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Parameter info
                Constraint::Length(3), // Value gauge
                Constraint::Length(3), // Help
            ])
            .split(area);
        
        // Parameter info
        if let Some(param_name) = self.parameter_names.get(self.selected_parameter) {
            let current_value = self.get_current_parameter_value();
            let info = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("Parameter: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(param_name),
                ]),
                Line::from(vec![
                    Span::styled("Current Value: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        self.get_parameter_display_value(param_name),
                        Style::default().fg(Color::Green),
                    ),
                ]),
                Line::from(Span::raw(self.get_parameter_description(param_name))),
            ])
            .block(Block::default().borders(Borders::ALL).title("Parameter Details"));
            f.render_widget(info, chunks[0]);
            
            // Value gauge
            let (min_val, max_val) = self.get_parameter_range(param_name);
            let ratio = if max_val > min_val {
                ((current_value - min_val) / (max_val - min_val)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("Value"))
                .gauge_style(Style::default().fg(Color::Green))
                .ratio(ratio as f64)
                .label(format!("{:.3}", current_value));
            f.render_widget(gauge, chunks[1]);
        }
        
        // Help text
        let help = Paragraph::new("←/→: Adjust | Enter: Confirm | Esc: Back")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Help"));
        f.render_widget(help, chunks[2]);
    }
    
    /// Render preset preview
    fn render_preset_preview(&mut self, f: &mut Frame, area: Rect) {
        // Implementation for preset preview
        let preview = Paragraph::new("Preset preview not implemented yet")
            .block(Block::default().borders(Borders::ALL).title("Preset Preview"));
        f.render_widget(preview, area);
    }
    
    /// Get parameter display value
    fn get_parameter_display_value(&self, param_name: &str) -> String {
        match param_name {
            "temperature" => format!("{:.2}", self.current_config.temperature),
            "max_tokens" => self.current_config.max_tokens.to_string(),
            "top_p" => format!("{:.2}", self.current_config.top_p.unwrap_or(1.0)),
            "frequency_penalty" => format!("{:.2}", self.current_config.frequency_penalty.unwrap_or(0.0)),
            "presence_penalty" => format!("{:.2}", self.current_config.presence_penalty.unwrap_or(0.0)),
            _ => "N/A".to_string(),
        }
    }
    
    /// Get parameter range
    fn get_parameter_range(&self, param_name: &str) -> (f32, f32) {
        match param_name {
            "temperature" => self.ranges.temperature,
            "max_tokens" => (self.ranges.max_tokens.0 as f32, self.ranges.max_tokens.1 as f32),
            "top_p" => self.ranges.top_p,
            "frequency_penalty" => self.ranges.frequency_penalty,
            "presence_penalty" => self.ranges.presence_penalty,
            _ => (0.0, 1.0),
        }
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

impl Default for ParameterAdjustment {
    fn default() -> Self {
        Self::new()
    }
}