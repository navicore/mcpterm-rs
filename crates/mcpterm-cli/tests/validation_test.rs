#[cfg(test)]
mod validation_tests {
    use anyhow::Result;
    use mcp_core::context::MessageRole;
    use mcpterm_cli::{mock::MockLlmClient, CliApp, CliConfig};

    /// Helper function to create a mock client with an invalid response
    fn create_invalid_response_mock() -> MockLlmClient {
        // Create a custom mock client that returns an invalid (non-JSON-RPC) response
        let mut mock = MockLlmClient::new("This is not a valid JSON-RPC response");

        // Override the default response format with a custom implementation
        mock.response_content = "This is an invalid response".to_string();
        mock.use_jsonrpc_format = false;

        mock
    }

    /// Helper function to create a mock client with a valid JSON-RPC response
    fn create_valid_response_mock() -> MockLlmClient {
        // The mock client implementation now creates valid JSON-RPC responses
        // so we can use it directly
        MockLlmClient::new("This is a valid response")
    }

    #[tokio::test]
    async fn test_validation_with_invalid_response() -> Result<()> {
        // Create a mock client with an invalid non-JSON-RPC response
        let invalid_mock = create_invalid_response_mock();

        // Create app with validation enabled
        let mut app = CliApp::new()
            .with_llm_client(invalid_mock)
            .with_config(CliConfig {
                streaming: false, // Use non-streaming for simplicity
                ..Default::default()
            });

        // Run with a test prompt
        let result = app.run("test validation").await?;

        // Response should be valid JSON-RPC after correction
        assert!(result.contains("jsonrpc"));

        // Check that the conversation contains a correction message
        let context = app.debug_context_size();
        assert!(
            context >= 3,
            "Expected at least 3 messages in context after validation"
        );

        // The roles should include a correction request and response
        let roles = app.debug_last_message_roles(3);
        assert!(roles.contains("User"), "Missing user message");
        assert!(roles.contains("Assistant"), "Missing assistant message");

        Ok(())
    }

    #[tokio::test]
    async fn test_validation_with_valid_response() -> Result<()> {
        // Create a mock client with a valid JSON-RPC response
        let valid_mock = create_valid_response_mock();

        // Create app with validation enabled
        let mut app = CliApp::new()
            .with_llm_client(valid_mock)
            .with_config(CliConfig {
                streaming: false, // Use non-streaming for simplicity
                ..Default::default()
            });

        // Run with a test prompt
        let result = app.run("test validation").await?;

        // Response should be valid JSON-RPC
        assert!(result.contains("jsonrpc"));

        // Check conversation has the expected message count (no correction needed)
        let context = app.debug_context_size();
        assert_eq!(
            context, 2,
            "Expected exactly 2 messages in context: user and assistant"
        );

        // The roles should just be user and assistant
        let roles = app.debug_last_message_roles(2);
        assert!(roles.contains("User"), "Missing user message");
        assert!(roles.contains("Assistant"), "Missing assistant message");
        assert!(!roles.contains("System"), "Unexpected system message");

        Ok(())
    }
}
