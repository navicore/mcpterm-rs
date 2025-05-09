use anyhow::Result;
use mcpterm_cli::{CliApp, CliConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Create a simple CLI configuration for testing
    let cli_config = CliConfig {
        model: "us.anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
        use_mcp: true,
        region: None, // Will use the default AWS region
        streaming: true,
        enable_tools: true,
        require_tool_confirmation: false, // No confirmation for this test
        auto_approve_tools: true,
    };

    // Create CLI application with configuration
    let mut app = CliApp::new().with_config(cli_config);

    // Initialize the application
    println!("Initializing CLI application");
    if let Err(e) = app.initialize().await {
        println!("Failed to initialize app: {}", e);
        return Err(e);
    }

    // Run with a test prompt that should trigger a tool call
    let prompt = "Please list the current directory";
    println!("Sending prompt: {}", prompt);
    
    let response = app.run(prompt).await?;
    println!("Response: {}", response);
    
    // The application should continue running until all tool calls are processed
    println!("Test completed successfully");
    
    Ok(())
}