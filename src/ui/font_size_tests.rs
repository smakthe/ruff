#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::ui::UI;
    use tempfile::tempdir;

    fn create_test_ui() -> UI {
        let config = Config::default();
        let mut ui = UI::new(config).expect("Failed to create test UI");

        // Reset to default font size to ensure consistent test state
        let _ = ui.reset_font_size();

        ui
    }

    #[test]
    fn test_default_font_size() {
        let ui = create_test_ui();
        assert_eq!(ui.get_font_size(), 14);
    }

    #[test]
    fn test_set_font_size() {
        let mut ui = create_test_ui();

        // Set valid font size
        assert!(ui.set_font_size(16).is_ok());
        assert_eq!(ui.get_font_size(), 16);

        // Set another valid font size
        assert!(ui.set_font_size(20).is_ok());
        assert_eq!(ui.get_font_size(), 20);
    }

    #[test]
    fn test_font_size_bounds() {
        let mut ui = create_test_ui();

        // Test minimum bound
        assert!(ui.set_font_size(7).is_err()); // Below minimum
        assert!(ui.set_font_size(8).is_ok()); // At minimum
        assert_eq!(ui.get_font_size(), 8);

        // Test maximum bound
        assert!(ui.set_font_size(73).is_err()); // Above maximum
        assert!(ui.set_font_size(72).is_ok()); // At maximum
        assert_eq!(ui.get_font_size(), 72);
    }

    #[test]
    fn test_increase_font_size() {
        let mut ui = create_test_ui();

        // Reset to known state first
        ui.reset_font_size().unwrap();
        assert_eq!(ui.get_font_size(), 14);

        // Increase font size
        let new_size = ui.increase_font_size().unwrap();
        assert_eq!(new_size, 15);
        assert_eq!(ui.get_font_size(), 15);

        // Increase again
        let new_size = ui.increase_font_size().unwrap();
        assert_eq!(new_size, 16);
        assert_eq!(ui.get_font_size(), 16);
    }

    #[test]
    fn test_decrease_font_size() {
        let mut ui = create_test_ui();

        // Set to a higher size first
        ui.set_font_size(20).unwrap();

        // Decrease font size
        let new_size = ui.decrease_font_size().unwrap();
        assert_eq!(new_size, 19);
        assert_eq!(ui.get_font_size(), 19);

        // Decrease again
        let new_size = ui.decrease_font_size().unwrap();
        assert_eq!(new_size, 18);
        assert_eq!(ui.get_font_size(), 18);
    }

    #[test]
    fn test_font_size_bounds_with_increase_decrease() {
        let mut ui = create_test_ui();

        // Test maximum bound with increase
        ui.set_font_size(72).unwrap();
        let new_size = ui.increase_font_size().unwrap();
        assert_eq!(new_size, 72); // Should stay at maximum
        assert_eq!(ui.get_font_size(), 72);

        // Test minimum bound with decrease
        ui.set_font_size(8).unwrap();
        let new_size = ui.decrease_font_size().unwrap();
        assert_eq!(new_size, 8); // Should stay at minimum
        assert_eq!(ui.get_font_size(), 8);
    }

    #[test]
    fn test_reset_font_size() {
        let mut ui = create_test_ui();

        // Change font size
        ui.set_font_size(24).unwrap();
        assert_eq!(ui.get_font_size(), 24);

        // Reset to default
        assert!(ui.reset_font_size().is_ok());
        assert_eq!(ui.get_font_size(), 14);
    }

    #[test]
    fn test_layout_manager_access() {
        let mut ui = create_test_ui();

        // Reset to known state first
        ui.reset_font_size().unwrap();

        // Test immutable access
        let layout_manager = ui.get_layout_manager();
        assert_eq!(layout_manager.get_font_size(), 14);

        // Test mutable access
        let layout_manager = ui.get_layout_manager_mut();
        assert!(layout_manager.set_font_size(18).is_ok());
        assert_eq!(ui.get_font_size(), 18);
    }

    #[test]
    fn test_font_size_persistence() {
        let _temp_dir = tempdir().unwrap();

        // Test that font size methods work correctly
        // Note: Actual persistence testing would require more complex setup
        // with shared config directories between UI instances
        let mut ui = create_test_ui();

        // Test setting and getting font size
        ui.set_font_size(20).unwrap();
        assert_eq!(ui.get_font_size(), 20);

        // Test that save_layout doesn't fail
        assert!(ui.get_layout_manager().save_layout().is_ok());
    }
}
