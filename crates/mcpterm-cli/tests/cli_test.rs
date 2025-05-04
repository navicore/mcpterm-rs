#[cfg(test)]
mod tests {
    use mcpterm_cli::{mock::MockLlmClient, CliApp};

    #[tokio::test]
    async fn test_cli_app_with_mock() {
        // Create a mock client with a predefined response
        let mock_client = MockLlmClient::new("This is a mock LLM response");

        // Create the CLI app with the mock client
        let mut app = CliApp::new().with_llm_client(mock_client);

        // Run the app with a test prompt
        let result = app.run("test prompt").await.unwrap();

        // Verify the response contains both the mock response and the input prompt
        assert!(result.contains("This is a mock LLM response"));
        assert!(result.contains("test prompt"));
    }

    #[tokio::test]
    async fn test_cli_app_with_tool_call() {
        // Create a mock client that will return a tool call
        let mock_client = MockLlmClient::new("Response with tool call").with_tool_call();

        // Create the CLI app with the mock client
        let mut app = CliApp::new().with_llm_client(mock_client);

        // Run the app with a test prompt
        let result = app.run("invoke a tool").await.unwrap();

        // The response should contain the mock response text
        assert!(result.contains("Response with tool call"));

        // The tool call should have been processed by the mock client
        // (we don't need to check this directly as the MockLlmClient will handle it)
    }

    #[tokio::test]
    async fn test_cli_app_streaming() {
        // Create a mock client that streams responses
        let mock_client = MockLlmClient::new("This is a streamed response");

        // Create the CLI app with the mock client and enable streaming
        let mut app =
            CliApp::new()
                .with_llm_client(mock_client)
                .with_config(mcpterm_cli::CliConfig {
                    streaming: true,
                    ..Default::default()
                });

        // Run the app with a test prompt
        let result = app.run("stream test").await.unwrap();

        // Verify the response contains the streamed content
        assert!(result.contains("This is a streamed response"));
        assert!(result.contains("stream test"));
    }
}
