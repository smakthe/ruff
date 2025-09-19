use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use ratatui::{layout::Rect, Frame};
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

use crate::plugin::{
    traits::{UIExtension, UIPosition},
    ui_extensions::{UIExtensionManager, UIExtensionConfig},
    PluginId,
};
use crate::events::EventBus;
use crate::RuffError;

/// Test UI extension implementation
struct TestUIExtension {
    id: String,
    name: String,
    position: UIPosition,
    visible: AtomicBool,
    min_size: (u16, u16),
    render_count: AtomicUsize,
    input_handled: AtomicBool,
}

impl TestUIExtension {
    fn new(id: &str, name: &str, position: UIPosition) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            position,
            visible: AtomicBool::new(true),
            min_size: (20, 5),
            render_count: AtomicUsize::new(0),
            input_handled: AtomicBool::new(false),
        }
    }

    fn with_min_size(mut self, width: u16, height: u16) -> Self {
        self.min_size = (width, height);
        self
    }

    fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::SeqCst);
    }

    fn get_render_count(&self) -> usize {
        self.render_count.load(Ordering::SeqCst)
    }

    fn was_input_handled(&self) -> bool {
        self.input_handled.load(Ordering::SeqCst)
    }
}

impl UIExtension for TestUIExtension {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn position(&self) -> UIPosition {
        self.position.clone()
    }

    fn render(&self, _area: Rect, _frame: &mut Frame<'_>) -> Result<(), RuffError> {
        self.render_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn handle_input(&mut self, event: &CrosstermEvent) -> Result<bool, RuffError> {
        match event {
            CrosstermEvent::Key(KeyEvent { code: KeyCode::F(1), .. }) => {
                self.input_handled.store(true, Ordering::SeqCst);
                Ok(true) // Handled
            }
            _ => Ok(false), // Not handled
        }
    }

    fn is_visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }

    fn min_size(&self) -> (u16, u16) {
        self.min_size
    }
}

/// Test UI extension that always fails to render
struct FailingUIExtension {
    id: String,
    name: String,
}

impl FailingUIExtension {
    fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
        }
    }
}

impl UIExtension for FailingUIExtension {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn position(&self) -> UIPosition {
        UIPosition::Top
    }

    fn render(&self, _area: Rect, _frame: &mut Frame<'_>) -> Result<(), RuffError> {
        Err(RuffError::Plugin {
            plugin_name: "test-plugin".to_string(),
            message: "Intentional render failure".to_string(),
        })
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn min_size(&self) -> (u16, u16) {
        (10, 3)
    }
}

#[tokio::test]
async fn test_ui_extension_registration() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension = Box::new(TestUIExtension::new("test-ext", "Test Extension", UIPosition::Top));
    let result = manager.register_extension("test-plugin".to_string(), extension).await;
    
    assert!(result.is_ok());

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_extensions, 1);
    assert_eq!(stats.visible_extensions, 1);
    assert!(stats.stats_by_position.contains_key(&UIPosition::Top));
}

#[tokio::test]
async fn test_duplicate_extension_registration() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension1 = Box::new(TestUIExtension::new("test-ext", "Test Extension 1", UIPosition::Top));
    let extension2 = Box::new(TestUIExtension::new("test-ext", "Test Extension 2", UIPosition::Bottom));

    // First registration should succeed
    let result1 = manager.register_extension("plugin1".to_string(), extension1).await;
    assert!(result1.is_ok());

    // Second registration with same ID should fail
    let result2 = manager.register_extension("plugin2".to_string(), extension2).await;
    assert!(result2.is_err());

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_extensions, 1);
}

#[tokio::test]
async fn test_ui_extension_unregistration() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension = Box::new(TestUIExtension::new("test-ext", "Test Extension", UIPosition::Top));
    manager.register_extension("test-plugin".to_string(), extension).await.unwrap();

    let result = manager.unregister_extension("test-ext").await;
    assert!(result.is_ok());

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_extensions, 0);
    assert_eq!(stats.visible_extensions, 0);
}

#[tokio::test]
async fn test_unregister_nonexistent_extension() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let result = manager.unregister_extension("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_plugin_extension_cleanup() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension1 = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top));
    let extension2 = Box::new(TestUIExtension::new("ext2", "Extension 2", UIPosition::Bottom));
    let extension3 = Box::new(TestUIExtension::new("ext3", "Extension 3", UIPosition::Left));

    manager.register_extension("test-plugin".to_string(), extension1).await.unwrap();
    manager.register_extension("test-plugin".to_string(), extension2).await.unwrap();
    manager.register_extension("other-plugin".to_string(), extension3).await.unwrap();

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_extensions, 3);

    // Unregister all extensions from test-plugin
    manager.unregister_plugin_extensions(&"test-plugin".to_string()).await.unwrap();

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_extensions, 1); // Only other-plugin extension remains
}

#[tokio::test]
async fn test_layout_calculation() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension1 = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top).with_min_size(50, 10));
    let extension2 = Box::new(TestUIExtension::new("ext2", "Extension 2", UIPosition::Bottom).with_min_size(50, 8));
    let extension3 = Box::new(TestUIExtension::new("ext3", "Extension 3", UIPosition::Left).with_min_size(20, 30));

    manager.register_extension("plugin1".to_string(), extension1).await.unwrap();
    manager.register_extension("plugin2".to_string(), extension2).await.unwrap();
    manager.register_extension("plugin3".to_string(), extension3).await.unwrap();

    let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
    let config = UIExtensionConfig::default();
    
    let layout = manager.calculate_layout(total_area, &config).await.unwrap();
    
    assert_eq!(layout.total_extensions, 3);
    assert!(layout.extension_areas.contains_key("ext1"));
    assert!(layout.extension_areas.contains_key("ext2"));
    assert!(layout.extension_areas.contains_key("ext3"));
    
    // Main content area should be smaller than total area due to extensions
    assert!(layout.main_content_area.width <= total_area.width);
    assert!(layout.main_content_area.height <= total_area.height);
}

#[tokio::test]
async fn test_layout_with_invisible_extensions() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension1 = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top));
    let extension2 = Box::new(TestUIExtension::new("ext2", "Extension 2", UIPosition::Bottom));

    extension2.set_visible(false);

    manager.register_extension("plugin1".to_string(), extension1).await.unwrap();
    manager.register_extension("plugin2".to_string(), extension2).await.unwrap();

    let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
    let config = UIExtensionConfig::default();
    
    let layout = manager.calculate_layout(total_area, &config).await.unwrap();
    
    // Only visible extension should be in layout
    assert_eq!(layout.total_extensions, 1);
    assert!(layout.extension_areas.contains_key("ext1"));
    assert!(!layout.extension_areas.contains_key("ext2"));
}

#[tokio::test]
async fn test_extension_visibility_control() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension = Box::new(TestUIExtension::new("test-ext", "Test Extension", UIPosition::Top));
    manager.register_extension("test-plugin".to_string(), extension).await.unwrap();

    // Initially visible
    let stats = manager.get_stats().await;
    assert_eq!(stats.visible_extensions, 1);

    // Hide extension
    manager.set_extension_visibility("test-ext", false).await.unwrap();
    let stats = manager.get_stats().await;
    assert_eq!(stats.visible_extensions, 0);

    // Show extension again
    manager.set_extension_visibility("test-ext", true).await.unwrap();
    let stats = manager.get_stats().await;
    assert_eq!(stats.visible_extensions, 1);
}

#[tokio::test]
async fn test_extension_visibility_nonexistent() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let result = manager.set_extension_visibility("nonexistent", false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_extensions() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension1 = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top));
    let extension2 = Box::new(TestUIExtension::new("ext2", "Extension 2", UIPosition::Bottom));

    manager.register_extension("plugin1".to_string(), extension1).await.unwrap();
    manager.register_extension("plugin2".to_string(), extension2).await.unwrap();

    let extensions = manager.list_extensions().await;
    assert_eq!(extensions.len(), 2);
    
    // Extensions should be sorted by name
    assert_eq!(extensions[0].name, "Extension 1");
    assert_eq!(extensions[1].name, "Extension 2");
    
    let ext1 = extensions.iter().find(|e| e.id == "ext1").unwrap();
    assert_eq!(ext1.plugin_id, "plugin1");
    assert_eq!(ext1.position, UIPosition::Top);
    assert!(ext1.is_visible);
    assert_eq!(ext1.min_size, (20, 5));
}

#[tokio::test]
async fn test_input_handling() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension = Box::new(TestUIExtension::new("test-ext", "Test Extension", UIPosition::Top));
    manager.register_extension("test-plugin".to_string(), extension).await.unwrap();

    // Test F1 key (should be handled by extension)
    let f1_event = CrosstermEvent::Key(KeyEvent {
        code: KeyCode::F(1),
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    });

    let handled = manager.handle_input(&f1_event).await.unwrap();
    assert!(handled);

    // Test other key (should not be handled)
    let other_event = CrosstermEvent::Key(KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    });

    let handled = manager.handle_input(&other_event).await.unwrap();
    assert!(!handled);
}

#[tokio::test]
async fn test_input_handling_with_invisible_extension() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension = Box::new(TestUIExtension::new("test-ext", "Test Extension", UIPosition::Top));
    extension.set_visible(false);
    manager.register_extension("test-plugin".to_string(), extension).await.unwrap();

    let f1_event = CrosstermEvent::Key(KeyEvent {
        code: KeyCode::F(1),
        modifiers: KeyModifiers::NONE,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    });

    // Invisible extension should not handle input
    let handled = manager.handle_input(&f1_event).await.unwrap();
    assert!(!handled);
}

#[tokio::test]
async fn test_layout_with_custom_position() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Custom { x: 10, y: 5 }));
    manager.register_extension("plugin1".to_string(), extension).await.unwrap();

    let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
    let config = UIExtensionConfig::default();
    
    let layout = manager.calculate_layout(total_area, &config).await.unwrap();
    
    assert_eq!(layout.total_extensions, 1);
    assert!(layout.extension_areas.contains_key("ext1"));
    
    let ext_area = layout.extension_areas.get("ext1").unwrap();
    assert_eq!(ext_area.x, 10);
    assert_eq!(ext_area.y, 5);
}

#[tokio::test]
async fn test_layout_with_floating_position() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Floating));
    manager.register_extension("plugin1".to_string(), extension).await.unwrap();

    let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
    let config = UIExtensionConfig::default();
    
    let layout = manager.calculate_layout(total_area, &config).await.unwrap();
    
    assert_eq!(layout.total_extensions, 1);
    assert!(layout.extension_areas.contains_key("ext1"));
    
    // Floating extensions should not reduce main content area
    assert_eq!(layout.main_content_area, total_area);
}

#[tokio::test]
async fn test_layout_cache() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top));
    manager.register_extension("plugin1".to_string(), extension).await.unwrap();

    let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
    let config = UIExtensionConfig::default();
    
    // First calculation
    let start = std::time::Instant::now();
    let layout1 = manager.calculate_layout(total_area, &config).await.unwrap();
    let first_duration = start.elapsed();
    
    // Second calculation (should use cache)
    let start = std::time::Instant::now();
    let layout2 = manager.calculate_layout(total_area, &config).await.unwrap();
    let second_duration = start.elapsed();
    
    // Results should be identical
    assert_eq!(layout1.total_extensions, layout2.total_extensions);
    assert_eq!(layout1.main_content_area, layout2.main_content_area);
    
    // Second calculation should be faster (cached)
    assert!(second_duration <= first_duration);
}

#[tokio::test]
async fn test_extension_limits() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let mut config = UIExtensionConfig::default();
    config.max_extensions_per_position = 2;

    // Register 3 extensions at the same position
    for i in 0..3 {
        let extension = Box::new(TestUIExtension::new(
            &format!("ext{}", i),
            &format!("Extension {}", i),
            UIPosition::Top,
        ));
        manager.register_extension(format!("plugin{}", i), extension).await.unwrap();
    }

    let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
    let layout = manager.calculate_layout(total_area, &config).await.unwrap();
    
    // Only 2 extensions should be in the layout due to the limit
    assert_eq!(layout.total_extensions, 2);
}

#[tokio::test]
async fn test_min_main_content_ratio() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let mut config = UIExtensionConfig::default();
    config.min_main_content_ratio = 0.8; // 80% minimum

    // Register large extensions that would normally take up most space
    let extension1 = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top).with_min_size(100, 20));
    let extension2 = Box::new(TestUIExtension::new("ext2", "Extension 2", UIPosition::Bottom).with_min_size(100, 20));

    manager.register_extension("plugin1".to_string(), extension1).await.unwrap();
    manager.register_extension("plugin2".to_string(), extension2).await.unwrap();

    let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
    let layout = manager.calculate_layout(total_area, &config).await.unwrap();
    
    // Main content area should respect minimum ratio
    let min_width = (total_area.width as f32 * config.min_main_content_ratio) as u16;
    let min_height = (total_area.height as f32 * config.min_main_content_ratio) as u16;
    
    assert!(layout.main_content_area.width >= min_width);
    assert!(layout.main_content_area.height >= min_height);
}

#[tokio::test]
async fn test_stats_by_position() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let extension1 = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top));
    let extension2 = Box::new(TestUIExtension::new("ext2", "Extension 2", UIPosition::Top));
    let extension3 = Box::new(TestUIExtension::new("ext3", "Extension 3", UIPosition::Bottom));

    extension2.set_visible(false);

    manager.register_extension("plugin1".to_string(), extension1).await.unwrap();
    manager.register_extension("plugin2".to_string(), extension2).await.unwrap();
    manager.register_extension("plugin3".to_string(), extension3).await.unwrap();

    let stats = manager.get_stats().await;
    
    assert_eq!(stats.total_extensions, 3);
    assert_eq!(stats.visible_extensions, 2);
    
    let top_stats = stats.stats_by_position.get(&UIPosition::Top).unwrap();
    assert_eq!(top_stats.total_extensions, 2);
    assert_eq!(top_stats.visible_extensions, 1);
    
    let bottom_stats = stats.stats_by_position.get(&UIPosition::Bottom).unwrap();
    assert_eq!(bottom_stats.total_extensions, 1);
    assert_eq!(bottom_stats.visible_extensions, 1);
}

#[tokio::test]
async fn test_render_error_handling() {
    let event_bus = Arc::new(EventBus::new());
    let manager = UIExtensionManager::new(event_bus);

    let failing_extension = Box::new(FailingUIExtension::new("failing-ext", "Failing Extension"));
    let working_extension = Box::new(TestUIExtension::new("working-ext", "Working Extension", UIPosition::Bottom));

    manager.register_extension("failing-plugin".to_string(), failing_extension).await.unwrap();
    manager.register_extension("working-plugin".to_string(), working_extension).await.unwrap();

    let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
    let config = UIExtensionConfig::default();
    
    let layout = manager.calculate_layout(total_area, &config).await.unwrap();
    assert_eq!(layout.total_extensions, 2);

    // Test that render_extensions handles errors gracefully
    // We can't easily test the actual rendering without a complex setup,
    // but we can verify that the method doesn't panic with failing extensions
    
    // Create a simple mock frame - we'll just verify the method can be called
    use ratatui::{backend::TestBackend, Terminal};
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    
    // The render method should handle errors gracefully and not panic
    let render_result = terminal.draw(|f| {
        // We can't use block_on here as we're already in an async context
        // Instead, we'll just verify the layout was calculated correctly
        assert_eq!(layout.total_extensions, 2);
    });
    
    assert!(render_result.is_ok());
}