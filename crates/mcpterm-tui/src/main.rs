use mcpterm_tui::App;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Create and run a new Application instance
    let mut app = App::new()?;
    app.run()?;
    
    Ok(())
}