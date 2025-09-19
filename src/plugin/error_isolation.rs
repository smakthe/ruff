//! Plugin error isolation and recovery mechanisms
//! 
//! This module provides error isolation for plugins to prevent plugin failures
//! from affecting the main application or other plugins.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};


use crate::events::{EventBus, AppEvent, PluginId};
use crate::error::{EnhancedError, ErrorRecoveryManager};
use crate::logging::StructuredLogger;
use crate::RuffError;

/// Plugin error isolation manager
pub struct PluginErrorIsolation {
    /// Plugin health tracking
    plugin_health: Arc<RwLock<HashMap<PluginId, PluginHealth>>>,
    /// Error recovery manager
    recovery_manager: ErrorRecoveryManager,
    /// Event bus for notifications
    event_bus: EventBus,
    /// Logger for error tracking
    logger: Arc<RwLock<StructuredLogger>>,
    /// Isolation configuration
    config: IsolationConfig,
    /// Plugin quarantine status
    quarantined_plugins: Arc<RwLock<HashMap<PluginId, QuarantineInfo>>>,
}

/// Plugin health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealth {
    /// Plugin ID
    pub plugin_id: PluginId,
    /// Current health status
    pub status: HealthStatus,
    /// Error count in current window
    pub error_count: u32,
    /// Last error timestamp
    pub last_error: Option<DateTime<Local>>,
    /// Total errors since startup
    pub total_errors: u64,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Last successful operation
    pub last_success: Option<DateTime<Local>>,
    /// Performance metrics
    pub performance: PluginPerformanceMetrics,
    /// Recovery attempts
    pub recovery_attempts: u32,
    /// Plugin uptime
    pub uptime: Duration,
    /// Memory usage
    pub memory_usage: Option<u64>,
}

/// Plugin health status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Plugin is healthy and functioning normally
    Healthy,
    /// Plugin has some issues but is still functional
    Degraded,
    /// Plugin is experiencing significant problems
    Unhealthy,
    /// Plugin has been quarantined due to repeated failures
    Quarantined,
    /// Plugin has been disabled
    Disabled,
}

/// Plugin performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPerformanceMetrics {
    /// Average response time in milliseconds
    pub avg_response_time: f64,
    /// Maximum response time in milliseconds
    pub max_response_time: u64,
    /// Minimum response time in milliseconds
    pub min_response_time: u64,
    /// Total operations performed
    pub total_operations: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Success rate percentage
    pub success_rate: f64,
}

/// Quarantine information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineInfo {
    /// When the plugin was quarantined
    pub quarantined_at: DateTime<Local>,
    /// Reason for quarantine
    pub reason: String,
    /// Number of quarantine attempts
    pub quarantine_count: u32,
    /// When quarantine expires (if automatic)
    pub expires_at: Option<DateTime<Local>>,
    /// Whether manual intervention is required
    pub requires_manual_intervention: bool,
}

/// Isolation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationConfig {
    /// Maximum errors per time window before degrading health
    pub max_errors_per_window: u32,
    /// Time window for error counting (in seconds)
    pub error_window_seconds: u64,
    /// Maximum consecutive failures before quarantine
    pub max_consecutive_failures: u32,
    /// Maximum response time before considering degraded (in milliseconds)
    pub max_response_time_ms: u64,
    /// Minimum success rate before considering unhealthy
    pub min_success_rate: f64,
    /// Automatic quarantine duration (in seconds)
    pub quarantine_duration_seconds: u64,
    /// Maximum quarantine attempts before permanent disable
    pub max_quarantine_attempts: u32,
    /// Enable automatic recovery
    pub enable_auto_recovery: bool,
    /// Recovery cooldown period (in seconds)
    pub recovery_cooldown_seconds: u64,
}

/// Plugin error event
#[derive(Debug, Clone)]
pub struct PluginErrorEvent {
    /// Plugin ID
    pub plugin_id: PluginId,
    /// Error that occurred
    pub error: EnhancedError,
    /// Operation that was being performed
    pub operation: String,
    /// Response time if applicable
    pub response_time: Option<Duration>,
    /// Additional context
    pub context: HashMap<String, String>,
}

/// Plugin recovery action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginRecoveryAction {
    /// Restart the plugin
    Restart,
    /// Reload plugin configuration
    ReloadConfig,
    /// Clear plugin cache
    ClearCache,
    /// Reset plugin to default state
    ResetToDefault,
    /// Quarantine the plugin
    Quarantine,
    /// Disable the plugin
    Disable,
    /// Reduce plugin privileges
    ReducePrivileges,
    /// Switch to safe mode
    SafeMode,
}

impl PluginErrorIsolation {
    /// Create a new plugin error isolation manager
    pub fn new(
        event_bus: EventBus,
        logger: Arc<RwLock<StructuredLogger>>,
        config: IsolationConfig,
    ) -> Self {
        Self {
            plugin_health: Arc::new(RwLock::new(HashMap::new())),
            recovery_manager: ErrorRecoveryManager::new(),
            event_bus,
            logger,
            config,
            quarantined_plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a plugin for health monitoring
    pub fn register_plugin(&self, plugin_id: PluginId) {
        let mut health_map = self.plugin_health.write().unwrap();
        health_map.insert(plugin_id.clone(), PluginHealth {
            plugin_id: plugin_id.clone(),
            status: HealthStatus::Healthy,
            error_count: 0,
            last_error: None,
            total_errors: 0,
            consecutive_failures: 0,
            last_success: Some(Local::now()),
            performance: PluginPerformanceMetrics::default(),
            recovery_attempts: 0,
            uptime: Duration::from_secs(0),
            memory_usage: None,
        });

        // Log plugin registration
        {
            let mut logger = self.logger.write().unwrap();
            logger.info("plugin_isolation", &format!("Registered plugin for health monitoring: {}", plugin_id));
        }
    }

    /// Unregister a plugin from health monitoring
    pub fn unregister_plugin(&self, plugin_id: &PluginId) {
        let mut health_map = self.plugin_health.write().unwrap();
        health_map.remove(plugin_id);

        let mut quarantine_map = self.quarantined_plugins.write().unwrap();
        quarantine_map.remove(plugin_id);

        // Log plugin unregistration
        {
            let mut logger = self.logger.write().unwrap();
            logger.info("plugin_isolation", &format!("Unregistered plugin from health monitoring: {}", plugin_id));
        }
    }

    /// Report a plugin error
    pub async fn report_error(&self, event: PluginErrorEvent) -> Result<(), RuffError> {
        let plugin_id = &event.plugin_id;
        
        // Update plugin health
        let health_updated = {
            let mut health_map = self.plugin_health.write().unwrap();
            if let Some(health) = health_map.get_mut(plugin_id) {
                health.error_count += 1;
                health.total_errors += 1;
                health.consecutive_failures += 1;
                health.last_error = Some(Local::now());
                
                // Update performance metrics
                health.performance.failed_operations += 1;
                health.performance.total_operations += 1;
                health.performance.success_rate = 
                    health.performance.successful_operations as f64 / health.performance.total_operations as f64;
                
                true
            } else {
                false
            }
        };

        if !health_updated {
            return Err(RuffError::Plugin {
                plugin_name: plugin_id.clone(),
                message: "Plugin not registered for health monitoring".to_string(),
            });
        }

        // Log the error
        {
            let mut logger = self.logger.write().unwrap();
            logger.log_error(&event.error);
        }

        // Check if health status needs to be updated
        self.update_plugin_health_status(plugin_id).await?;

        // Attempt recovery if needed
        if self.config.enable_auto_recovery {
            self.attempt_plugin_recovery(plugin_id, &event).await?;
        }

        // Publish event
        self.event_bus.publish(AppEvent::PluginError {
            plugin_id: plugin_id.clone(),
            error: event.error.to_string(),
        }).await.map_err(|e| RuffError::App(e.to_string()))?;

        Ok(())
    }

    /// Report a successful plugin operation
    pub async fn report_success(&self, plugin_id: &PluginId, response_time: Duration) -> Result<(), RuffError> {
        let mut health_map = self.plugin_health.write().unwrap();
        if let Some(health) = health_map.get_mut(plugin_id) {
            health.consecutive_failures = 0;
            health.last_success = Some(Local::now());
            
            // Update performance metrics
            health.performance.successful_operations += 1;
            health.performance.total_operations += 1;
            health.performance.success_rate = 
                health.performance.successful_operations as f64 / health.performance.total_operations as f64;
            
            let response_time_ms = response_time.as_millis() as u64;
            
            // Update response time metrics
            if health.performance.total_operations == 1 {
                health.performance.avg_response_time = response_time_ms as f64;
                health.performance.min_response_time = response_time_ms;
                health.performance.max_response_time = response_time_ms;
            } else {
                let total_ops = health.performance.total_operations as f64;
                health.performance.avg_response_time = 
                    (health.performance.avg_response_time * (total_ops - 1.0) + response_time_ms as f64) / total_ops;
                health.performance.min_response_time = health.performance.min_response_time.min(response_time_ms);
                health.performance.max_response_time = health.performance.max_response_time.max(response_time_ms);
            }
        } else {
            return Err(RuffError::Plugin {
                plugin_name: plugin_id.clone(),
                message: "Plugin not registered for health monitoring".to_string(),
            });
        }

        // Check if health status needs to be updated
        self.update_plugin_health_status(plugin_id).await?;

        Ok(())
    }

    /// Update plugin health status based on current metrics
    async fn update_plugin_health_status(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        let new_status = {
            let health_map = self.plugin_health.read().unwrap();
            if let Some(health) = health_map.get(plugin_id) {
                self.calculate_health_status(health)
            } else {
                return Ok(());
            }
        };

        // Update status if changed
        {
            let mut health_map = self.plugin_health.write().unwrap();
            if let Some(health) = health_map.get_mut(plugin_id) {
                if health.status != new_status {
                    let old_status = health.status.clone();
                    health.status = new_status.clone();
                    
                    // Log status change
                    {
                        let mut logger = self.logger.write().unwrap();
                        logger.warn("plugin_isolation", &format!(
                            "Plugin {} health status changed from {:?} to {:?}",
                            plugin_id, old_status, new_status
                        ));
                    }

                    // Handle quarantine if needed
                    if new_status == HealthStatus::Quarantined {
                        self.quarantine_plugin(plugin_id).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Calculate health status based on metrics
    fn calculate_health_status(&self, health: &PluginHealth) -> HealthStatus {
        // Check if already quarantined or disabled
        if health.status == HealthStatus::Quarantined || health.status == HealthStatus::Disabled {
            return health.status.clone();
        }

        // Check for quarantine conditions
        if health.consecutive_failures >= self.config.max_consecutive_failures {
            return HealthStatus::Quarantined;
        }

        // Check error rate in time window
        if let Some(last_error) = health.last_error {
            let window_start = Local::now() - chrono::Duration::seconds(self.config.error_window_seconds as i64);
            if last_error > window_start && health.error_count >= self.config.max_errors_per_window {
                return HealthStatus::Unhealthy;
            }
        }

        // Check performance metrics
        if health.performance.total_operations > 0 {
            if health.performance.success_rate < self.config.min_success_rate {
                return HealthStatus::Unhealthy;
            }
            
            if health.performance.avg_response_time > self.config.max_response_time_ms as f64 {
                return HealthStatus::Degraded;
            }
        }

        HealthStatus::Healthy
    }

    /// Quarantine a plugin
    async fn quarantine_plugin(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        let quarantine_info = QuarantineInfo {
            quarantined_at: Local::now(),
            reason: "Exceeded failure thresholds".to_string(),
            quarantine_count: 1,
            expires_at: Some(Local::now() + chrono::Duration::seconds(self.config.quarantine_duration_seconds as i64)),
            requires_manual_intervention: false,
        };

        {
            let mut quarantine_map = self.quarantined_plugins.write().unwrap();
            quarantine_map.insert(plugin_id.clone(), quarantine_info);
        }

        // Log quarantine
        {
            let mut logger = self.logger.write().unwrap();
            logger.error("plugin_isolation", &format!("Plugin {} has been quarantined", plugin_id));
        }

        // Publish event
        self.event_bus.publish(AppEvent::PluginError {
            plugin_id: plugin_id.clone(),
            error: "Plugin quarantined due to repeated failures".to_string(),
        }).await.map_err(|e| RuffError::App(e.to_string()))?;

        Ok(())
    }

    /// Attempt to recover a plugin
    async fn attempt_plugin_recovery(&self, plugin_id: &PluginId, error_event: &PluginErrorEvent) -> Result<(), RuffError> {
        // Check if plugin is in recovery cooldown
        if let Some(last_success) = self.get_plugin_last_success(plugin_id) {
            let cooldown_end = last_success + Duration::from_secs(self.config.recovery_cooldown_seconds);
            if Instant::now() < cooldown_end {
                return Ok(()); // Still in cooldown
            }
        }

        // Determine recovery action based on error and health
        let recovery_action = self.determine_recovery_action(plugin_id, error_event);

        // Execute recovery action
        match recovery_action {
            PluginRecoveryAction::Restart => {
                self.restart_plugin(plugin_id).await?;
            }
            PluginRecoveryAction::ReloadConfig => {
                self.reload_plugin_config(plugin_id).await?;
            }
            PluginRecoveryAction::ClearCache => {
                self.clear_plugin_cache(plugin_id).await?;
            }
            PluginRecoveryAction::ResetToDefault => {
                self.reset_plugin_to_default(plugin_id).await?;
            }
            PluginRecoveryAction::Quarantine => {
                self.quarantine_plugin(plugin_id).await?;
            }
            PluginRecoveryAction::Disable => {
                self.disable_plugin(plugin_id).await?;
            }
            PluginRecoveryAction::ReducePrivileges => {
                self.reduce_plugin_privileges(plugin_id).await?;
            }
            PluginRecoveryAction::SafeMode => {
                self.enable_plugin_safe_mode(plugin_id).await?;
            }
        }

        // Increment recovery attempts
        {
            let mut health_map = self.plugin_health.write().unwrap();
            if let Some(health) = health_map.get_mut(plugin_id) {
                health.recovery_attempts += 1;
            }
        }

        Ok(())
    }

    /// Determine appropriate recovery action
    fn determine_recovery_action(&self, plugin_id: &PluginId, _error_event: &PluginErrorEvent) -> PluginRecoveryAction {
        let health_map = self.plugin_health.read().unwrap();
        if let Some(health) = health_map.get(plugin_id) {
            match health.status {
                HealthStatus::Healthy => PluginRecoveryAction::ClearCache,
                HealthStatus::Degraded => PluginRecoveryAction::ReloadConfig,
                HealthStatus::Unhealthy => {
                    if health.recovery_attempts < 3 {
                        PluginRecoveryAction::Restart
                    } else {
                        PluginRecoveryAction::Quarantine
                    }
                }
                HealthStatus::Quarantined => PluginRecoveryAction::Disable,
                HealthStatus::Disabled => PluginRecoveryAction::Disable,
            }
        } else {
            PluginRecoveryAction::Restart
        }
    }

    /// Get plugin health information
    pub fn get_plugin_health(&self, plugin_id: &PluginId) -> Option<PluginHealth> {
        let health_map = self.plugin_health.read().unwrap();
        health_map.get(plugin_id).cloned()
    }

    /// Get all plugin health information
    pub fn get_all_plugin_health(&self) -> HashMap<PluginId, PluginHealth> {
        let health_map = self.plugin_health.read().unwrap();
        health_map.clone()
    }

    /// Get quarantined plugins
    pub fn get_quarantined_plugins(&self) -> HashMap<PluginId, QuarantineInfo> {
        let quarantine_map = self.quarantined_plugins.read().unwrap();
        quarantine_map.clone()
    }

    /// Check if plugin is healthy
    pub fn is_plugin_healthy(&self, plugin_id: &PluginId) -> bool {
        let health_map = self.plugin_health.read().unwrap();
        health_map.get(plugin_id)
            .map(|health| health.status == HealthStatus::Healthy)
            .unwrap_or(false)
    }

    /// Get plugin last success time
    fn get_plugin_last_success(&self, _plugin_id: &PluginId) -> Option<Instant> {
        // This would need to be implemented with actual timing
        None
    }

    // Recovery action implementations (these would integrate with actual plugin management)
    async fn restart_plugin(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Implementation would restart the plugin
        {
            let mut logger = self.logger.write().unwrap();
            logger.info("plugin_isolation", &format!("Restarting plugin: {}", plugin_id));
        }
        Ok(())
    }

    async fn reload_plugin_config(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Implementation would reload plugin configuration
        {
            let mut logger = self.logger.write().unwrap();
            logger.info("plugin_isolation", &format!("Reloading config for plugin: {}", plugin_id));
        }
        Ok(())
    }

    async fn clear_plugin_cache(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Implementation would clear plugin cache
        {
            let mut logger = self.logger.write().unwrap();
            logger.info("plugin_isolation", &format!("Clearing cache for plugin: {}", plugin_id));
        }
        Ok(())
    }

    async fn reset_plugin_to_default(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Implementation would reset plugin to default state
        {
            let mut logger = self.logger.write().unwrap();
            logger.info("plugin_isolation", &format!("Resetting plugin to default: {}", plugin_id));
        }
        Ok(())
    }

    async fn disable_plugin(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Implementation would disable the plugin
        {
            let mut health_map = self.plugin_health.write().unwrap();
            if let Some(health) = health_map.get_mut(plugin_id) {
                health.status = HealthStatus::Disabled;
            }
        }

        {
            let mut logger = self.logger.write().unwrap();
            logger.warn("plugin_isolation", &format!("Disabled plugin: {}", plugin_id));
        }
        Ok(())
    }

    async fn reduce_plugin_privileges(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Implementation would reduce plugin privileges
        {
            let mut logger = self.logger.write().unwrap();
            logger.info("plugin_isolation", &format!("Reducing privileges for plugin: {}", plugin_id));
        }
        Ok(())
    }

    async fn enable_plugin_safe_mode(&self, plugin_id: &PluginId) -> Result<(), RuffError> {
        // Implementation would enable safe mode for plugin
        {
            let mut logger = self.logger.write().unwrap();
            logger.info("plugin_isolation", &format!("Enabling safe mode for plugin: {}", plugin_id));
        }
        Ok(())
    }
}

impl Default for PluginPerformanceMetrics {
    fn default() -> Self {
        Self {
            avg_response_time: 0.0,
            max_response_time: 0,
            min_response_time: 0,
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            success_rate: 1.0,
        }
    }
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            max_errors_per_window: 5,
            error_window_seconds: 300, // 5 minutes
            max_consecutive_failures: 3,
            max_response_time_ms: 5000, // 5 seconds
            min_success_rate: 0.8, // 80%
            quarantine_duration_seconds: 1800, // 30 minutes
            max_quarantine_attempts: 3,
            enable_auto_recovery: true,
            recovery_cooldown_seconds: 60, // 1 minute
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{StructuredLogger, LoggerConfig};
    use crate::error::{ErrorCategory, ErrorSeverity};

    fn create_test_isolation() -> PluginErrorIsolation {
        let event_bus = EventBus::new();
        let logger = Arc::new(RwLock::new(StructuredLogger::new(LoggerConfig::default())));
        let config = IsolationConfig::default();
        
        PluginErrorIsolation::new(event_bus, logger, config)
    }

    #[test]
    fn test_plugin_registration() {
        let isolation = create_test_isolation();
        let plugin_id = "test_plugin".to_string();
        
        isolation.register_plugin(plugin_id.clone());
        
        let health = isolation.get_plugin_health(&plugin_id);
        assert!(health.is_some());
        assert_eq!(health.unwrap().status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_error_reporting() {
        let isolation = create_test_isolation();
        let plugin_id = "test_plugin".to_string();
        
        isolation.register_plugin(plugin_id.clone());
        
        let error_event = PluginErrorEvent {
            plugin_id: plugin_id.clone(),
            error: EnhancedError::new(
                ErrorCategory::Plugin,
                ErrorSeverity::Error,
                "Test error".to_string(),
            ),
            operation: "test_operation".to_string(),
            response_time: Some(Duration::from_millis(100)),
            context: HashMap::new(),
        };
        
        let result = isolation.report_error(error_event).await;
        assert!(result.is_ok());
        
        let health = isolation.get_plugin_health(&plugin_id).unwrap();
        assert_eq!(health.error_count, 1);
        assert_eq!(health.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn test_success_reporting() {
        let isolation = create_test_isolation();
        let plugin_id = "test_plugin".to_string();
        
        isolation.register_plugin(plugin_id.clone());
        
        let result = isolation.report_success(&plugin_id, Duration::from_millis(100)).await;
        assert!(result.is_ok());
        
        let health = isolation.get_plugin_health(&plugin_id).unwrap();
        assert_eq!(health.consecutive_failures, 0);
        assert_eq!(health.performance.successful_operations, 1);
    }

    #[test]
    fn test_health_status_calculation() {
        let isolation = create_test_isolation();
        
        let healthy_plugin = PluginHealth {
            plugin_id: "healthy".to_string(),
            status: HealthStatus::Healthy,
            error_count: 0,
            last_error: None,
            total_errors: 0,
            consecutive_failures: 0,
            last_success: Some(Local::now()),
            performance: PluginPerformanceMetrics {
                success_rate: 0.95,
                avg_response_time: 100.0,
                ..Default::default()
            },
            recovery_attempts: 0,
            uptime: Duration::from_secs(3600),
            memory_usage: None,
        };
        
        let status = isolation.calculate_health_status(&healthy_plugin);
        assert_eq!(status, HealthStatus::Healthy);
        
        let unhealthy_plugin = PluginHealth {
            consecutive_failures: 5, // Exceeds max_consecutive_failures
            ..healthy_plugin.clone()
        };
        
        let status = isolation.calculate_health_status(&unhealthy_plugin);
        assert_eq!(status, HealthStatus::Quarantined);
    }
}