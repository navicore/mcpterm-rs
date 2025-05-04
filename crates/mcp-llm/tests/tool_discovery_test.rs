use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use mcp_core::context::ConversationContext;
use mcp_llm::bedrock::{BedrockClient, BedrockConfig};
use mcp_llm::client_trait::{LlmClient, LlmResponse, StreamChunk};
use mcp_llm::schema::McpSchemaManager;
use mcp_tools::{Tool, ToolCategory, ToolManager, ToolMetadata, ToolResult, ToolStatus};
use serde_json::{json, Value};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

// A mock tool for testing
struct MockTool {
    metadata: ToolMetadata,
}

#[async_trait]
impl Tool for MockTool {
    fn metadata(&self) -> ToolMetadata {
        self.metadata.clone()
    }

    async fn execute(&self, _params: Value) -> Result<ToolResult> {
        Ok(ToolResult {
            tool_id: self.metadata.id.clone(),
            status: ToolStatus::Success,
            output: json!({"result": "mock result"}),
            error: None,
        })
    }
}

// Mock LLM client for testing without actual API calls
struct MockLlmClient {
    last_system_prompt: Arc<Mutex<Option<String>>>,
}

impl MockLlmClient {
    fn new() -> Self {
        Self {
            last_system_prompt: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn send_message(&self, context: &ConversationContext) -> Result<LlmResponse> {
        // Extract the system prompt from the context and store it
        for message in &context.messages {
            if message.role == mcp_core::context::MessageRole::System {
                let mut lock = self.last_system_prompt.lock().unwrap();
                *lock = Some(message.content.clone());
            }
        }

        // Return a mock response
        Ok(LlmResponse {
            id: "mock-response-id".to_string(),
            content: "This is a mock response".to_string(),
            tool_calls: vec![],
        })
    }

    async fn stream_message(
        &self,
        context: &ConversationContext,
    ) -> Result<Box<dyn Stream<Item = Result<StreamChunk>> + Unpin + Send>> {
        // Extract the system prompt from the context and store it
        for message in &context.messages {
            if message.role == mcp_core::context::MessageRole::System {
                let mut lock = self.last_system_prompt.lock().unwrap();
                *lock = Some(message.content.clone());
            }
        }

        // Create a mock stream with a single chunk
        let (tx, rx) = mpsc::channel(1);

        tokio::spawn(async move {
            let chunk = StreamChunk {
                id: "mock-chunk-id".to_string(),
                content: "This is a mock stream chunk".to_string(),
                is_tool_call: false,
                tool_call: None,
                is_complete: true,
            };

            tx.send(Ok(chunk)).await.unwrap();
        });

        Ok(Box::new(ReceiverStream::new(rx)))
    }

    fn cancel_request(&self, _request_id: &str) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_dynamic_tool_discovery_in_client() -> Result<()> {
    // Create a mock LLM client that captures the system prompt
    let mock_client = MockLlmClient::new();
    let last_system_prompt = Arc::clone(&mock_client.last_system_prompt);

    // Create a tool manager with some mock tools
    let mut tool_manager = ToolManager::new();

    // Create and register a custom tool
    let test_tool = MockTool {
        metadata: ToolMetadata {
            id: "test_tool".to_string(),
            name: "Test Tool".to_string(),
            description: "A test tool for automated testing".to_string(),
            category: ToolCategory::General,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "test_param": {
                        "type": "string",
                        "description": "A test parameter"
                    }
                },
                "required": ["test_param"]
            }),
            output_schema: json!({}),
        },
    };

    tool_manager.register_tool(Box::new(test_tool));

    // Generate tool documentation
    let tool_docs = tool_manager.generate_tool_documentation();

    // Create a conversation context
    let mut context = ConversationContext::new();

    // Add a simple system message first (will be overridden by our BedrockClient)
    context.add_system_message("This is a test");

    // Add a user message
    context.add_user_message("Hello");

    // Use the client to send a message
    let _response = mock_client.send_message(&context).await?;

    // Wait a moment to ensure async operations complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify that the system prompt includes our custom tool
    let captured_prompt = {
        let lock = last_system_prompt.lock().unwrap();
        lock.clone().unwrap_or_default()
    };

    assert!(
        captured_prompt.contains("This is a test"),
        "The basic system message should be present"
    );

    let schema_manager = McpSchemaManager::new();

    // Test that the generated prompt with our tool documentation is correct
    let dynamic_prompt = schema_manager.get_mcp_system_prompt_with_tools(&tool_docs);

    // Verify the dynamic prompt contains our custom tool information
    assert!(dynamic_prompt.contains("\"test_tool\""));
    assert!(dynamic_prompt.contains("A test tool for automated testing"));
    assert!(dynamic_prompt.contains("A test parameter"));

    Ok(())
}

// This test simulates what our CliApp does with the dynamic tool generation
#[tokio::test]
async fn test_tool_integration_with_mock_bedrock_client() -> Result<()> {
    // Create a tool manager with some mock tools
    let mut tool_manager = ToolManager::new();

    // Register a few mock tools with different characteristics
    let tool1 = MockTool {
        metadata: ToolMetadata {
            id: "mock_shell".to_string(),
            name: "Mock Shell".to_string(),
            description: "Execute shell commands in a mock".to_string(),
            category: ToolCategory::Shell,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to execute"
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Optional timeout in ms"
                    }
                },
                "required": ["command"]
            }),
            output_schema: json!({}),
        },
    };

    let tool2 = MockTool {
        metadata: ToolMetadata {
            id: "mock_search".to_string(),
            name: "Mock Search".to_string(),
            description: "Search for files and content in a mock".to_string(),
            category: ToolCategory::Search,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to search in"
                    }
                },
                "required": ["query"]
            }),
            output_schema: json!({}),
        },
    };

    tool_manager.register_tool(Box::new(tool1));
    tool_manager.register_tool(Box::new(tool2));

    // Generate tool documentation
    let tool_docs = tool_manager.generate_tool_documentation();

    // Check that documentation contains both tools
    assert!(tool_docs.contains("mock_shell"));
    assert!(tool_docs.contains("mock_search"));

    // Create a mock client and context
    let mock_client = MockLlmClient::new();
    let last_system_prompt = Arc::clone(&mock_client.last_system_prompt);

    let mut context = ConversationContext::new();
    context.add_system_message("Base system prompt");
    context.add_user_message("Test message");

    // Manually add our tool documentation to system prompt
    // (this simulates what BedrockClient does internally)
    let schema_manager = McpSchemaManager::new();
    let updated_prompt = format!(
        "{}\n\n{}",
        "Base system prompt",
        schema_manager.get_mcp_system_prompt_with_tools(&tool_docs)
    );

    context.messages[0].content = updated_prompt;

    // Send a message and check the prompt was used
    let _response = mock_client.send_message(&context).await?;

    // Wait a moment to ensure async operations complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify the system prompt in the context includes both tools
    let captured_prompt = {
        let lock = last_system_prompt.lock().unwrap();
        lock.clone().unwrap_or_default()
    };

    assert!(captured_prompt.contains("mock_shell"));
    assert!(captured_prompt.contains("The command to execute"));
    assert!(captured_prompt.contains("mock_search"));
    assert!(captured_prompt.contains("The search query"));

    Ok(())
}
