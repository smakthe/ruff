use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use ratatui::{layout::Rect, Frame};
use crossterm::event::Event as CrosstermEvent;

use crate::plugin::{
    traits::{UIExtension, UIPosition},
    PluginId,
};
use crate::events::{AppEvent, EventBus};
use crate::EnhancedError;

/// Manages UI extensions from plugins
pub struct UIExtensionManager {
    /// Extensions organized by position
    extensions_by_position: Arc<RwLock<HashMap<UIPosition, Vec<ExtensionInstance>>>>,
    /// All extensions by ID for quick lookup
    extensions_by_id: Arc<RwLock<HashMap<String, ExtensionInstance>>>,
    /// Event bus for communication
    event_bus: Arc<EventBus>,
    /// Layout cache for performance
    layout_cache: Arc<RwLock<Option<UILayoutCache>>>,
}

/// Instance of a UI extension with metadata
#[derive(Clone)]
struct ExtensionInstance {
    extension: Arc<RwLock<Box<dyn UIExtension>>>,
    plugin_id: PluginId,
    position: UIPosition,
    is_visible: bool,
    min_size: (u16, u16),
    #[allow(dead_code)] // Future functionality
    allocated_area: Option<Rect>,
}

/// Cached layout information for UI extensions
#[derive(Clone)]
struct UILayoutCache {
    total_area: Rect,
    extension_areas: HashMap<String, Rect>,
    main_content_area: Rect,
    last_calculated: std::time::Instant,
}

/// Configuration for UI extension rendering
#[derive(Debug, Clone)]
pub struct UIExtensionConfig {
    pub max_extensions_per_position: usize,
    pub min_main_content_ratio: f32,
    pub extension_spacing: u16,
    pub enable_floating_extensions: bool,
}

impl Default for UIExtensionConfig {
    fn default() -> Self {
        Self {
            max_extensions_per_position: 5,
            min_main_content_ratio: 0.6,
            extension_spacing: 1,
            enable_floating_extensions: true,
        }
    }
}

/// Result of UI extension layout calculation
#[derive(Debug, Clone)]
pub struct UIExtensionLayout {
    pub main_content_area: Rect,
    pub extension_areas: HashMap<String, Rect>,
    pub total_extensions: usize,
}

impl UIExtensionManager {
    /// Create a new UI extension manager
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            extensions_by_position: Arc::new(RwLock::new(HashMap::new())),
            extensions_by_id: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            layout_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a UI extension from a plugin
    pub async fn register_extension(
        &self,
        plugin_id: PluginId,
        extension: Box<dyn UIExtension>,
    ) -> Result<(), EnhancedError> {
        let extension_id = extension.id().to_string();
        let position = extension.position();
        let min_size = extension.min_size();
        let is_visible = extension.is_visible();

        // Check if extension ID is already registered
        {
            let extensions_by_id = self.extensions_by_id.read().await;
            if extensions_by_id.contains_key(&extension_id) {
                return Err(EnhancedError::plugin(format!("Extension '{}' is already registered", extension_id)));
            }
        }

        let instance = ExtensionInstance {
            extension: Arc::new(RwLock::new(extension)),
            plugin_id: plugin_id.clone(),
            position: position.clone(),
            is_visible,
            min_size,
            allocated_area: None,
        };

        // Add to position-based map
        {
            let mut extensions_by_position = self.extensions_by_position.write().await;
            extensions_by_position
                .entry(position)
                .or_insert_with(Vec::new)
                .push(instance.clone());
        }

        // Add to ID-based map
        {
            let mut extensions_by_id = self.extensions_by_id.write().await;
            extensions_by_id.insert(extension_id.clone(), instance);
        }

        // Invalidate layout cache
        {
            let mut cache = self.layout_cache.write().await;
            *cache = None;
        }

        // Publish event
        self.event_bus
            .publish(AppEvent::PluginLoaded(plugin_id.clone()))
            .await
            .map_err(|e| EnhancedError::plugin(format!("Failed to publish plugin loaded event: {}", e)))?;

        Ok(())
    }

    /// Unregister a UI extension
    pub async fn unregister_extension(&self, extension_id: &str) -> Result<(), EnhancedError> {
        let instance = {
            let mut extensions_by_id = self.extensions_by_id.write().await;
            extensions_by_id.remove(extension_id)
        };

        if let Some(instance) = instance {
            // Remove from position-based map
            {
                let mut extensions_by_position = self.extensions_by_position.write().await;
                if let Some(extensions) = extensions_by_position.get_mut(&instance.position) {
                    extensions.retain(|ext| {
                        let ext_id = {
                            let extension_guard = ext.extension.try_read();
                            match extension_guard {
                                Ok(guard) => guard.id().to_string(),
                                Err(_) => return false,
                            }
                        };
                        ext_id != extension_id
                    });

                    // Remove empty position entries
                    if extensions.is_empty() {
                        extensions_by_position.remove(&instance.position);
                    }
                }
            }

            // Invalidate layout cache
            {
                let mut cache = self.layout_cache.write().await;
                *cache = None;
            }

            // Publish event
            self.event_bus
                .publish(AppEvent::PluginUnloaded(instance.plugin_id.clone()))
                .await
                .map_err(|e| EnhancedError::plugin(format!("Failed to publish plugin unloaded event: {}", e)))?;

            Ok(())
        } else {
            Err(EnhancedError::plugin(format!("Extension '{}' not found", extension_id)))
        }
    }

    /// Unregister all extensions from a plugin
    pub async fn unregister_plugin_extensions(&self, plugin_id: &PluginId) -> Result<(), EnhancedError> {
        let extension_ids: Vec<String> = {
            let extensions_by_id = self.extensions_by_id.read().await;
            extensions_by_id
                .iter()
                .filter(|(_, instance)| &instance.plugin_id == plugin_id)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for extension_id in extension_ids {
            if let Err(e) = self.unregister_extension(&extension_id).await {
                eprintln!("Failed to unregister extension {}: {}", extension_id, e);
            }
        }

        Ok(())
    }

    /// Calculate layout for UI extensions
    pub async fn calculate_layout(
        &self,
        total_area: Rect,
        config: &UIExtensionConfig,
    ) -> Result<UIExtensionLayout, EnhancedError> {
        // Check cache first
        {
            let cache = self.layout_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.total_area == total_area
                    && cached.last_calculated.elapsed().as_millis() < 100
                {
                    return Ok(UIExtensionLayout {
                        main_content_area: cached.main_content_area,
                        extension_areas: cached.extension_areas.clone(),
                        total_extensions: cached.extension_areas.len(),
                    });
                }
            }
        }

        let mut extension_areas = HashMap::new();
        let mut remaining_area = total_area;

        let extensions_by_position = self.extensions_by_position.read().await;

        // Calculate areas for each position
        for (position, extensions) in extensions_by_position.iter() {
            let visible_extensions: Vec<_> = extensions
                .iter()
                .filter(|ext| ext.is_visible) // Use manager's visibility state
                .take(config.max_extensions_per_position)
                .collect();

            if visible_extensions.is_empty() {
                continue;
            }

            let (allocated_area, new_remaining) = self
                .allocate_area_for_position(
                    position,
                    &visible_extensions,
                    remaining_area,
                    config,
                )
                .await?;

            // Distribute area among extensions at this position
            let area_per_extension = self
                .distribute_area_among_extensions(&visible_extensions, allocated_area)
                .await?;

            for (extension, area) in visible_extensions.iter().zip(area_per_extension.iter()) {
                let extension_guard = extension.extension.read().await;
                let extension_id = extension_guard.id().to_string();
                extension_areas.insert(extension_id, *area);
            }

            remaining_area = new_remaining;
        }

        // Ensure minimum main content area
        let min_main_area = Rect {
            x: total_area.x,
            y: total_area.y,
            width: (total_area.width as f32 * config.min_main_content_ratio) as u16,
            height: (total_area.height as f32 * config.min_main_content_ratio) as u16,
        };

        let main_content_area = if remaining_area.width < min_main_area.width
            || remaining_area.height < min_main_area.height
        {
            min_main_area
        } else {
            remaining_area
        };

        let layout = UIExtensionLayout {
            main_content_area,
            extension_areas: extension_areas.clone(),
            total_extensions: extension_areas.len(),
        };

        // Update cache
        {
            let mut cache = self.layout_cache.write().await;
            *cache = Some(UILayoutCache {
                total_area,
                extension_areas,
                main_content_area,
                last_calculated: std::time::Instant::now(),
            });
        }

        Ok(layout)
    }

    /// Render all visible UI extensions
    pub async fn render_extensions(
        &self,
        frame: &mut Frame<'_>,
        layout: &UIExtensionLayout,
    ) -> Result<(), EnhancedError> {
        let extensions_by_id = self.extensions_by_id.read().await;

        for (extension_id, area) in &layout.extension_areas {
            if let Some(instance) = extensions_by_id.get(extension_id) {
                if instance.is_visible { // Use manager's visibility state
                    let extension_guard = instance.extension.read().await;
                    if let Err(e) = extension_guard.render(*area, frame) {
                        eprintln!(
                            "Error rendering UI extension '{}' from plugin '{}': {}",
                            extension_id, instance.plugin_id, e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle input events for UI extensions
    pub async fn handle_input(&self, event: &CrosstermEvent) -> Result<bool, EnhancedError> {
        let extensions_by_id = self.extensions_by_id.read().await;
        
        for instance in extensions_by_id.values() {
            if instance.is_visible { // Use manager's visibility state
                let mut extension_guard = instance.extension.write().await;
                match extension_guard.handle_input(event) {
                    Ok(handled) => {
                        if handled {
                            return Ok(true);
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Error handling input in UI extension from plugin '{}': {}",
                            instance.plugin_id, e
                        );
                    }
                }
            }
        }

        Ok(false)
    }

    /// Get statistics about registered extensions
    pub async fn get_stats(&self) -> UIExtensionStats {
        let extensions_by_position = self.extensions_by_position.read().await;
        let extensions_by_id = self.extensions_by_id.read().await;

        let mut stats_by_position = HashMap::new();
        let mut visible_count = 0;

        for (position, extensions) in extensions_by_position.iter() {
            let position_stats = PositionStats {
                total_extensions: extensions.len(),
                visible_extensions: extensions.iter().filter(|ext| {
                    // Use manager's visibility state, not the extension's own state
                    ext.is_visible
                }).count(),
            };
            
            visible_count += position_stats.visible_extensions;
            stats_by_position.insert(position.clone(), position_stats);
        }

        UIExtensionStats {
            total_extensions: extensions_by_id.len(),
            visible_extensions: visible_count,
            stats_by_position,
        }
    }

    /// Set visibility of a UI extension
    pub async fn set_extension_visibility(
        &self,
        extension_id: &str,
        visible: bool,
    ) -> Result<(), EnhancedError> {
        // Update in ID-based map
        let position = {
            let mut extensions_by_id = self.extensions_by_id.write().await;

            if let Some(instance) = extensions_by_id.get_mut(extension_id) {
                instance.is_visible = visible;
                instance.position.clone()
            } else {
                return Err(EnhancedError::plugin(format!("Extension '{}' not found", extension_id)));
            }
        };

        // Update in position-based map
        {
            let mut extensions_by_position = self.extensions_by_position.write().await;
            if let Some(extensions) = extensions_by_position.get_mut(&position) {
                for instance in extensions.iter_mut() {
                    let extension_guard = instance.extension.read().await;
                    if extension_guard.id() == extension_id {
                        instance.is_visible = visible;
                        break;
                    }
                }
            }
        }
        
        // Invalidate layout cache
        {
            let mut cache = self.layout_cache.write().await;
            *cache = None;
        }
        
        Ok(())
    }

    /// Get list of all registered extensions
    pub async fn list_extensions(&self) -> Vec<UIExtensionInfo> {
        let extensions_by_id = self.extensions_by_id.read().await;
        let mut extensions = Vec::new();

        for (id, instance) in extensions_by_id.iter() {
            let extension_guard = instance.extension.read().await;
            extensions.push(UIExtensionInfo {
                id: id.clone(),
                name: extension_guard.name().to_string(),
                plugin_id: instance.plugin_id.clone(),
                position: instance.position.clone(),
                is_visible: instance.is_visible, // Use manager's visibility state
                min_size: instance.min_size,
            });
        }

        extensions.sort_by(|a, b| a.name.cmp(&b.name));
        extensions
    }

    // Private helper methods

    async fn allocate_area_for_position(
        &self,
        position: &UIPosition,
        extensions: &[&ExtensionInstance],
        available_area: Rect,
        config: &UIExtensionConfig,
    ) -> Result<(Rect, Rect), EnhancedError> {
        if extensions.is_empty() {
            return Ok((Rect::default(), available_area));
        }

        // Calculate total minimum size needed
        let total_min_width: u16 = extensions.iter().map(|ext| ext.min_size.0).sum();
        let total_min_height: u16 = extensions.iter().map(|ext| ext.min_size.1).sum();

        match position {
            UIPosition::Top => {
                let height = (total_min_height + config.extension_spacing * (extensions.len() as u16).saturating_sub(1))
                    .min(available_area.height / 3);
                
                let allocated = Rect {
                    x: available_area.x,
                    y: available_area.y,
                    width: available_area.width,
                    height,
                };
                
                let remaining = Rect {
                    x: available_area.x,
                    y: available_area.y + height,
                    width: available_area.width,
                    height: available_area.height.saturating_sub(height),
                };
                
                Ok((allocated, remaining))
            }
            UIPosition::Bottom => {
                let height = (total_min_height + config.extension_spacing * (extensions.len() as u16).saturating_sub(1))
                    .min(available_area.height / 3);
                
                let allocated = Rect {
                    x: available_area.x,
                    y: available_area.y + available_area.height.saturating_sub(height),
                    width: available_area.width,
                    height,
                };
                
                let remaining = Rect {
                    x: available_area.x,
                    y: available_area.y,
                    width: available_area.width,
                    height: available_area.height.saturating_sub(height),
                };
                
                Ok((allocated, remaining))
            }
            UIPosition::Left => {
                let width = (total_min_width + config.extension_spacing * (extensions.len() as u16).saturating_sub(1))
                    .min(available_area.width / 3);
                
                let allocated = Rect {
                    x: available_area.x,
                    y: available_area.y,
                    width,
                    height: available_area.height,
                };
                
                let remaining = Rect {
                    x: available_area.x + width,
                    y: available_area.y,
                    width: available_area.width.saturating_sub(width),
                    height: available_area.height,
                };
                
                Ok((allocated, remaining))
            }
            UIPosition::Right => {
                let width = (total_min_width + config.extension_spacing * (extensions.len() as u16).saturating_sub(1))
                    .min(available_area.width / 3);
                
                let allocated = Rect {
                    x: available_area.x + available_area.width.saturating_sub(width),
                    y: available_area.y,
                    width,
                    height: available_area.height,
                };
                
                let remaining = Rect {
                    x: available_area.x,
                    y: available_area.y,
                    width: available_area.width.saturating_sub(width),
                    height: available_area.height,
                };
                
                Ok((allocated, remaining))
            }
            UIPosition::Floating => {
                // Floating extensions don't reduce the main content area
                Ok((available_area, available_area))
            }
            UIPosition::Custom { x, y } => {
                // Custom positioned extensions get a small area at the specified coordinates
                let width = extensions.iter().map(|ext| ext.min_size.0).max().unwrap_or(20);
                let height = total_min_height + config.extension_spacing * (extensions.len() as u16).saturating_sub(1);
                
                let allocated = Rect {
                    x: (*x).min(available_area.width.saturating_sub(width)),
                    y: (*y).min(available_area.height.saturating_sub(height)),
                    width,
                    height,
                };
                
                Ok((allocated, available_area))
            }
        }
    }

    async fn distribute_area_among_extensions(
        &self,
        extensions: &[&ExtensionInstance],
        total_area: Rect,
    ) -> Result<Vec<Rect>, EnhancedError> {
        if extensions.is_empty() {
            return Ok(vec![]);
        }

        if extensions.len() == 1 {
            return Ok(vec![total_area]);
        }

        let mut areas = Vec::new();
        let area_per_extension = total_area.height / extensions.len() as u16;
        
        for (i, _extension) in extensions.iter().enumerate() {
            let y_offset = i as u16 * area_per_extension;
            let height = if i == extensions.len() - 1 {
                // Last extension gets remaining height
                total_area.height - y_offset
            } else {
                area_per_extension
            };
            
            areas.push(Rect {
                x: total_area.x,
                y: total_area.y + y_offset,
                width: total_area.width,
                height,
            });
        }

        Ok(areas)
    }
}

/// Statistics about UI extensions
#[derive(Debug, Clone)]
pub struct UIExtensionStats {
    pub total_extensions: usize,
    pub visible_extensions: usize,
    pub stats_by_position: HashMap<UIPosition, PositionStats>,
}

/// Statistics for extensions at a specific position
#[derive(Debug, Clone)]
pub struct PositionStats {
    pub total_extensions: usize,
    pub visible_extensions: usize,
}

/// Information about a registered UI extension
#[derive(Debug, Clone)]
pub struct UIExtensionInfo {
    pub id: String,
    pub name: String,
    pub plugin_id: PluginId,
    pub position: UIPosition,
    pub is_visible: bool,
    pub min_size: (u16, u16),
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::*;
    use crate::plugin::traits::{UIExtension, UIPosition};
    use async_trait::async_trait;
    use ratatui::{layout::Rect, Frame};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestUIExtension {
        id: String,
        name: String,
        position: UIPosition,
        visible: AtomicBool,
        min_size: (u16, u16),
    }

    impl TestUIExtension {
        fn new(id: &str, name: &str, position: UIPosition) -> Self {
            Self {
                id: id.to_string(),
                name: name.to_string(),
                position,
                visible: AtomicBool::new(true),
                min_size: (20, 5),
            }
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

        fn render(&self, _area: Rect, _frame: &mut Frame) -> Result<(), EnhancedError> {
            Ok(())
        }

        fn is_visible(&self) -> bool {
            self.visible.load(Ordering::SeqCst)
        }

        fn min_size(&self) -> (u16, u16) {
            self.min_size
        }
    }

    #[tokio::test]
    async fn test_extension_registration() {
        let event_bus = Arc::new(EventBus::new());
        let manager = UIExtensionManager::new(event_bus);

        let extension = Box::new(TestUIExtension::new("test-ext", "Test Extension", UIPosition::Top));
        let result = manager.register_extension("test-plugin".to_string(), extension).await;
        
        assert!(result.is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_extensions, 1);
        assert_eq!(stats.visible_extensions, 1);
    }

    #[tokio::test]
    async fn test_extension_unregistration() {
        let event_bus = Arc::new(EventBus::new());
        let manager = UIExtensionManager::new(event_bus);

        let extension = Box::new(TestUIExtension::new("test-ext", "Test Extension", UIPosition::Top));
        manager.register_extension("test-plugin".to_string(), extension).await.unwrap();

        let result = manager.unregister_extension("test-ext").await;
        assert!(result.is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_extensions, 0);
    }

    #[tokio::test]
    async fn test_layout_calculation() {
        let event_bus = Arc::new(EventBus::new());
        let manager = UIExtensionManager::new(event_bus);

        let extension1 = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top));
        let extension2 = Box::new(TestUIExtension::new("ext2", "Extension 2", UIPosition::Bottom));

        manager.register_extension("plugin1".to_string(), extension1).await.unwrap();
        manager.register_extension("plugin2".to_string(), extension2).await.unwrap();

        let total_area = Rect { x: 0, y: 0, width: 100, height: 50 };
        let config = UIExtensionConfig::default();
        
        let layout = manager.calculate_layout(total_area, &config).await.unwrap();
        
        assert_eq!(layout.total_extensions, 2);
        assert!(layout.extension_areas.contains_key("ext1"));
        assert!(layout.extension_areas.contains_key("ext2"));
        assert!(layout.main_content_area.width > 0);
        assert!(layout.main_content_area.height > 0);
    }

    #[tokio::test]
    async fn test_extension_visibility() {
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
    async fn test_plugin_extension_cleanup() {
        let event_bus = Arc::new(EventBus::new());
        let manager = UIExtensionManager::new(event_bus);

        let extension1 = Box::new(TestUIExtension::new("ext1", "Extension 1", UIPosition::Top));
        let extension2 = Box::new(TestUIExtension::new("ext2", "Extension 2", UIPosition::Bottom));

        manager.register_extension("test-plugin".to_string(), extension1).await.unwrap();
        manager.register_extension("test-plugin".to_string(), extension2).await.unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_extensions, 2);

        // Unregister all extensions from the plugin
        manager.unregister_plugin_extensions(&"test-plugin".to_string()).await.unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_extensions, 0);
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
        
        let ext1 = extensions.iter().find(|e| e.id == "ext1").unwrap();
        assert_eq!(ext1.name, "Extension 1");
        assert_eq!(ext1.plugin_id, "plugin1");
        assert_eq!(ext1.position, UIPosition::Top);
    }
}