use anyhow::Result;
use clap::Parser;
use ruff::app::App;
use ruff::config::Config;
use ruff::chat::ChatSession;
use colored::Colorize;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "ruff")]
#[command(about = "A terminal-based AI chat application")]
#[command(version = "0.1.0")]
struct Cli {
    /// Initialize configuration
    #[arg(long)]
    init: bool,
    
    /// Show current configuration
    #[arg(long)]
    show_config: bool,

    /// List all saved chat sessions
    #[arg(long)]
    list_sessions: bool,

    /// Delete a specific chat session by ID
    #[arg(long)]
    delete_session: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
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
    
    let mut app = App::new().await?;
    app.run().await
}