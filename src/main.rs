use anyhow::Result;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // If no args are provided (other than the program name), launch TUI mode
    if args.len() <= 1 {
        // Launch TUI mode
        println!("Launching TUI mode...");
        return mcpterm_tui::App::new()?.run().await;
    } else {
        // Otherwise, launch CLI mode with all args
        println!("Launching CLI mode...");
        // Call the CLI main function
        mcpterm_cli::cli_main::main().await
    }
}