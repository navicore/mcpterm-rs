use anyhow::Result;
use clap::Parser;
use mcpterm_cli::CliApp;

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    /// Prompt to send to the model
    #[clap(index = 1)]
    prompt: Option<String>,
    
    /// Input file containing prompts (one per line)
    #[clap(long, short)]
    input: Option<String>,
    
    /// Output file for responses
    #[clap(long, short)]
    output: Option<String>,
    
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
    let cli = Cli::parse();
    
    // Create CLI application
    let mut app = CliApp::new();
    
    // Process prompt
    if let Some(prompt) = cli.prompt {
        let response = app.run(&prompt).await?;
        println!("{}", response);
    } else if let Some(input_file) = cli.input {
        // Process input file - not implemented yet
        println!("Input file: {}", input_file);
    } else {
        eprintln!("Error: No prompt or input file provided");
        std::process::exit(1);
    }
    
    Ok(())
}