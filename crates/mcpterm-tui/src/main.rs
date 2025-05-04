use anyhow::Result;
use clap::Parser;
use mcpterm_tui::App;

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    /// LLM model to use
    #[clap(long, default_value = "anthropic.claude-3-sonnet-20240229-v1:0")]
    model: String,

    /// Enable MCP protocol
    #[clap(long)]
    mcp: bool,

    /// AWS region for Bedrock
    #[clap(long)]
    region: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let _cli = Cli::parse();

    // Create and run the application
    let mut app = App::new()?;
    app.run().await?;

    Ok(())
}
