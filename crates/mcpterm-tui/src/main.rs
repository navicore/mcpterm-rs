use anyhow::Result;
use clap::Parser;
use mcpterm_tui::{App, direct_impl, clean_impl};

/// Command line arguments for mcpterm-tui
#[derive(Parser)]
#[clap(author, version, about = "Terminal User Interface for MCP")]
struct Cli {
    /// Use direct key handling implementation for improved keyboard behavior
    #[clap(long, short = 'd')]
    direct_mode: bool,
    
    /// Use ultra-simple standalone implementation for testing
    #[clap(long, short = 's')]
    simple_mode: bool,
    
    /// Use clean implementation with working scrolling
    #[clap(long, short = 'c')]
    clean_mode: bool,
    
    /// Disable mouse capture for terminals that don't support it
    #[clap(long)]
    no_mouse: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Parse command line arguments
    let cli = Cli::parse();
    
    if cli.clean_mode {
        // Run the clean implementation that works with proper scrolling
        println!("Running clean implementation with working scrolling...");
        clean_impl::run_clean_with_options(cli.no_mouse)?;
    } else if cli.simple_mode {
        // Run the ultra-simple implementation for basic testing
        println!("Running ultra-simple implementation for testing...");
        direct_impl::run_direct()?;
    } else if cli.direct_mode {
        // Run the direct implementation with basic features
        direct_impl::run_direct_ui()?;
    } else {
        // Run the standard implementation
        let mut app = App::new()?;
        app.run()?;
    }
    
    Ok(())
}