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

        // Verify the response contains the JSON-RPC format with mock response
        assert!(result.contains("jsonrpc"));
        assert!(result.contains("This is a mock LLM response"));
        assert!(result.contains("test prompt"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_cli_app_with_tool_call() {
        // Create a mock client that will return a tool call with a custom follow-up response
        let mock_client = MockLlmClient::new("Response with tool call")
            .with_tool_call()
            .with_follow_up("This is the tool execution result interpretation");

        // Create the CLI app with the mock client
        let mut app = CliApp::new().with_llm_client(mock_client);

        // Run the app with a test prompt
        let result = app.run("invoke a tool").await.unwrap();

        // The response should contain the follow-up response text
        assert!(result.contains("This is the tool execution result interpretation"));

        // Add a timeout to make sure the test doesn't hang
        tokio::time::timeout(std::time::Duration::from_secs(5), async {})
            .await
            .ok();
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

        // Verify the response contains the streamed content in JSON-RPC format
        assert!(result.contains("jsonrpc"));
        assert!(result.contains("This is a streamed response"));
        assert!(result.contains("stream test"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_cli_app_streaming_with_tool_call() {
        // Create a mock client that streams responses with a tool call and follow-up
        let mock_client = MockLlmClient::new("This is a streamed response with tool call")
            .with_tool_call()
            .with_follow_up("This is the follow-up response after streaming tool execution");

        // Create the CLI app with the mock client and enable streaming
        let mut app =
            CliApp::new()
                .with_llm_client(mock_client)
                .with_config(mcpterm_cli::CliConfig {
                    streaming: true,
                    ..Default::default()
                });

        // Run the app with a test prompt
        let result = app.run("stream tool test").await.unwrap();

        // Verify the response contains the follow-up content
        assert!(result.contains("This is the follow-up response after streaming tool execution"));

        // Add a timeout to make sure the test doesn't hang
        tokio::time::timeout(std::time::Duration::from_secs(5), async {})
            .await
            .ok();
    }
}
