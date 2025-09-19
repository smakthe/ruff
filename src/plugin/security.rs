use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::plugin::{Permission, PluginId, PluginMetadata};
use crate::RuffError;

/// Security policy for plugin execution
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// Maximum execution time for plugin operations
    pub max_execution_time: Duration,
    /// Maximum memory usage (in bytes)
    pub max_memory_usage: usize,
    /// Allowed file system paths
    pub allowed_paths: Vec<PathBuf>,
    /// Blocked file system paths
    pub blocked_paths: Vec<PathBuf>,
    /// Allowed network hosts
    pub allowed_hosts: Vec<String>,
    /// Blocked network hosts
    pub blocked_hosts: Vec<String>,
    /// Maximum number of concurrent operations
    pub max_concurrent_operations: usize,
    /// Whether to allow system command execution
    pub allow_system_commands: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            max_execution_time: Duration::from_secs(30),
            max_memory_usage: 100 * 1024 * 1024, // 100MB
            allowed_paths: vec![],
            blocked_paths: vec![
                PathBuf::from("/etc"),
                PathBuf::from("/sys"),
                PathBuf::from("/proc"),
                PathBuf::from("/dev"),
            ],
            allowed_hosts: vec![],
            blocked_hosts: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "::1".to_string(),
            ],
            max_concurrent_operations: 10,
            allow_system_commands: false,
        }
    }
}

/// Plugin sandbox for security enforcement
pub struct PluginSandbox {
    plugin_id: PluginId,
    policy: SecurityPolicy,
    granted_permissions: HashSet<Permission>,
    active_operations: usize,
    start_time: Instant,
}

impl PluginSandbox {
    /// Create a new plugin sandbox
    pub fn new(plugin_id: PluginId, policy: SecurityPolicy, permissions: Vec<Permission>) -> Self {
        Self {
            plugin_id,
            policy,
            granted_permissions: permissions.into_iter().collect(),
            active_operations: 0,
            start_time: Instant::now(),
        }
    }

    /// Check if a permission is granted
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.granted_permissions.contains(permission)
    }

    /// Validate plugin metadata against security policy
    pub fn validate_metadata(&self, metadata: &PluginMetadata) -> Result<(), RuffError> {
        // Check if all requested permissions are granted
        for permission in &metadata.permissions {
            if !self.has_permission(permission) {
                return Err(RuffError::Plugin {
                    plugin_name: self.plugin_id.clone(),
                    message: format!("Permission {:?} not granted", permission),
                });
            }
        }

        // Validate plugin ID matches
        if metadata.id != self.plugin_id {
            return Err(RuffError::Plugin {
                plugin_name: self.plugin_id.clone(),
                message: "Plugin ID mismatch".to_string(),
            });
        }

        Ok(())
    }

    /// Check if a file path is allowed
    pub fn is_path_allowed(&self, path: &PathBuf) -> bool {
        // Check if path is explicitly blocked
        for blocked_path in &self.policy.blocked_paths {
            if path.starts_with(blocked_path) {
                return false;
            }
        }

        // If allowed paths are specified, path must be in the list
        if !self.policy.allowed_paths.is_empty() {
            return self.policy.allowed_paths.iter().any(|allowed_path| {
                path.starts_with(allowed_path)
            });
        }

        true
    }

    /// Check if a network host is allowed
    pub fn is_host_allowed(&self, host: &str) -> bool {
        // Check if host is explicitly blocked
        if self.policy.blocked_hosts.contains(&host.to_string()) {
            return false;
        }

        // If allowed hosts are specified, host must be in the list
        if !self.policy.allowed_hosts.is_empty() {
            return self.policy.allowed_hosts.contains(&host.to_string());
        }

        true
    }

    /// Start a new operation (for concurrency control)
    pub fn start_operation(&mut self) -> Result<OperationGuard, RuffError> {
        if self.active_operations >= self.policy.max_concurrent_operations {
            return Err(RuffError::Plugin {
                plugin_name: self.plugin_id.clone(),
                message: "Maximum concurrent operations exceeded".to_string(),
            });
        }

        self.active_operations += 1;
        Ok(OperationGuard::new(self))
    }

    /// Check if execution time limit is exceeded
    pub fn check_execution_time(&self) -> Result<(), RuffError> {
        if self.start_time.elapsed() > self.policy.max_execution_time {
            return Err(RuffError::Plugin {
                plugin_name: self.plugin_id.clone(),
                message: "Execution time limit exceeded".to_string(),
            });
        }
        Ok(())
    }

    /// Validate a system command
    pub fn validate_system_command(&self, command: &str) -> Result<(), RuffError> {
        if !self.policy.allow_system_commands {
            return Err(RuffError::Plugin {
                plugin_name: self.plugin_id.clone(),
                message: "System commands not allowed".to_string(),
            });
        }

        if !self.has_permission(&Permission::SystemCommands) {
            return Err(RuffError::Plugin {
                plugin_name: self.plugin_id.clone(),
                message: "SystemCommands permission required".to_string(),
            });
        }

        // Basic command validation - block dangerous commands
        let dangerous_commands = [
            "rm", "del", "format", "fdisk", "mkfs", "dd", "sudo", "su",
            "chmod", "chown", "passwd", "useradd", "userdel", "groupadd",
            "systemctl", "service", "reboot", "shutdown", "halt",
        ];

        let command_name = command.split_whitespace().next().unwrap_or("");
        if dangerous_commands.contains(&command_name) {
            return Err(RuffError::Plugin {
                plugin_name: self.plugin_id.clone(),
                message: format!("Dangerous command '{}' not allowed", command_name),
            });
        }

        Ok(())
    }

    /// Reset the sandbox (for reuse)
    pub fn reset(&mut self) {
        self.active_operations = 0;
        self.start_time = Instant::now();
    }

    /// Get current resource usage statistics
    pub fn get_stats(&self) -> SandboxStats {
        SandboxStats {
            plugin_id: self.plugin_id.clone(),
            active_operations: self.active_operations,
            execution_time: self.start_time.elapsed(),
            granted_permissions: self.granted_permissions.len(),
        }
    }
}

/// Guard for tracking active operations
pub struct OperationGuard<'a> {
    sandbox: &'a mut PluginSandbox,
}

impl<'a> OperationGuard<'a> {
    fn new(sandbox: &'a mut PluginSandbox) -> Self {
        Self { sandbox }
    }
}

impl<'a> Drop for OperationGuard<'a> {
    fn drop(&mut self) {
        if self.sandbox.active_operations > 0 {
            self.sandbox.active_operations -= 1;
        }
    }
}

/// Statistics about sandbox usage
#[derive(Debug, Clone)]
pub struct SandboxStats {
    pub plugin_id: PluginId,
    pub active_operations: usize,
    pub execution_time: Duration,
    pub granted_permissions: usize,
}

/// Security validator for plugin operations
pub struct SecurityValidator;

impl SecurityValidator {
    /// Validate file system access
    pub fn validate_file_access(
        sandbox: &PluginSandbox,
        path: &PathBuf,
        operation: FileOperation,
    ) -> Result<(), RuffError> {
        // Check file system permission
        match operation {
            FileOperation::Read => {
                if !sandbox.has_permission(&Permission::FileSystem) {
                    return Err(RuffError::Plugin {
                        plugin_name: sandbox.plugin_id.clone(),
                        message: "FileSystem permission required for reading".to_string(),
                    });
                }
            }
            FileOperation::Write | FileOperation::Delete => {
                if !sandbox.has_permission(&Permission::FileSystem) {
                    return Err(RuffError::Plugin {
                        plugin_name: sandbox.plugin_id.clone(),
                        message: "FileSystem permission required for writing".to_string(),
                    });
                }
            }
        }

        // Check path access
        if !sandbox.is_path_allowed(path) {
            return Err(RuffError::Plugin {
                plugin_name: sandbox.plugin_id.clone(),
                message: format!("Access to path {:?} not allowed", path),
            });
        }

        Ok(())
    }

    /// Validate network access
    pub fn validate_network_access(
        sandbox: &PluginSandbox,
        host: &str,
        port: u16,
    ) -> Result<(), RuffError> {
        if !sandbox.has_permission(&Permission::Network) {
            return Err(RuffError::Plugin {
                plugin_name: sandbox.plugin_id.clone(),
                message: "Network permission required".to_string(),
            });
        }

        if !sandbox.is_host_allowed(host) {
            return Err(RuffError::Plugin {
                plugin_name: sandbox.plugin_id.clone(),
                message: format!("Access to host '{}' not allowed", host),
            });
        }

        // Block privileged ports unless explicitly allowed
        if port < 1024 && !sandbox.policy.allowed_hosts.contains(&host.to_string()) {
            return Err(RuffError::Plugin {
                plugin_name: sandbox.plugin_id.clone(),
                message: format!("Access to privileged port {} not allowed", port),
            });
        }

        Ok(())
    }
}

/// File system operation types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOperation {
    Read,
    Write,
    Delete,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_policy_default() {
        let policy = SecurityPolicy::default();
        assert_eq!(policy.max_execution_time, Duration::from_secs(30));
        assert_eq!(policy.max_memory_usage, 100 * 1024 * 1024);
        assert!(!policy.allow_system_commands);
    }

    #[test]
    fn test_plugin_sandbox_permissions() {
        let plugin_id = "test-plugin".to_string();
        let policy = SecurityPolicy::default();
        let permissions = vec![Permission::ReadSessions, Permission::Network];
        
        let sandbox = PluginSandbox::new(plugin_id, policy, permissions);
        
        assert!(sandbox.has_permission(&Permission::ReadSessions));
        assert!(sandbox.has_permission(&Permission::Network));
        assert!(!sandbox.has_permission(&Permission::WriteSessions));
    }

    #[test]
    fn test_path_validation() {
        let plugin_id = "test-plugin".to_string();
        let mut policy = SecurityPolicy::default();
        policy.allowed_paths = vec![PathBuf::from("/tmp")];
        policy.blocked_paths = vec![PathBuf::from("/etc")];
        
        let sandbox = PluginSandbox::new(plugin_id, policy, vec![]);
        
        assert!(sandbox.is_path_allowed(&PathBuf::from("/tmp/test.txt")));
        assert!(!sandbox.is_path_allowed(&PathBuf::from("/etc/passwd")));
        assert!(!sandbox.is_path_allowed(&PathBuf::from("/home/user/test.txt")));
    }

    #[test]
    fn test_host_validation() {
        let plugin_id = "test-plugin".to_string();
        let mut policy = SecurityPolicy::default();
        policy.allowed_hosts = vec!["api.example.com".to_string()];
        policy.blocked_hosts = vec!["localhost".to_string()];
        
        let sandbox = PluginSandbox::new(plugin_id, policy, vec![]);
        
        assert!(sandbox.is_host_allowed("api.example.com"));
        assert!(!sandbox.is_host_allowed("localhost"));
        assert!(!sandbox.is_host_allowed("evil.com"));
    }

    #[test]
    fn test_system_command_validation() {
        let plugin_id = "test-plugin".to_string();
        let mut policy = SecurityPolicy::default();
        policy.allow_system_commands = true;
        
        let sandbox = PluginSandbox::new(plugin_id, policy, vec![Permission::SystemCommands]);
        
        assert!(sandbox.validate_system_command("echo hello").is_ok());
        assert!(sandbox.validate_system_command("ls -la").is_ok());
        assert!(sandbox.validate_system_command("rm -rf /").is_err());
        assert!(sandbox.validate_system_command("sudo rm file").is_err());
    }

    #[test]
    fn test_operation_guard() {
        let plugin_id = "test-plugin".to_string();
        let policy = SecurityPolicy::default();
        let mut sandbox = PluginSandbox::new(plugin_id, policy, vec![]);
        
        assert_eq!(sandbox.active_operations, 0);
        
        {
            let _guard = sandbox.start_operation().unwrap();
            // Can't check active_operations while guard is held due to borrow checker
        }
        
        assert_eq!(sandbox.active_operations, 0);
    }
}