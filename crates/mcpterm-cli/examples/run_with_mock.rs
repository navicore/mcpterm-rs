use anyhow::Result;
use mcpterm_cli::{mock::MockLlmClient, CliApp, CliConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    mcp_core::init_debug_log()?;
    mcp_core::debug_log("Starting mock CLI example");

    // Enable verbose logging
    mcp_core::set_verbose_logging(true);
    mcp_core::debug_log("Verbose logging enabled");

    // Create a mock client
    let mock_client = MockLlmClient::new("This is a response from the mock LLM client");

    // Create the CLI app with the mock client
    let mut app = CliApp::new()
        .with_llm_client(mock_client)
        .with_config(CliConfig {
            model: "mock-model".to_string(),
            use_mcp: false,
            region: Some("us-east-1".to_string()),
            streaming: true,
            enable_tools: true,
            require_tool_confirmation: false, // Don't require confirmation in tests
            auto_approve_tools: true, // Auto-approve tools in tests
        });

    // Run the app with a test prompt
    println!("Sending test prompt to mock LLM...");
    let response = app.run("Test prompt for mock LLM").await?;
    println!("Response: {}", response);

    println!("Check the logs to see what was recorded!");
    Ok(())
}
