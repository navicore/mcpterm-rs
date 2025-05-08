use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use mcp_core::context::ConversationContext;
use mcp_llm::{LlmClient, LlmResponse, StreamChunk, ToolCall};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

// Mock LLM client for testing
pub struct MockLlmClient {
    // Configuration options
    pub response_content: String,
    pub add_tool_call: bool,
    pub follow_up_response: Option<String>,
    pub use_jsonrpc_format: bool,
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self {
            response_content: "This is a mock response from the LLM".to_string(),
            add_tool_call: false,
            follow_up_response: Some(
                "This is a follow-up response after tool execution".to_string(),
            ),
            use_jsonrpc_format: true, // Use JSON-RPC format by default
        }
    }
}

impl MockLlmClient {
    pub fn new(response: &str) -> Self {
        Self {
            response_content: response.to_string(),
            add_tool_call: false,
            follow_up_response: Some(
                "This is a follow-up response after tool execution".to_string(),
            ),
            use_jsonrpc_format: true, // Use JSON-RPC format by default
        }
    }

    pub fn with_tool_call(mut self) -> Self {
        self.add_tool_call = true;
        self
    }

    pub fn with_follow_up(mut self, follow_up: &str) -> Self {
        self.follow_up_response = Some(follow_up.to_string());
        self
    }

    pub fn without_follow_up(mut self) -> Self {
        self.follow_up_response = None;
        self
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn send_message(&self, context: &ConversationContext) -> Result<LlmResponse> {
        // Check if this is a follow-up request after a tool call
        // We can determine this by looking at the last few message roles
        let is_follow_up_request = context.messages.len() >= 2
            && matches!(
                context.messages[context.messages.len() - 1].role,
                mcp_core::context::MessageRole::Tool
            );

        // Check if this is a second-level follow-up (happens after a tool call and the first follow-up)
        let is_second_follow_up = context.messages.len() >= 4
            && matches!(
                context.messages[context.messages.len() - 2].role,
                mcp_core::context::MessageRole::Tool
            )
            && matches!(
                context.messages[context.messages.len() - 1].role,
                mcp_core::context::MessageRole::User
            )
            && context.messages[context.messages.len() - 1]
                .content
                .contains("continue");

        // For second-level follow-ups, return an empty response to avoid infinite recursion
        if is_second_follow_up {
            return Ok(LlmResponse {
                id: "mock-empty-follow-up-id".to_string(),
                content: String::new(), // Empty content to end the recursion
                tool_calls: vec![],     // No tool calls
            });
        }

        if is_follow_up_request && self.follow_up_response.is_some() {
            // This is a follow-up request after a tool call, use the follow-up response
            let follow_up_text = self.follow_up_response.as_ref().unwrap().clone();

            return Ok(LlmResponse {
                id: "mock-follow-up-id".to_string(),
                content: follow_up_text,
                tool_calls: vec![], // No tool calls in follow-up response
            });
        }

        // Extract the user's last message to include in the response
        let last_message = context
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // For follow-up responses, don't modify the text - use it directly
        let response_content = if is_follow_up_request && self.follow_up_response.is_some() {
            self.follow_up_response.as_ref().unwrap().clone()
        } else {
            format!(
                "{} (responding to: {})",
                self.response_content, last_message
            )
        };

        // Create response text, either as JSON-RPC or plain text
        let response_text = if self.use_jsonrpc_format {
            // Valid JSON-RPC format
            format!(
                r#"{{"jsonrpc":"2.0","result":"{}","id":"mock-response-id"}}"#,
                response_content.replace("\"", "\\\"")
            )
        } else {
            // Plain text format for testing validation
            response_content
        };

        // Create tool calls if requested
        let tool_calls = if self.add_tool_call {
            vec![ToolCall {
                id: "mock-tool-call-1".to_string(),
                tool: "mock-tool".to_string(),
                params: serde_json::json!({
                    "param1": "value1",
                    "param2": 42
                }),
            }]
        } else {
            vec![]
        };

        Ok(LlmResponse {
            id: "mock-response-id".to_string(),
            content: response_text,
            tool_calls,
        })
    }

    async fn stream_message(
        &self,
        context: &ConversationContext,
    ) -> Result<Box<dyn Stream<Item = Result<StreamChunk>> + Unpin + Send>> {
        // Check if this is a follow-up request after a tool call
        let is_follow_up_request = context.messages.len() >= 2
            && matches!(
                context.messages[context.messages.len() - 1].role,
                mcp_core::context::MessageRole::Tool
            );

        // Check if this is a second-level follow-up request (after a tool call and follow-up)
        let is_second_follow_up = context.messages.len() >= 4
            && matches!(
                context.messages[context.messages.len() - 2].role,
                mcp_core::context::MessageRole::Tool
            )
            && matches!(
                context.messages[context.messages.len() - 1].role,
                mcp_core::context::MessageRole::User
            )
            && context.messages[context.messages.len() - 1]
                .content
                .contains("continue");

        // For second-level follow-ups, return an empty response to avoid infinite recursion
        if is_second_follow_up {
            // Create an empty channel
            let (tx, rx) = mpsc::channel::<Result<StreamChunk>>(1);

            // Send only a completion message with empty content
            tokio::spawn(async move {
                let final_chunk = StreamChunk {
                    id: "mock-stream-chunk".to_string(),
                    content: String::new(),
                    is_tool_call: false,
                    tool_call: None,
                    is_complete: true,
                };

                let _ = tx.send(Ok(final_chunk)).await;
            });

            return Ok(Box::new(ReceiverStream::new(rx)));
        }

        // Extract the user's last message to include in the response
        let last_message = context
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // First determine the raw content to use
        let content = if is_follow_up_request && self.follow_up_response.is_some() {
            // Use the follow-up response for tool execution results
            self.follow_up_response.as_ref().unwrap().clone()
        } else {
            // Regular response with reference to the prompt
            format!(
                "{} (responding to: {})",
                self.response_content, last_message
            )
        };

        // Then format according to the format preference
        let response_text = if self.use_jsonrpc_format {
            // Format as JSON-RPC
            format!(
                r#"{{"jsonrpc":"2.0","result":"{}","id":"mock-stream-id"}}"#,
                content.replace("\"", "\\\"")
            )
        } else {
            // Plain text for testing validation
            content
        };

        // Create a channel for the stream
        let (tx, rx) = mpsc::channel::<Result<StreamChunk>>(5);

        // Clone data for the async task
        let response_text_clone = response_text.clone();
        let add_tool_call = if is_follow_up_request {
            false // No tool calls in follow-up responses
        } else {
            self.add_tool_call
        };

        // Spawn a task to simulate streaming the response
        tokio::spawn(async move {
            // Split the response into chunks
            let words: Vec<&str> = response_text_clone.split_whitespace().collect();

            // Send each chunk with small delay
            for chunk in words.chunks(2) {
                let chunk_text = chunk.join(" ");
                if !chunk_text.is_empty() {
                    let stream_chunk = StreamChunk {
                        id: "mock-stream-chunk".to_string(),
                        content: format!("{} ", chunk_text), // Add space after each chunk
                        is_tool_call: false,
                        tool_call: None,
                        is_complete: false,
                    };

                    if let Err(e) = tx.send(Ok(stream_chunk)).await {
                        eprintln!("Error sending stream chunk: {}", e);
                        break;
                    }

                    // Small delay between chunks to simulate streaming
                    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                }
            }

            // Send final chunk with tool call if needed
            let tool_call = if add_tool_call {
                Some(ToolCall {
                    id: "mock-tool-call-1".to_string(),
                    tool: "mock-tool".to_string(),
                    params: serde_json::json!({
                        "param1": "value1",
                        "param2": 42
                    }),
                })
            } else {
                None
            };

            // Send completion message
            let final_chunk = StreamChunk {
                id: "mock-stream-chunk".to_string(),
                content: String::new(),
                is_tool_call: add_tool_call,
                tool_call,
                is_complete: true,
            };

            let _ = tx.send(Ok(final_chunk)).await;
        });

        // Return the receiver as a stream
        Ok(Box::new(ReceiverStream::new(rx)))
    }

    fn cancel_request(&self, _request_id: &str) -> Result<()> {
        // Nothing to do for the mock
        Ok(())
    }
}
