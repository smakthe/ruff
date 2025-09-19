//! CLI integration tests
//! 
//! This module contains comprehensive tests for all CLI commands and options
//! to ensure they work correctly and handle edge cases properly.

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::process::Command;
    use std::fs;
    use tempfile::TempDir;
    use uuid::Uuid;
    use crate::session::SessionManager;
    use crate::plugin::PluginManager;
    use crate::config::ConfigurationService;
    use crate::export::{ExportService, ImportService, BackupManager};

    /// Helper function to run CLI commands in tests
    async fn run_cli_command(args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("cargo")
            .args(&["run", "--"])
            .args(args)
            .output()?;
        Ok(output)
    }

    /// Helper function to create a test session
    async fn create_test_session() -> Result<Uuid> {
        let mut session_manager = SessionManager::new().await?;
        let session_id = session_manager.create_session(
            Some("Test Session".to_string()),
            Some("You are a helpful assistant.".to_string()),
            Some("gpt-3.5-turbo".to_string())
        ).await?;
        Ok(session_id)
    }

    #[tokio::test]
    async fn test_session_list_command() -> Result<()> {
        // Create a test session first
        let _session_id = create_test_session().await?;
        
        let output = run_cli_command(&["session", "list"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Sessions:"));
        assert!(stdout.contains("Test Session"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_session_list_with_filters() -> Result<()> {
        let _session_id = create_test_session().await?;
        
        // Test filter by title
        let output = run_cli_command(&["session", "list", "--filter", "Test"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Test Session"));
        
        // Test filter that should return no results
        let output = run_cli_command(&["session", "list", "--filter", "NonExistent"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("No sessions found") || !stdout.contains("Test Session"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_session_create_command() -> Result<()> {
        let output = run_cli_command(&[
            "session", "create", 
            "--title", "CLI Test Session",
            "--system-prompt", "You are a test assistant.",
            "--model", "gpt-4"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Created session"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_session_delete_command() -> Result<()> {
        let session_id = create_test_session().await?;
        
        let output = run_cli_command(&[
            "session", "delete", 
            &session_id.to_string(),
            "--force"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Deleted session"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_session_rename_command() -> Result<()> {
        let session_id = create_test_session().await?;
        
        let output = run_cli_command(&[
            "session", "rename", 
            &session_id.to_string(),
            "Renamed Test Session"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Renamed session"));
        assert!(stdout.contains("Renamed Test Session"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_session_archive_command() -> Result<()> {
        let session_id = create_test_session().await?;
        
        // Test archive
        let output = run_cli_command(&[
            "session", "archive", 
            &session_id.to_string()
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Archived session"));
        
        // Test unarchive
        let output = run_cli_command(&[
            "session", "archive", 
            &session_id.to_string(),
            "--unarchive"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Unarchived session"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_session_search_command() -> Result<()> {
        let _session_id = create_test_session().await?;
        
        let output = run_cli_command(&[
            "session", "search", 
            "Test",
            "--limit", "5"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Found") || stdout.contains("No sessions found"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_session_show_command() -> Result<()> {
        let session_id = create_test_session().await?;
        
        let output = run_cli_command(&[
            "session", "show", 
            &session_id.to_string()
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Session Details:"));
        assert!(stdout.contains("Test Session"));
        
        // Test summary mode
        let output = run_cli_command(&[
            "session", "show", 
            &session_id.to_string(),
            "--summary"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("messages"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_export_session_command() -> Result<()> {
        let session_id = create_test_session().await?;
        let temp_dir = TempDir::new()?;
        let output_path = temp_dir.path().join("test_export.md");
        
        let output = run_cli_command(&[
            "export", "session", 
            &session_id.to_string(),
            "--output", output_path.to_str().unwrap(),
            "--format", "markdown",
            "--metadata"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Exported session"));
        
        // Verify file was created
        assert!(output_path.exists());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_export_all_sessions_command() -> Result<()> {
        let _session_id = create_test_session().await?;
        let temp_dir = TempDir::new()?;
        
        let output = run_cli_command(&[
            "export", "all", 
            "--output", temp_dir.path().to_str().unwrap(),
            "--format", "json",
            "--metadata",
            "--compress"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Exported"));
        assert!(stdout.contains("sessions"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_import_preview_command() -> Result<()> {
        // Create a test export first
        let session_id = create_test_session().await?;
        let temp_dir = TempDir::new()?;
        let export_path = temp_dir.path().join("test_export.json");
        
        // Export session
        let export_service = ExportService::new().await?;
        export_service.export_session(
            session_id, 
            &export_path, 
            crate::export::ExportFormat::Json, 
            true
        ).await?;
        
        // Test import preview
        let output = run_cli_command(&[
            "export", "import", 
            export_path.to_str().unwrap(),
            "--preview"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Import Preview:"));
        assert!(stdout.contains("Sessions to import:"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_backup_and_restore_commands() -> Result<()> {
        let _session_id = create_test_session().await?;
        let temp_dir = TempDir::new()?;
        let backup_path = temp_dir.path().join("test_backup.zip");
        
        // Test backup creation
        let output = run_cli_command(&[
            "export", "backup", 
            "--output", backup_path.to_str().unwrap(),
            "--config",
            "--plugins",
            "--compress"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Created backup"));
        
        // Verify backup file exists
        assert!(backup_path.exists());
        
        // Test restore preview
        let output = run_cli_command(&[
            "export", "restore", 
            backup_path.to_str().unwrap(),
            "--preview"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Restore Preview:"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_plugin_list_command() -> Result<()> {
        let output = run_cli_command(&["plugin", "list"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("plugins") || stdout.contains("No plugins"));
        
        // Test detailed listing
        let output = run_cli_command(&["plugin", "list", "--detailed"]).await?;
        assert!(output.status.success());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_plugin_validate_command() -> Result<()> {
        // Create a mock plugin file for testing
        let temp_dir = TempDir::new()?;
        let plugin_path = temp_dir.path().join("test_plugin.json");
        
        let plugin_metadata = serde_json::json!({
            "id": "test-plugin",
            "name": "Test Plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "author": "Test Author",
            "permissions": ["ReadSessions"],
            "min_ruff_version": "0.1.0"
        });
        
        fs::write(&plugin_path, serde_json::to_string_pretty(&plugin_metadata)?)?;
        
        let output = run_cli_command(&[
            "plugin", "validate", 
            plugin_path.to_str().unwrap()
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("validation"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_config_show_command() -> Result<()> {
        let output = run_cli_command(&["config", "show"]).await?;
        assert!(output.status.success());
        
        // Test specific section
        let output = run_cli_command(&[
            "config", "show", 
            "--section", "ui",
            "--format", "json"
        ]).await?;
        assert!(output.status.success());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_config_init_command() -> Result<()> {
        let temp_dir = TempDir::new()?;
        
        // Set temporary config directory
        std::env::set_var("RUFF_CONFIG_DIR", temp_dir.path());
        
        let output = run_cli_command(&[
            "config", "init", 
            "--minimal",
            "--force"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Configuration initialized"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_config_validate_command() -> Result<()> {
        let output = run_cli_command(&["config", "validate"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("validation"));
        
        // Test with fix option
        let output = run_cli_command(&["config", "validate", "--fix"]).await?;
        assert!(output.status.success());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_config_set_get_commands() -> Result<()> {
        // Test set command
        let output = run_cli_command(&[
            "config", "set", 
            "ui.theme", 
            "dark"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Set ui.theme = dark"));
        
        // Test get command
        let output = run_cli_command(&[
            "config", "get", 
            "ui.theme"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("ui.theme:"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_config_reset_command() -> Result<()> {
        let output = run_cli_command(&[
            "config", "reset", 
            "--section", "ui",
            "--force"
        ]).await?;
        
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Reset ui"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_session_id_handling() -> Result<()> {
        let output = run_cli_command(&[
            "session", "delete", 
            "invalid-uuid",
            "--force"
        ]).await?;
        
        assert!(!output.status.success());
        
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("Invalid") || stderr.contains("error"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_nonexistent_session_handling() -> Result<()> {
        let fake_uuid = Uuid::new_v4();
        
        let output = run_cli_command(&[
            "session", "show", 
            &fake_uuid.to_string()
        ]).await?;
        
        assert!(!output.status.success());
        
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("not found") || stderr.contains("error"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_export_invalid_format_handling() -> Result<()> {
        let session_id = create_test_session().await?;
        let temp_dir = TempDir::new()?;
        let output_path = temp_dir.path().join("test_export.txt");
        
        let output = run_cli_command(&[
            "export", "session", 
            &session_id.to_string(),
            "--output", output_path.to_str().unwrap(),
            "--format", "invalid_format"
        ]).await?;
        
        assert!(!output.status.success());
        
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("Unsupported") || stderr.contains("error"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_plugin_nonexistent_handling() -> Result<()> {
        let output = run_cli_command(&[
            "plugin", "info", 
            "nonexistent-plugin"
        ]).await?;
        
        assert!(!output.status.success());
        
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("not found") || stderr.contains("error"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_config_invalid_key_handling() -> Result<()> {
        let output = run_cli_command(&[
            "config", "get", 
            "invalid.nonexistent.key"
        ]).await?;
        
        assert!(!output.status.success());
        
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("not found") || stderr.contains("error"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_help_and_version_flags() -> Result<()> {
        // Test help flag
        let output = run_cli_command(&["--help"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Usage:") || stdout.contains("USAGE:"));
        
        // Test version flag
        let output = run_cli_command(&["--version"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("0.1.0"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_subcommand_help() -> Result<()> {
        // Test session subcommand help
        let output = run_cli_command(&["session", "--help"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Session management"));
        
        // Test export subcommand help
        let output = run_cli_command(&["export", "--help"]).await?;
        assert!(output.status.success());
        
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("Export and import"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_legacy_commands_compatibility() -> Result<()> {
        // Test legacy list-sessions flag
        let output = run_cli_command(&["--list-sessions"]).await?;
        assert!(output.status.success());
        
        // Test legacy show-config flag
        let output = run_cli_command(&["--show-config"]).await?;
        assert!(output.status.success());
        
        // Test legacy init flag
        let output = run_cli_command(&["--init"]).await?;
        assert!(output.status.success());
        
        Ok(())
    }
}