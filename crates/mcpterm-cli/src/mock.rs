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
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self {
            response_content: "This is a mock response from the LLM".to_string(),
            add_tool_call: false,
        }
    }
}

impl MockLlmClient {
    pub fn new(response: &str) -> Self {
        Self {
            response_content: response.to_string(),
            add_tool_call: false,
        }
    }

    pub fn with_tool_call(mut self) -> Self {
        self.add_tool_call = true;
        self
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn send_message(&self, context: &ConversationContext) -> Result<LlmResponse> {
        // Extract the user's last message to include in the response
        let last_message = context.messages.last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Create mock response
        let response_text = format!("{} (responding to: {})", self.response_content, last_message);
        
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
        // Extract the user's last message to include in the response
        let last_message = context.messages.last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Create the response text with the last message
        let response_text = format!("{} (responding to: {})", self.response_content, last_message);
        
        // Create a channel for the stream
        let (tx, rx) = mpsc::channel::<Result<StreamChunk>>(5);
        
        // Clone data for the async task
        let response_text_clone = response_text.clone();
        let add_tool_call = self.add_tool_call;
        
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