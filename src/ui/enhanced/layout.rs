//! Layout management for resizable UI components
//! 
//! This module provides dynamic pane sizing, mouse and keyboard-based resizing,
//! and layout persistence functionality.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use ratatui::layout::Rect;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use crate::RuffError;

/// Unique identifier for panes
pub type PaneId = String;

/// Size constraints for panes
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

/// Pane configuration with constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneConfig {
    pub id: PaneId,
    pub min_size: Size,
    pub max_size: Option<Size>,
    pub preferred_size: Size,
    pub is_resizable: bool,
    pub resize_direction: ResizeDirection,
}

/// Direction in which a pane can be resized
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ResizeDirection {
    Horizontal,
    Vertical,
    Both,
    None,
}

/// Layout configuration that can be persisted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    pub pane_configs: HashMap<PaneId, PaneConfig>,
    pub font_size: u16,
    pub layout_version: u32,
}

/// Resizable pane with runtime state
#[derive(Debug, Clone)]
pub struct ResizablePane {
    pub config: PaneConfig,
    pub current_size: Size,
    pub area: Rect,
    pub is_focused: bool,
    pub is_being_resized: bool,
}

/// Layout manager for dynamic pane sizing
pub struct LayoutManager {
    panes: HashMap<PaneId, ResizablePane>,
    layout_config: LayoutConfig,
    focused_pane: Option<PaneId>,
    resize_mode: bool,
    resize_target: Option<PaneId>,
    last_mouse_pos: Option<(u16, u16)>,
    config_file_path: Option<std::path::PathBuf>,
}

/// Layout calculation result
#[derive(Debug)]
pub struct LayoutResult {
    pub pane_areas: HashMap<PaneId, Rect>,
    pub total_area: Rect,
}

impl Default for Size {
    fn default() -> Self {
        Self {
            width: 80,
            height: 24,
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            pane_configs: HashMap::new(),
            font_size: 14,
            layout_version: 1,
        }
    }
}

impl PaneConfig {
    pub fn new(id: PaneId) -> Self {
        Self {
            id,
            min_size: Size { width: 10, height: 3 },
            max_size: None,
            preferred_size: Size { width: 80, height: 24 },
            is_resizable: true,
            resize_direction: ResizeDirection::Both,
        }
    }

    pub fn with_min_size(mut self, width: u16, height: u16) -> Self {
        self.min_size = Size { width, height };
        self
    }

    pub fn with_max_size(mut self, width: u16, height: u16) -> Self {
        self.max_size = Some(Size { width, height });
        self
    }

    pub fn with_preferred_size(mut self, width: u16, height: u16) -> Self {
        self.preferred_size = Size { width, height };
        self
    }

    pub fn with_resize_direction(mut self, direction: ResizeDirection) -> Self {
        self.resize_direction = direction;
        self
    }

    pub fn non_resizable(mut self) -> Self {
        self.is_resizable = false;
        self.resize_direction = ResizeDirection::None;
        self
    }
}

impl ResizablePane {
    pub fn new(config: PaneConfig) -> Self {
        Self {
            current_size: config.preferred_size,
            area: Rect::default(),
            is_focused: false,
            is_being_resized: false,
            config,
        }
    }

    pub fn can_resize(&self, direction: ResizeDirection) -> bool {
        if !self.config.is_resizable {
            return false;
        }

        match (self.config.resize_direction, direction) {
            (ResizeDirection::Both, _) => true,
            (ResizeDirection::Horizontal, ResizeDirection::Horizontal) => true,
            (ResizeDirection::Vertical, ResizeDirection::Vertical) => true,
            (dir, req_dir) if dir == req_dir => true,
            _ => false,
        }
    }

    pub fn resize(&mut self, delta_width: i16, delta_height: i16) -> Result<(), RuffError> {
        if !self.config.is_resizable {
            return Err(RuffError::App("Pane is not resizable".to_string()));
        }

        let new_width = (self.current_size.width as i16 + delta_width).max(self.config.min_size.width as i16) as u16;
        let new_height = (self.current_size.height as i16 + delta_height).max(self.config.min_size.height as i16) as u16;

        // Apply max size constraints if set
        let final_width = if let Some(max_size) = self.config.max_size {
            new_width.min(max_size.width)
        } else {
            new_width
        };

        let final_height = if let Some(max_size) = self.config.max_size {
            new_height.min(max_size.height)
        } else {
            new_height
        };

        self.current_size = Size {
            width: final_width,
            height: final_height,
        };

        Ok(())
    }
}

impl LayoutManager {
    pub fn new() -> Self {
        Self {
            panes: HashMap::new(),
            layout_config: LayoutConfig::default(),
            focused_pane: None,
            resize_mode: false,
            resize_target: None,
            last_mouse_pos: None,
            config_file_path: None,
        }
    }

    pub fn with_config_file<P: Into<std::path::PathBuf>>(mut self, path: P) -> Self {
        self.config_file_path = Some(path.into());
        self
    }

    /// Add a new pane to the layout
    pub fn add_pane(&mut self, config: PaneConfig) -> Result<(), RuffError> {
        let pane_id = config.id.clone();
        let pane = ResizablePane::new(config.clone());
        
        self.panes.insert(pane_id.clone(), pane);
        self.layout_config.pane_configs.insert(pane_id, config);
        
        Ok(())
    }

    /// Remove a pane from the layout
    pub fn remove_pane(&mut self, pane_id: &PaneId) -> Result<(), RuffError> {
        self.panes.remove(pane_id);
        self.layout_config.pane_configs.remove(pane_id);
        
        if self.focused_pane.as_ref() == Some(pane_id) {
            self.focused_pane = None;
        }
        
        Ok(())
    }

    /// Get a pane by ID
    pub fn get_pane(&self, pane_id: &PaneId) -> Option<&ResizablePane> {
        self.panes.get(pane_id)
    }

    /// Get a mutable pane by ID
    pub fn get_pane_mut(&mut self, pane_id: &PaneId) -> Option<&mut ResizablePane> {
        self.panes.get_mut(pane_id)
    }

    /// Set focus to a specific pane
    pub fn focus_pane(&mut self, pane_id: &PaneId) -> Result<(), RuffError> {
        if !self.panes.contains_key(pane_id) {
            return Err(RuffError::App(format!("Pane '{}' not found", pane_id)));
        }

        // Remove focus from current pane
        if let Some(current_focused) = &self.focused_pane {
            if let Some(pane) = self.panes.get_mut(current_focused) {
                pane.is_focused = false;
            }
        }

        // Set focus to new pane
        if let Some(pane) = self.panes.get_mut(pane_id) {
            pane.is_focused = true;
        }

        self.focused_pane = Some(pane_id.clone());
        Ok(())
    }

    /// Get the currently focused pane ID
    pub fn get_focused_pane(&self) -> Option<&PaneId> {
        self.focused_pane.as_ref()
    }

    /// Calculate layout for all panes within the given area
    pub fn calculate_layout(&mut self, total_area: Rect) -> Result<LayoutResult, RuffError> {
        let mut pane_areas = HashMap::new();

        if self.panes.is_empty() {
            return Ok(LayoutResult {
                pane_areas,
                total_area,
            });
        }

        // For now, implement a simple vertical stack layout
        // In a more complex implementation, this would support various layout algorithms
        let pane_count = self.panes.len();
        let height_per_pane = total_area.height / pane_count as u16;
        
        let mut current_y = total_area.y;
        
        for (i, (pane_id, pane)) in self.panes.iter_mut().enumerate() {
            let is_last = i == pane_count - 1;
            let pane_height = if is_last {
                // Give remaining height to last pane
                total_area.y + total_area.height - current_y
            } else {
                height_per_pane.max(pane.config.min_size.height)
            };

            let pane_area = Rect {
                x: total_area.x,
                y: current_y,
                width: total_area.width,
                height: pane_height,
            };

            pane.area = pane_area;
            pane.current_size = Size {
                width: pane_area.width,
                height: pane_area.height,
            };

            pane_areas.insert(pane_id.clone(), pane_area);
            current_y += pane_height;
        }

        Ok(LayoutResult {
            pane_areas,
            total_area,
        })
    }

    /// Handle keyboard input for layout management
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Result<bool, RuffError> {
        match key.code {
            // Toggle resize mode with Ctrl+R
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.resize_mode = !self.resize_mode;
                Ok(true)
            }
            
            // Resize focused pane with arrow keys in resize mode
            KeyCode::Up if self.resize_mode => {
                if let Some(focused_id) = self.focused_pane.clone() {
                    if let Some(pane) = self.panes.get_mut(&focused_id) {
                        pane.resize(0, -1)?;
                    }
                }
                Ok(true)
            }
            KeyCode::Down if self.resize_mode => {
                if let Some(focused_id) = self.focused_pane.clone() {
                    if let Some(pane) = self.panes.get_mut(&focused_id) {
                        pane.resize(0, 1)?;
                    }
                }
                Ok(true)
            }
            KeyCode::Left if self.resize_mode => {
                if let Some(focused_id) = self.focused_pane.clone() {
                    if let Some(pane) = self.panes.get_mut(&focused_id) {
                        pane.resize(-1, 0)?;
                    }
                }
                Ok(true)
            }
            KeyCode::Right if self.resize_mode => {
                if let Some(focused_id) = self.focused_pane.clone() {
                    if let Some(pane) = self.panes.get_mut(&focused_id) {
                        pane.resize(1, 0)?;
                    }
                }
                Ok(true)
            }

            // Exit resize mode with Escape
            KeyCode::Esc if self.resize_mode => {
                self.resize_mode = false;
                Ok(true)
            }

            _ => Ok(false),
        }
    }

    /// Handle mouse events for pane resizing
    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<bool, RuffError> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if mouse is on a pane border for resizing
                let mouse_pos = (mouse.column, mouse.row);
                
                // First, find the target pane
                let mut target_pane_id = None;
                for (pane_id, pane) in &self.panes {
                    if self.is_on_resize_border(mouse_pos, &pane.area) {
                        target_pane_id = Some(pane_id.clone());
                        break;
                    }
                }
                
                // Then update the target pane
                if let Some(pane_id) = target_pane_id {
                    if let Some(pane) = self.panes.get_mut(&pane_id) {
                        self.resize_target = Some(pane_id);
                        pane.is_being_resized = true;
                        self.last_mouse_pos = Some(mouse_pos);
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            
            MouseEventKind::Drag(MouseButton::Left) => {
                if let (Some(target_id), Some(last_pos)) = (&self.resize_target, self.last_mouse_pos) {
                    let current_pos = (mouse.column, mouse.row);
                    let delta_x = current_pos.0 as i16 - last_pos.0 as i16;
                    let delta_y = current_pos.1 as i16 - last_pos.1 as i16;
                    
                    if let Some(pane) = self.panes.get_mut(target_id) {
                        pane.resize(delta_x, delta_y)?;
                    }
                    
                    self.last_mouse_pos = Some(current_pos);
                    return Ok(true);
                }
                Ok(false)
            }
            
            MouseEventKind::Up(MouseButton::Left) => {
                if self.resize_target.is_some() {
                    // End resize operation
                    if let Some(target_id) = &self.resize_target {
                        if let Some(pane) = self.panes.get_mut(target_id) {
                            pane.is_being_resized = false;
                        }
                    }
                    self.resize_target = None;
                    self.last_mouse_pos = None;
                    return Ok(true);
                }
                Ok(false)
            }
            
            _ => Ok(false),
        }
    }

    /// Check if mouse position is on a resize border
    fn is_on_resize_border(&self, mouse_pos: (u16, u16), area: &Rect) -> bool {
        let (x, y) = mouse_pos;
        
        // Check if on right or bottom border (within 1 character)
        (x == area.x + area.width - 1 && y >= area.y && y < area.y + area.height) ||
        (y == area.y + area.height - 1 && x >= area.x && x < area.x + area.width)
    }

    /// Save layout configuration to file
    pub fn save_layout(&self) -> Result<(), RuffError> {
        if let Some(config_path) = &self.config_file_path {
            let config_json = serde_json::to_string_pretty(&self.layout_config)
                .map_err(|e| RuffError::App(format!("Failed to serialize layout config: {}", e)))?;
            
            std::fs::write(config_path, config_json)
                .map_err(|e| RuffError::App(format!("Failed to write layout config: {}", e)))?;
        }
        Ok(())
    }

    /// Load layout configuration from file
    pub fn load_layout(&mut self) -> Result<(), RuffError> {
        if let Some(config_path) = &self.config_file_path {
            if config_path.exists() {
                let config_json = std::fs::read_to_string(config_path)
                    .map_err(|e| RuffError::App(format!("Failed to read layout config: {}", e)))?;
                
                let loaded_config: LayoutConfig = serde_json::from_str(&config_json)
                    .map_err(|e| RuffError::App(format!("Failed to parse layout config: {}", e)))?;
                
                self.layout_config = loaded_config;
                
                // Recreate panes from loaded config
                self.panes.clear();
                for (pane_id, pane_config) in &self.layout_config.pane_configs {
                    let pane = ResizablePane::new(pane_config.clone());
                    self.panes.insert(pane_id.clone(), pane);
                }
            }
        }
        Ok(())
    }

    /// Get current font size
    pub fn get_font_size(&self) -> u16 {
        self.layout_config.font_size
    }

    /// Set font size
    pub fn set_font_size(&mut self, size: u16) -> Result<(), RuffError> {
        if size < 8 || size > 72 {
            return Err(RuffError::App("Font size must be between 8 and 72".to_string()));
        }
        
        self.layout_config.font_size = size;
        Ok(())
    }

    /// Check if currently in resize mode
    pub fn is_resize_mode(&self) -> bool {
        self.resize_mode
    }

    /// Get all pane IDs
    pub fn get_pane_ids(&self) -> Vec<PaneId> {
        self.panes.keys().cloned().collect()
    }

    /// Get layout statistics
    pub fn get_layout_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        
        stats.insert("pane_count".to_string(), serde_json::Value::Number(self.panes.len().into()));
        stats.insert("font_size".to_string(), serde_json::Value::Number(self.layout_config.font_size.into()));
        stats.insert("resize_mode".to_string(), serde_json::Value::Bool(self.resize_mode));
        stats.insert("focused_pane".to_string(), 
            serde_json::Value::String(self.focused_pane.clone().unwrap_or_else(|| "none".to_string())));
        
        stats
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;
    use tempfile::tempdir;

    fn create_test_pane_config(id: &str) -> PaneConfig {
        PaneConfig::new(id.to_string())
            .with_min_size(10, 3)
            .with_preferred_size(40, 12)
    }

    #[test]
    fn test_layout_manager_creation() {
        let manager = LayoutManager::new();
        assert_eq!(manager.get_pane_ids().len(), 0);
        assert_eq!(manager.get_focused_pane(), None);
        assert!(!manager.is_resize_mode());
    }

    #[test]
    fn test_add_and_remove_panes() {
        let mut manager = LayoutManager::new();
        
        // Add panes
        let config1 = create_test_pane_config("pane1");
        let config2 = create_test_pane_config("pane2");
        
        assert!(manager.add_pane(config1).is_ok());
        assert!(manager.add_pane(config2).is_ok());
        
        assert_eq!(manager.get_pane_ids().len(), 2);
        assert!(manager.get_pane(&"pane1".to_string()).is_some());
        assert!(manager.get_pane(&"pane2".to_string()).is_some());
        
        // Remove pane
        assert!(manager.remove_pane(&"pane1".to_string()).is_ok());
        assert_eq!(manager.get_pane_ids().len(), 1);
        assert!(manager.get_pane(&"pane1".to_string()).is_none());
        assert!(manager.get_pane(&"pane2".to_string()).is_some());
    }

    #[test]
    fn test_pane_focus_management() {
        let mut manager = LayoutManager::new();
        let config = create_test_pane_config("test_pane");
        
        manager.add_pane(config).unwrap();
        
        // Initially no focus
        assert_eq!(manager.get_focused_pane(), None);
        
        // Set focus
        assert!(manager.focus_pane(&"test_pane".to_string()).is_ok());
        assert_eq!(manager.get_focused_pane(), Some(&"test_pane".to_string()));
        
        let pane = manager.get_pane(&"test_pane".to_string()).unwrap();
        assert!(pane.is_focused);
        
        // Focus non-existent pane should fail
        assert!(manager.focus_pane(&"non_existent".to_string()).is_err());
    }

    #[test]
    fn test_pane_resizing() {
        let mut pane = ResizablePane::new(create_test_pane_config("test"));
        
        let original_size = pane.current_size;
        
        // Test resize
        assert!(pane.resize(5, -2).is_ok());
        assert_eq!(pane.current_size.width, original_size.width + 5);
        assert_eq!(pane.current_size.height, original_size.height - 2);
        
        // Test minimum size constraint
        assert!(pane.resize(-100, -100).is_ok());
        assert!(pane.current_size.width >= pane.config.min_size.width);
        assert!(pane.current_size.height >= pane.config.min_size.height);
    }

    #[test]
    fn test_pane_resize_constraints() {
        let config = PaneConfig::new("test".to_string())
            .with_min_size(20, 10)
            .with_max_size(100, 50)
            .with_preferred_size(50, 25);
        
        let mut pane = ResizablePane::new(config);
        
        // Test max size constraint
        assert!(pane.resize(100, 100).is_ok());
        assert!(pane.current_size.width <= 100);
        assert!(pane.current_size.height <= 50);
        
        // Test min size constraint
        assert!(pane.resize(-100, -100).is_ok());
        assert!(pane.current_size.width >= 20);
        assert!(pane.current_size.height >= 10);
    }

    #[test]
    fn test_non_resizable_pane() {
        let config = PaneConfig::new("test".to_string()).non_resizable();
        let mut pane = ResizablePane::new(config);
        
        assert!(!pane.config.is_resizable);
        assert_eq!(pane.config.resize_direction, ResizeDirection::None);
        
        // Resize should fail
        assert!(pane.resize(10, 10).is_err());
    }

    #[test]
    fn test_resize_direction_constraints() {
        let horizontal_config = PaneConfig::new("h_pane".to_string())
            .with_resize_direction(ResizeDirection::Horizontal);
        let vertical_config = PaneConfig::new("v_pane".to_string())
            .with_resize_direction(ResizeDirection::Vertical);
        
        let h_pane = ResizablePane::new(horizontal_config);
        let v_pane = ResizablePane::new(vertical_config);
        
        assert!(h_pane.can_resize(ResizeDirection::Horizontal));
        assert!(!h_pane.can_resize(ResizeDirection::Vertical));
        
        assert!(!v_pane.can_resize(ResizeDirection::Horizontal));
        assert!(v_pane.can_resize(ResizeDirection::Vertical));
    }

    #[test]
    fn test_layout_calculation() {
        let mut manager = LayoutManager::new();
        
        // Add test panes
        manager.add_pane(create_test_pane_config("pane1")).unwrap();
        manager.add_pane(create_test_pane_config("pane2")).unwrap();
        manager.add_pane(create_test_pane_config("pane3")).unwrap();
        
        let total_area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 60,
        };
        
        let result = manager.calculate_layout(total_area).unwrap();
        
        assert_eq!(result.pane_areas.len(), 3);
        assert_eq!(result.total_area, total_area);
        
        // Check that all panes have areas
        assert!(result.pane_areas.contains_key("pane1"));
        assert!(result.pane_areas.contains_key("pane2"));
        assert!(result.pane_areas.contains_key("pane3"));
        
        // Check that areas don't overlap and fill the total area
        let mut total_height = 0;
        for area in result.pane_areas.values() {
            total_height += area.height;
            assert_eq!(area.width, total_area.width);
        }
        assert_eq!(total_height, total_area.height);
    }

    #[test]
    fn test_keyboard_resize_mode() {
        let mut manager = LayoutManager::new();
        manager.add_pane(create_test_pane_config("test")).unwrap();
        manager.focus_pane(&"test".to_string()).unwrap();
        
        // Toggle resize mode
        let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert!(manager.handle_key_event(ctrl_r).unwrap());
        assert!(manager.is_resize_mode());
        
        // Test resize with arrow keys
        let up_key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert!(manager.handle_key_event(up_key).unwrap());
        
        let down_key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert!(manager.handle_key_event(down_key).unwrap());
        
        // Exit resize mode
        let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(manager.handle_key_event(esc_key).unwrap());
        assert!(!manager.is_resize_mode());
    }

    #[test]
    fn test_mouse_resize_detection() {
        let manager = LayoutManager::new();
        
        let area = Rect {
            x: 10,
            y: 10,
            width: 50,
            height: 20,
        };
        
        // Test border detection
        assert!(manager.is_on_resize_border((59, 15), &area)); // Right border
        assert!(manager.is_on_resize_border((30, 29), &area)); // Bottom border
        assert!(!manager.is_on_resize_border((30, 15), &area)); // Inside
        assert!(!manager.is_on_resize_border((5, 15), &area)); // Outside
    }

    #[test]
    fn test_font_size_management() {
        let mut manager = LayoutManager::new();
        
        // Default font size
        assert_eq!(manager.get_font_size(), 14);
        
        // Set valid font size
        assert!(manager.set_font_size(16).is_ok());
        assert_eq!(manager.get_font_size(), 16);
        
        // Test bounds
        assert!(manager.set_font_size(7).is_err()); // Too small
        assert!(manager.set_font_size(73).is_err()); // Too large
        
        // Edge cases
        assert!(manager.set_font_size(8).is_ok()); // Min valid
        assert!(manager.set_font_size(72).is_ok()); // Max valid
    }

    #[test]
    fn test_layout_persistence() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("layout.json");
        
        // Create manager with config file
        let mut manager = LayoutManager::new().with_config_file(&config_path);
        
        // Add panes and set font size
        manager.add_pane(create_test_pane_config("pane1")).unwrap();
        manager.set_font_size(18).unwrap();
        
        // Save layout
        assert!(manager.save_layout().is_ok());
        assert!(config_path.exists());
        
        // Create new manager and load layout
        let mut new_manager = LayoutManager::new().with_config_file(&config_path);
        assert!(new_manager.load_layout().is_ok());
        
        // Verify loaded data
        assert_eq!(new_manager.get_font_size(), 18);
        assert_eq!(new_manager.get_pane_ids().len(), 1);
        assert!(new_manager.get_pane(&"pane1".to_string()).is_some());
    }

    #[test]
    fn test_layout_stats() {
        let mut manager = LayoutManager::new();
        manager.add_pane(create_test_pane_config("pane1")).unwrap();
        manager.add_pane(create_test_pane_config("pane2")).unwrap();
        manager.focus_pane(&"pane1".to_string()).unwrap();
        manager.set_font_size(20).unwrap();
        
        let stats = manager.get_layout_stats();
        
        assert_eq!(stats.get("pane_count").unwrap().as_u64().unwrap(), 2);
        assert_eq!(stats.get("font_size").unwrap().as_u64().unwrap(), 20);
        assert_eq!(stats.get("resize_mode").unwrap().as_bool().unwrap(), false);
        assert_eq!(stats.get("focused_pane").unwrap().as_str().unwrap(), "pane1");
    }

    #[test]
    fn test_empty_layout_calculation() {
        let mut manager = LayoutManager::new();
        
        let total_area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 60,
        };
        
        let result = manager.calculate_layout(total_area).unwrap();
        
        assert_eq!(result.pane_areas.len(), 0);
        assert_eq!(result.total_area, total_area);
    }

    #[test]
    fn test_pane_config_builder() {
        let config = PaneConfig::new("test".to_string())
            .with_min_size(15, 5)
            .with_max_size(200, 100)
            .with_preferred_size(80, 40)
            .with_resize_direction(ResizeDirection::Horizontal)
            .non_resizable();
        
        assert_eq!(config.id, "test");
        assert_eq!(config.min_size.width, 15);
        assert_eq!(config.min_size.height, 5);
        assert_eq!(config.max_size.unwrap().width, 200);
        assert_eq!(config.max_size.unwrap().height, 100);
        assert_eq!(config.preferred_size.width, 80);
        assert_eq!(config.preferred_size.height, 40);
        assert!(!config.is_resizable);
        assert_eq!(config.resize_direction, ResizeDirection::None);
    }
}