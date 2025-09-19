#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
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
        assert!(manager.get_pane("pane1").is_some());
        assert!(manager.get_pane("pane2").is_some());
        
        // Remove pane
        assert!(manager.remove_pane(&"pane1".to_string()).is_ok());
        assert_eq!(manager.get_pane_ids().len(), 1);
        assert!(manager.get_pane("pane1").is_none());
        assert!(manager.get_pane("pane2").is_some());
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
        
        let pane = manager.get_pane("test_pane").unwrap();
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
        assert!(new_manager.get_pane("pane1").is_some());
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