use anyhow::Result;
use std::env;

// This function checks if stdin is redirected
fn stdin_has_data() -> bool {
    // Using atty for cross-platform detection
    // If stdin is not a tty, it's likely redirected (pipe, file, etc.)
    !atty::is(atty::Stream::Stdin)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let has_stdin_data = stdin_has_data();
    
    // Launch CLI mode if there are args or if stdin has piped data
    if args.len() > 1 || has_stdin_data {
        // Set environment variable to indicate if input is coming from stdin
        if has_stdin_data {
            std::env::set_var("MCP_STDIN_INPUT", "1");
        }
        
        // Launch CLI mode
        println!("Launching CLI mode...");
        mcpterm_cli::cli_main::main().await
    } else {
        // No args and no piped stdin, launch TUI mode
        println!("Launching TUI mode...");
        mcpterm_tui::App::new()?.run().await
    }
}