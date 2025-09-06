use anyhow::Result;
use clap::Parser;
use ruff::app::App;
use ruff::config::Config;

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
    
    let mut app = App::new().await?;
    app.run().await
}