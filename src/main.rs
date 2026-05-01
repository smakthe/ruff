use ruff::EnhancedError;
type Result<T> = std::result::Result<T, EnhancedError>;
use clap::{Parser, Subcommand};
use colored::Colorize;
use ruff::app::App;
use ruff::chat::ChatSession;
use ruff::config::{Config, ConfigurationService};
use ruff::export::{BackupManager, ExportFormat, ExportService, ImportService};
use ruff::plugin::PluginManager;
use ruff::session::SessionManager;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "ruff")]
#[command(about = "A terminal-based AI chat application")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Initialize configuration (legacy)
    #[arg(long)]
    init: bool,

    /// Show current configuration (legacy)
    #[arg(long)]
    show_config: bool,

    /// List all saved chat sessions (legacy)
    #[arg(long)]
    list_sessions: bool,

    /// Delete a specific chat session by ID (legacy)
    #[arg(long)]
    delete_session: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Session management commands
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Export and import commands
    Export {
        #[command(subcommand)]
        action: ExportAction,
    },
    /// Plugin management commands
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Configuration management commands
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// API key management commands
    ApiKey {
        #[command(subcommand)]
        action: ApiKeyAction,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List all sessions
    List {
        /// Filter by title pattern
        #[arg(short, long)]
        filter: Option<String>,
        /// Show archived sessions
        #[arg(short, long)]
        archived: bool,
        /// Sort by (created, updated, title, messages)
        #[arg(short, long, default_value = "updated")]
        sort: String,
    },
    /// Create a new session
    Create {
        /// Session title
        #[arg(short, long)]
        title: Option<String>,
        /// System prompt
        #[arg(short, long)]
        system_prompt: Option<String>,
        /// Model to use
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Delete a session
    Delete {
        /// Session ID
        session_id: String,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Rename a session
    Rename {
        /// Session ID
        session_id: String,
        /// New title
        title: String,
    },
    /// Archive/unarchive a session
    Archive {
        /// Session ID
        session_id: String,
        /// Unarchive instead of archive
        #[arg(short, long)]
        unarchive: bool,
    },
    /// Search sessions
    Search {
        /// Search query
        query: String,
        /// Search in content, not just titles
        #[arg(short, long)]
        content: bool,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Show session details
    Show {
        /// Session ID
        session_id: String,
        /// Show message count only
        #[arg(short, long)]
        summary: bool,
    },
}

#[derive(Subcommand)]
enum ExportAction {
    /// Export a session
    Session {
        /// Session ID
        session_id: String,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Export format (markdown, json, html, text)
        #[arg(short, long, default_value = "markdown")]
        format: String,
        /// Include metadata
        #[arg(short, long)]
        metadata: bool,
    },
    /// Export all sessions
    All {
        /// Output directory
        #[arg(short, long)]
        output: PathBuf,
        /// Export format (markdown, json, html, text)
        #[arg(short, long, default_value = "markdown")]
        format: String,
        /// Include metadata
        #[arg(short, long)]
        metadata: bool,
        /// Compress output
        #[arg(short, long)]
        compress: bool,
    },
    /// Import sessions
    Import {
        /// Input file path
        input: PathBuf,
        /// Import format (auto-detect if not specified)
        #[arg(short, long)]
        format: Option<String>,
        /// Preview import without applying
        #[arg(short, long)]
        preview: bool,
        /// Merge with existing sessions
        #[arg(short, long)]
        merge: bool,
    },
    /// Create backup
    Backup {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Include configuration
        #[arg(short, long)]
        config: bool,
        /// Include plugins
        #[arg(short, long)]
        plugins: bool,
        /// Compress backup
        #[arg(short, long)]
        compress: bool,
    },
    /// Restore from backup
    Restore {
        /// Backup file path
        input: PathBuf,
        /// Preview restore without applying
        #[arg(short, long)]
        preview: bool,
        /// Force restore without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List installed plugins
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
        /// Show only enabled plugins
        #[arg(short, long)]
        enabled: bool,
    },
    /// Install a plugin
    Install {
        /// Plugin path or URL
        source: String,
        /// Force installation
        #[arg(short, long)]
        force: bool,
    },
    /// Uninstall a plugin
    Uninstall {
        /// Plugin ID
        plugin_id: String,
        /// Force uninstallation
        #[arg(short, long)]
        force: bool,
    },
    /// Enable a plugin
    Enable {
        /// Plugin ID
        plugin_id: String,
    },
    /// Disable a plugin
    Disable {
        /// Plugin ID
        plugin_id: String,
    },
    /// Show plugin information
    Info {
        /// Plugin ID
        plugin_id: String,
    },
    /// Update a plugin
    Update {
        /// Plugin ID (update all if not specified)
        plugin_id: Option<String>,
    },
    /// Validate plugin security
    Validate {
        /// Plugin ID or path
        target: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show {
        /// Show specific section (ui, models, plugins, network)
        #[arg(short, long)]
        section: Option<String>,
        /// Output format (yaml, json, toml)
        #[arg(short, long, default_value = "yaml")]
        format: String,
    },
    /// Initialize configuration
    Init {
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
        /// Use minimal configuration
        #[arg(short, long)]
        minimal: bool,
    },
    /// Validate configuration
    Validate {
        /// Configuration file path
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Fix validation errors automatically
        #[arg(short, long)]
        fix: bool,
    },
    /// Migrate configuration
    Migrate {
        /// Source configuration version
        #[arg(short, long)]
        from: Option<String>,
        /// Target configuration version
        #[arg(short, long)]
        to: Option<String>,
        /// Backup original configuration
        #[arg(short, long)]
        backup: bool,
    },
    /// Set configuration value
    Set {
        /// Configuration key (dot notation)
        key: String,
        /// Configuration value
        value: String,
    },
    /// Get configuration value
    Get {
        /// Configuration key (dot notation)
        key: String,
    },
    /// Reset configuration to defaults
    Reset {
        /// Reset specific section only
        #[arg(short, long)]
        section: Option<String>,
        /// Force reset without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum ApiKeyAction {
    /// Store an API key in the OS keyring
    Set {
        /// Provider name (e.g., openai, anthropic, groq)
        provider: String,
        /// API key value
        #[arg(short, long)]
        key: String,
    },
    /// Retrieve an API key from the OS keyring
    Get {
        /// Provider name
        provider: String,
    },
    /// Delete an API key from the OS keyring
    Delete {
        /// Provider name
        provider: String,
    },
    /// List all providers with stored keys
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle legacy flags for backward compatibility
    if cli.init {
        Config::init_config()?;
        return Ok(());
    }

    if cli.show_config {
        Config::show_config()?;
        return Ok(());
    }

    if cli.list_sessions {
        let sessions = ChatSession::list_sessions()?;
        if sessions.is_empty() {
            println!("{}", "No saved sessions found.".yellow());
        } else {
            println!("{}", "Saved Chat Sessions:".bright_green());
            for session in sessions {
                println!(
                    "- {} ({}): {} messages, created on {}",
                    session.id.to_string().bright_cyan(),
                    session.title.yellow(),
                    session.messages.len(),
                    session.created_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
        return Ok(());
    }

    if let Some(session_id_str) = cli.delete_session {
        match Uuid::parse_str(&session_id_str) {
            Ok(session_id) => {
                ChatSession::delete(session_id)?;
                println!("{} {}", "🗑️ Session".red(), session_id_str.bright_cyan());
            }
            Err(_) => {
                eprintln!("{}", "Invalid session ID format.".red());
            }
        }
        return Ok(());
    }

    // Handle new subcommands
    match cli.command {
        Some(Commands::Session { action }) => {
            handle_session_command(action).await?;
        }
        Some(Commands::Export { action }) => {
            handle_export_command(action).await?;
        }
        Some(Commands::Plugin { action }) => {
            handle_plugin_command(action).await?;
        }
        Some(Commands::Config { action }) => {
            handle_config_command(action).await?;
        }
        Some(Commands::ApiKey { action }) => {
            handle_apikey_command(action)?;
        }
        None => {
            // No subcommand provided, start the interactive app
            match App::new().await {
                Ok(mut app) => {
                    if let Err(e) = app.run().await {
                        eprintln!("\n{} Application error: {}", "❌".red(), e);
                        eprintln!("{}", format!("Details: {:?}", e).dimmed());

                        // Try to cleanup terminal
                        if let Err(cleanup_err) = app.cleanup() {
                            eprintln!(
                                "{} Failed to cleanup terminal: {}",
                                "⚠️".yellow(),
                                cleanup_err
                            );
                        }

                        return Err(e);
                    }
                }
                Err(e) => {
                    eprintln!("\n{} Failed to initialize application", "❌".red());
                    eprintln!("{}", format!("Error: {}", e).bright_red());
                    eprintln!();

                    // Check common issues
                    if format!("{:?}", e).contains("keyring")
                        || format!("{:?}", e).contains("api_key")
                    {
                        eprintln!(
                            "{}",
                            "It looks like you haven't set up any API keys yet.".yellow()
                        );
                        eprintln!("{}", "Set up a key with:".bright_cyan());
                        eprintln!(
                            "{}",
                            "  ruff api-key set openai --key YOUR_KEY".bright_cyan()
                        );
                        eprintln!();
                    }

                    if format!("{:?}", e).contains("Configuration")
                        || format!("{:?}", e).contains("config")
                    {
                        eprintln!("{}", "Configuration issue detected.".yellow());
                        eprintln!("{}", "Try initializing with:".bright_cyan());
                        eprintln!("{}", "  ruff config init".bright_cyan());
                        eprintln!();
                    }

                    return Err(e);
                }
            }
        }
    }

    Ok(())
}

// Session command handlers
async fn handle_session_command(action: SessionAction) -> Result<()> {
    let mut session_manager = SessionManager::new_default().await?;

    match action {
        SessionAction::List {
            filter,
            archived,
            sort,
        } => {
            let sessions = session_manager
                .list_sessions(filter.as_deref(), archived, &sort)
                .await?;
            if sessions.is_empty() {
                println!("{}", "No sessions found.".yellow());
            } else {
                println!("{}", "Sessions:".bright_green());
                for session in sessions {
                    let status = if session.is_archived {
                        " (archived)".dimmed()
                    } else {
                        "".into()
                    };
                    println!(
                        "- {} ({}): {} messages, updated {}{}",
                        session.id.to_string().bright_cyan(),
                        session.title.yellow(),
                        session.message_count,
                        session.updated_at.format("%Y-%m-%d %H:%M:%S"),
                        status
                    );
                }
            }
        }
        SessionAction::Create {
            title,
            system_prompt,
            model,
        } => {
            let session_id = session_manager
                .create_session_cli(title, system_prompt, model)
                .await?;
            println!(
                "{} {}",
                "✅ Created session".green(),
                session_id.to_string().bright_cyan()
            );
        }
        SessionAction::Delete { session_id, force } => {
            let session_uuid = Uuid::parse_str(&session_id)
                .map_err(|e| EnhancedError::parsing(format!("Invalid UUID: {}", e)))?;
            if !force {
                print!(
                    "Are you sure you want to delete session {}? (y/N): ",
                    session_id.bright_cyan()
                );
                use std::io::{self, Write};
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("{}", "Cancelled.".yellow());
                    return Ok(());
                }
            }
            session_manager.delete_session(session_uuid).await?;
            if let Ok(mut message_manager) = ruff::message::manager::MessageManager::new_default() {
                message_manager.clear_session_messages(session_uuid);
            }
            println!(
                "{} {}",
                "🗑️ Deleted session".red(),
                session_id.bright_cyan()
            );
        }
        SessionAction::Rename { session_id, title } => {
            let session_uuid = Uuid::parse_str(&session_id)
                .map_err(|e| EnhancedError::parsing(format!("Invalid UUID: {}", e)))?;
            session_manager
                .rename_session(session_uuid, title.clone())
                .await?;
            println!(
                "{} {} to '{}'",
                "✏️ Renamed session".green(),
                session_id.bright_cyan(),
                title.yellow()
            );
        }
        SessionAction::Archive {
            session_id,
            unarchive,
        } => {
            let session_uuid = Uuid::parse_str(&session_id)
                .map_err(|e| EnhancedError::parsing(format!("Invalid UUID: {}", e)))?;
            if unarchive {
                session_manager.unarchive_session(session_uuid).await?;
                println!(
                    "{} {}",
                    "📂 Unarchived session".green(),
                    session_id.bright_cyan()
                );
            } else {
                session_manager.archive_session(session_uuid).await?;
                println!(
                    "{} {}",
                    "📦 Archived session".green(),
                    session_id.bright_cyan()
                );
            }
        }
        SessionAction::Search {
            query,
            content,
            limit,
        } => {
            let results = session_manager
                .search_sessions_cli(&query, content, limit)
                .await?;
            if results.is_empty() {
                println!("{}", "No sessions found matching the query.".yellow());
            } else {
                println!("{} {} results:", "🔍 Found".green(), results.len());
                for result in results {
                    println!(
                        "- {} ({}): {}",
                        result.session_id.to_string().bright_cyan(),
                        result.title.yellow(),
                        result.snippet.dimmed()
                    );
                }
            }
        }
        SessionAction::Show {
            session_id,
            summary,
        } => {
            let session_uuid = Uuid::parse_str(&session_id)
                .map_err(|e| EnhancedError::parsing(format!("Invalid UUID: {}", e)))?;
            let session = session_manager.get_session_for_cli(session_uuid).await?;
            if summary {
                println!(
                    "{}: {} messages",
                    session_id.bright_cyan(),
                    session.message_count
                );
            } else {
                println!("{}", "Session Details:".bright_green());
                println!("ID: {}", session.id.to_string().bright_cyan());
                println!("Title: {}", session.title.yellow());
                println!("Messages: {}", session.message_count);
                println!("Model: {}", session.model.bright_blue());
                println!(
                    "Created: {}",
                    session.created_at.format("%Y-%m-%d %H:%M:%S")
                );
                println!(
                    "Updated: {}",
                    session.updated_at.format("%Y-%m-%d %H:%M:%S")
                );
                if let Some(system_prompt) = &session.system_prompt {
                    println!("System Prompt: {}", system_prompt.dimmed());
                }
            }
        }
    }

    Ok(())
}

// Export command handlers
async fn handle_export_command(action: ExportAction) -> Result<()> {
    let export_service = ExportService::new_default().await?;
    let mut import_service = ImportService::new_default().await?;
    let backup_manager = BackupManager::new_default().await?;

    match action {
        ExportAction::Session {
            session_id,
            output,
            format,
            metadata,
        } => {
            let session_uuid = Uuid::parse_str(&session_id)
                .map_err(|e| EnhancedError::parsing(format!("Invalid UUID: {}", e)))?;
            let export_format = parse_export_format(&format)?;
            let result = export_service
                .export_session_cli(session_uuid, &output, export_format, metadata)
                .await?;
            println!(
                "{} {} to {}",
                "📤 Exported session".green(),
                session_id.bright_cyan(),
                output.display().to_string().yellow()
            );
            println!("Size: {} bytes", result.size_bytes);
        }
        ExportAction::All {
            output,
            format,
            metadata,
            compress,
        } => {
            let export_format = parse_export_format(&format)?;
            let result = export_service
                .export_all_sessions(&output, export_format, metadata, compress)
                .await?;
            println!(
                "{} {} sessions to {}",
                "📤 Exported".green(),
                result.session_count,
                output.display().to_string().yellow()
            );
            println!("Total size: {} bytes", result.total_size);
        }
        ExportAction::Import {
            input,
            format,
            preview,
            merge,
        } => {
            let import_format = format.as_deref().map(parse_import_format).transpose()?;
            if preview {
                let preview_result = import_service
                    .preview_import_cli(&input, import_format)
                    .await?;
                println!("{}", "Import Preview:".bright_green());
                println!("Sessions to import: {}", preview_result.session_count);
                println!("Total messages: {}", preview_result.message_count);
                println!("Conflicts: {}", preview_result.conflicts.len());
                if !preview_result.conflicts.is_empty() {
                    println!("{}", "Conflicts:".yellow());
                    for conflict in preview_result.conflicts {
                        println!("  - {}", conflict);
                    }
                }
            } else {
                let result = import_service
                    .import_sessions_cli(&input, import_format, merge)
                    .await?;
                println!(
                    "{} {} sessions from {}",
                    "📥 Imported".green(),
                    result.imported_count,
                    input.display().to_string().yellow()
                );
                if result.skipped_count > 0 {
                    println!("Skipped: {} sessions", result.skipped_count);
                }
            }
        }
        ExportAction::Backup {
            output,
            config,
            plugins,
            compress,
        } => {
            let result = backup_manager
                .create_backup_cli(&output, config, plugins, compress)
                .await?;
            println!(
                "{} to {}",
                "💾 Created backup".green(),
                output.display().to_string().yellow()
            );
            println!(
                "Sessions: {}, Size: {} bytes",
                result.session_count, result.backup_size
            );
        }
        ExportAction::Restore {
            input,
            preview,
            force,
        } => {
            if preview {
                let preview_result = backup_manager.preview_restore(&input).await?;
                println!("{}", "Restore Preview:".bright_green());
                println!("Sessions: {}", preview_result.session_count);
                println!(
                    "Configuration: {}",
                    if preview_result.has_config {
                        "Yes"
                    } else {
                        "No"
                    }
                );
                println!("Plugins: {}", preview_result.plugin_count);
            } else {
                if !force {
                    print!("Are you sure you want to restore from backup? This may overwrite existing data. (y/N): ");
                    use std::io::{self, Write};
                    io::stdout().flush()?;
                    let mut input_str = String::new();
                    io::stdin().read_line(&mut input_str)?;
                    if !input_str.trim().to_lowercase().starts_with('y') {
                        println!("{}", "Cancelled.".yellow());
                        return Ok(());
                    }
                }
                let result = backup_manager.restore_backup_cli(&input).await?;
                println!(
                    "{} {} sessions",
                    "🔄 Restored".green(),
                    result.restored_count
                );
            }
        }
    }

    Ok(())
}

// Plugin command handlers
async fn handle_plugin_command(action: PluginAction) -> Result<()> {
    let mut plugin_manager = PluginManager::new_default().await?;

    match action {
        PluginAction::List { detailed, enabled } => {
            let plugins = plugin_manager.list_plugins(enabled).await?;
            if plugins.is_empty() {
                println!("{}", "No plugins installed.".yellow());
            } else {
                println!("{}", "Installed Plugins:".bright_green());
                for plugin in plugins {
                    let status_icon = match plugin.status {
                        ruff::plugin::PluginStatus::Loaded => "✅",
                        ruff::plugin::PluginStatus::Disabled => "⏸️",
                        ruff::plugin::PluginStatus::Error(_) => "❌",
                        ruff::plugin::PluginStatus::Unloaded => "⏹️",
                    };

                    if detailed {
                        println!(
                            "{} {} v{}",
                            status_icon,
                            plugin.metadata.name.bright_cyan(),
                            plugin.metadata.version.yellow()
                        );
                        println!("  ID: {}", plugin.metadata.id.dimmed());
                        println!("  Description: {}", plugin.metadata.description);
                        println!("  Author: {}", plugin.metadata.author);
                        if let Some(homepage) = &plugin.metadata.homepage {
                            println!("  Homepage: {}", homepage.bright_blue());
                        }
                        println!();
                    } else {
                        println!(
                            "{} {} v{} - {}",
                            status_icon,
                            plugin.metadata.name.bright_cyan(),
                            plugin.metadata.version.yellow(),
                            plugin.metadata.description.dimmed()
                        );
                    }
                }
            }
        }
        PluginAction::Install { source, force } => {
            let result = plugin_manager.install_plugin(&source, force).await?;
            println!(
                "{} {} v{}",
                "🔌 Installed plugin".green(),
                result.name.bright_cyan(),
                result.version.yellow()
            );
        }
        PluginAction::Uninstall { plugin_id, force } => {
            if !force {
                print!(
                    "Are you sure you want to uninstall plugin '{}'? (y/N): ",
                    plugin_id.bright_cyan()
                );
                use std::io::{self, Write};
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("{}", "Cancelled.".yellow());
                    return Ok(());
                }
            }
            plugin_manager.uninstall_plugin(&plugin_id).await?;
            println!(
                "{} {}",
                "🗑️ Uninstalled plugin".red(),
                plugin_id.bright_cyan()
            );
        }
        PluginAction::Enable { plugin_id } => {
            plugin_manager.enable_plugin_cli(&plugin_id).await?;
            println!(
                "{} {}",
                "✅ Enabled plugin".green(),
                plugin_id.bright_cyan()
            );
        }
        PluginAction::Disable { plugin_id } => {
            plugin_manager.disable_plugin_cli(&plugin_id).await?;
            println!(
                "{} {}",
                "⏸️ Disabled plugin".yellow(),
                plugin_id.bright_cyan()
            );
        }
        PluginAction::Info { plugin_id } => {
            let plugin_info = plugin_manager.get_plugin_info(&plugin_id).await?;
            println!("{}", "Plugin Information:".bright_green());
            println!("Name: {}", plugin_info.metadata.name.bright_cyan());
            println!("Version: {}", plugin_info.metadata.version.yellow());
            println!("ID: {}", plugin_info.metadata.id.dimmed());
            println!("Description: {}", plugin_info.metadata.description);
            println!("Author: {}", plugin_info.metadata.author);
            if let Some(homepage) = &plugin_info.metadata.homepage {
                println!("Homepage: {}", homepage.bright_blue());
            }
            if let Some(repository) = &plugin_info.metadata.repository {
                println!("Repository: {}", repository.bright_blue());
            }
            if let Some(license) = &plugin_info.metadata.license {
                println!("License: {}", license);
            }
            println!("Permissions: {}", plugin_info.metadata.permissions.len());
            for permission in &plugin_info.metadata.permissions {
                println!("  - {:?}", permission);
            }
        }
        PluginAction::Update { plugin_id } => {
            if let Some(id) = plugin_id {
                let result = plugin_manager.update_plugin(&id).await?;
                println!(
                    "{} {} to v{}",
                    "🔄 Updated plugin".green(),
                    id.bright_cyan(),
                    result.new_version.yellow()
                );
            } else {
                let results = plugin_manager.update_all_plugins().await?;
                println!("{} {} plugins", "🔄 Updated".green(), results.len());
                for result in results {
                    println!(
                        "  {} v{} -> v{}",
                        result.plugin_id.bright_cyan(),
                        result.old_version.dimmed(),
                        result.new_version.yellow()
                    );
                }
            }
        }
        PluginAction::Validate { target } => {
            let validation_result = plugin_manager.validate_plugin(&target).await?;
            if validation_result.is_valid {
                println!("{} Plugin validation passed", "✅".green());
            } else {
                println!("{} Plugin validation failed", "❌".red());
                for error in validation_result.errors {
                    println!("  - {}", error.red());
                }
            }
            if !validation_result.warnings.is_empty() {
                println!("{}", "Warnings:".yellow());
                for warning in validation_result.warnings {
                    println!("  - {}", warning.yellow());
                }
            }
        }
    }

    Ok(())
}

// Configuration command handlers
async fn handle_config_command(action: ConfigAction) -> Result<()> {
    let config_service = ConfigurationService::new_default().await?;

    match action {
        ConfigAction::Show { section, format } => {
            let config_str = config_service
                .show_config(section.as_deref(), &format)
                .await?;
            println!("{}", config_str);
        }
        ConfigAction::Init { force, minimal } => {
            if !force && config_service.config_exists().await? {
                print!("Configuration already exists. Overwrite? (y/N): ");
                use std::io::{self, Write};
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("{}", "Cancelled.".yellow());
                    return Ok(());
                }
            }
            config_service.init_config(minimal).await?;
            println!("{}", "✅ Configuration initialized".green());
        }
        ConfigAction::Validate { config, fix } => {
            let validation_result = if let Some(config_path) = config {
                config_service.validate_config_file(&config_path).await?
            } else {
                config_service.validate_current_config().await?
            };

            if validation_result.is_valid {
                println!("{} Configuration is valid", "✅".green());
            } else {
                println!("{} Configuration validation failed", "❌".red());
                for error in &validation_result.errors {
                    println!("  - {}", error.red());
                }

                if fix && !validation_result.errors.is_empty() {
                    println!("{}", "Attempting to fix errors...".yellow());
                    let fix_result = config_service
                        .fix_config_errors(&validation_result.errors)
                        .await?;
                    if fix_result.fixed_count > 0 {
                        println!("{} Fixed {} errors", "✅".green(), fix_result.fixed_count);
                    }
                    if fix_result.unfixed_count > 0 {
                        println!(
                            "{} {} errors could not be fixed automatically",
                            "⚠️".yellow(),
                            fix_result.unfixed_count
                        );
                    }
                }
            }

            if !validation_result.warnings.is_empty() {
                println!("{}", "Warnings:".yellow());
                for warning in &validation_result.warnings {
                    println!("  - {}", warning.yellow());
                }
            }
        }
        ConfigAction::Migrate { from, to, backup } => {
            let migration_result = config_service
                .migrate_config(from.as_deref(), to.as_deref(), backup)
                .await?;
            println!(
                "{} Configuration migrated from {} to {}",
                "🔄".green(),
                migration_result.from_version.yellow(),
                migration_result.to_version.yellow()
            );
            if backup && migration_result.backup_path.is_some() {
                println!(
                    "Backup created at: {}",
                    migration_result
                        .backup_path
                        .unwrap()
                        .display()
                        .to_string()
                        .dimmed()
                );
            }
        }
        ConfigAction::Set { key, value } => {
            config_service.set_config_value(&key, &value).await?;
            println!(
                "{} Set {} = {}",
                "✅".green(),
                key.bright_cyan(),
                value.yellow()
            );
        }
        ConfigAction::Get { key } => {
            let value = config_service.get_config_value(&key).await?;
            println!("{}: {}", key.bright_cyan(), value.yellow());
        }
        ConfigAction::Reset { section, force } => {
            if !force {
                let reset_target = section.as_deref().unwrap_or("entire configuration");
                print!("Are you sure you want to reset {}? (y/N): ", reset_target);
                use std::io::{self, Write};
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("{}", "Cancelled.".yellow());
                    return Ok(());
                }
            }
            config_service.reset_config(section.as_deref()).await?;
            let reset_target = section.as_deref().unwrap_or("configuration");
            println!("{} Reset {}", "🔄".green(), reset_target.yellow());
        }
    }

    Ok(())
}

// API key command handlers
fn handle_apikey_command(action: ApiKeyAction) -> Result<()> {
    use ruff::security::KeyringManager;

    let keyring = KeyringManager::new();

    match action {
        ApiKeyAction::Set { provider, key } => {
            keyring.store_api_key(&provider, &key)?;
            println!(
                "{} API key stored securely for {}",
                "✅".green(),
                provider.bright_cyan()
            );
        }
        ApiKeyAction::Get { provider } => {
            match keyring.get_api_key(&provider) {
                Ok(key) => {
                    // Only show first/last 4 chars for security
                    let masked = if key.len() > 8 {
                        format!("{}...{}", &key[..4], &key[key.len() - 4..])
                    } else {
                        "****".to_string()
                    };
                    println!(
                        "API key for {}: {}",
                        provider.bright_cyan(),
                        masked.yellow()
                    );
                }
                Err(e) => {
                    println!(
                        "{} No API key found for {}: {}",
                        "❌".red(),
                        provider.bright_cyan(),
                        e.to_string().dimmed()
                    );
                }
            }
        }
        ApiKeyAction::Delete { provider } => match keyring.delete_api_key(&provider) {
            Ok(_) => {
                println!(
                    "{} API key deleted for {}",
                    "✅".green(),
                    provider.bright_cyan()
                );
            }
            Err(e) => {
                println!(
                    "{} Failed to delete API key for {}: {}",
                    "❌".red(),
                    provider.bright_cyan(),
                    e.to_string().dimmed()
                );
            }
        },
        ApiKeyAction::List => {
            let providers = keyring.list_providers()?;
            println!("{}", "📋 API Key Status:".bright_green());
            println!();
            let mut has_any = false;
            for provider in providers {
                match keyring.get_api_key(&provider) {
                    Ok(_) => {
                        println!(
                            "  {} {} - Key stored securely",
                            "✅".green(),
                            provider.bright_cyan()
                        );
                        has_any = true;
                    }
                    Err(_) => {
                        println!("  {} {} - No key set", "❌".red(), provider.bright_cyan());
                    }
                }
            }
            println!();
            if !has_any {
                println!("{}", "No API keys found. Set one with:".yellow());
                println!(
                    "{}",
                    "  ruff api-key set openai --key YOUR_KEY".bright_cyan()
                );
            }
        }
    }

    Ok(())
}

// Helper functions
fn parse_export_format(format: &str) -> Result<ExportFormat> {
    match format.to_lowercase().as_str() {
        "markdown" | "md" => Ok(ExportFormat::Markdown),
        "json" => Ok(ExportFormat::Json),
        "html" => Ok(ExportFormat::Html),
        "text" | "txt" => Ok(ExportFormat::PlainText),
        _ => Err(EnhancedError::parsing(format!(
            "Unsupported export format: {}",
            format
        ))),
    }
}

fn parse_import_format(format: &str) -> Result<ruff::export::ImportFormat> {
    match format.to_lowercase().as_str() {
        "json" => Ok(ruff::export::ImportFormat::Json),
        "chatgpt" => Ok(ruff::export::ImportFormat::ChatGptExport),
        "claude" => Ok(ruff::export::ImportFormat::ClaudeExport),
        _ => Err(EnhancedError::parsing(format!(
            "Unsupported import format: {}",
            format
        ))),
    }
}
