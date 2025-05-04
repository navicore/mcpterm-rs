use crate::client_trait::{LlmClient, LlmResponse, StreamChunk, ToolCall as ClientToolCall};
use crate::schema::McpSchemaManager;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_bedrockruntime::Client as BedrockRuntimeClient;
use aws_smithy_types::Blob;
use futures::Stream;
use mcp_core::context::{ConversationContext, MessageRole};
use mcp_core::protocol::{Request as McpRequest, Response as McpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

// Bedrock specific errors
#[derive(Debug, thiserror::Error)]
pub enum BedrockError {
    #[error("Failed to parse Bedrock response: {0}")]
    ResponseParseError(String),

    #[error("Invalid MCP response format: {0}")]
    InvalidMcpFormat(String),

    #[error("Bedrock API error: {0}")]
    ApiError(String),

    #[error("Request cancelled")]
    Cancelled,
}

// Map of active requests that can be cancelled
type RequestMap = Arc<Mutex<HashMap<String, bool>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConfig {
    pub model_id: String,
    pub max_tokens: usize,
    pub temperature: f32,
    pub region: Option<String>,
    pub system_prompt: Option<String>,
    pub top_p: f32,
}

// Create a separate struct for the AWS region that will be used as a static reference
#[derive(Debug)]
struct AwsRegionHolder {
    region_string: String,
}

impl BedrockConfig {
    pub fn new(model_id: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            region: None,
            system_prompt: None,
            top_p: 0.9,
        }
    }

    pub fn claude() -> Self {
        Self::new("anthropic.claude-3-sonnet-20240229-v1:0")
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_region(mut self, region: String) -> Self {
        self.region = Some(region);
        self
    }

    pub fn with_system_prompt(mut self, system_prompt: String) -> Self {
        self.system_prompt = Some(system_prompt);
        self
    }
}

// Request Payload for Claude on Bedrock
#[derive(Debug, Serialize, Deserialize)]
struct ClaudePayload {
    anthropic_version: String,
    max_tokens: usize,
    messages: Vec<ClaudeMessage>,
    system: String,
    temperature: f32,
    top_p: f32,
}

// Claude Message Format
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: Vec<ClaudeContent>,
}

// Claude Message Content
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

// Claude Response Format
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeResponse {
    id: String,
    content: Vec<ClaudeResponseContent>,
    model: String,
    role: String,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    stop_sequence: Option<String>,
    usage: ClaudeUsage,
    #[serde(default)]
    type_: Option<String>,
}

// Claude Response Content
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

// Claude Streaming Response
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeStreamingResponse {
    #[serde(default)]
    delta: Option<ClaudeDelta>,
    #[serde(default)]
    message: Option<ClaudeResponse>,
    #[serde(default)]
    content_block: Option<ClaudeResponseContent>,
    #[serde(default)]
    usage: Option<ClaudeUsage>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    stop_sequence: Option<String>,
    #[serde(default)]
    type_: Option<String>,
}

// Claude Delta for streaming
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeDelta {
    #[serde(default)]
    text: String,
    #[serde(rename = "type", default)]
    type_: Option<String>,
    #[serde(default)]
    index: Option<usize>,
}

// Claude Usage Statistics
#[derive(Debug, Serialize, Deserialize)]
struct ClaudeUsage {
    input_tokens: usize,
    output_tokens: usize,
}

pub struct BedrockClient {
    client: BedrockRuntimeClient,
    config: BedrockConfig,
    schema_manager: McpSchemaManager,
    active_requests: RequestMap,
}

impl BedrockClient {
    pub async fn new(config: BedrockConfig) -> Result<Self> {
        // Configure AWS SDK
        let aws_config = if let Some(region) = &config.region {
            // Create a static region holder that will outlive the config
            let region_holder = Box::new(AwsRegionHolder {
                region_string: region.clone(),
            });
            // Leak this box to create a 'static reference
            // This is safe in this context as we need the region string to live for
            // the entire duration of the application
            let region_static = Box::leak(region_holder);

            aws_config::defaults(aws_config::BehaviorVersion::latest())
                .region(region_static.region_string.as_str())
                .load()
                .await
        } else {
            aws_config::defaults(aws_config::BehaviorVersion::latest())
                .load()
                .await
        };

        // Create Bedrock runtime client
        let client = BedrockRuntimeClient::new(&aws_config);

        Ok(Self {
            client,
            config,
            schema_manager: McpSchemaManager::new(),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    // Helper method to convert conversation context to Claude format
    fn prepare_claude_payload(&self, context: &ConversationContext) -> ClaudePayload {
        let mut claude_messages = Vec::new();

        // Get the enhanced system prompt (including MCP instructions)
        let system_prompt = match &self.config.system_prompt {
            Some(custom_prompt) => format!(
                "{}\n\n{}",
                self.schema_manager.get_mcp_system_prompt(),
                custom_prompt
            ),
            None => self.schema_manager.get_mcp_system_prompt().to_string(),
        };

        // Convert conversation messages to Claude format
        for message in &context.messages {
            let role = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                // System messages are handled separately in Claude
                MessageRole::System => continue,
                // Tool results should be added as assistant messages
                MessageRole::Tool => "assistant",
            };

            let content = ClaudeContent {
                content_type: "text".to_string(),
                text: message.content.clone(),
            };

            claude_messages.push(ClaudeMessage {
                role: role.to_string(),
                content: vec![content],
            });
        }

        ClaudePayload {
            anthropic_version: "bedrock-2023-05-31".to_string(),
            max_tokens: self.config.max_tokens,
            messages: claude_messages,
            system: system_prompt,
            temperature: self.config.temperature,
            top_p: self.config.top_p,
        }
    }

    // Parse a response from Claude into an MCP response
    fn parse_claude_response(&self, response: &ClaudeResponse) -> Result<LlmResponse> {
        let content = response
            .content
            .iter()
            .filter(|c| c.content_type == "text")
            .map(|c| c.text.clone())
            .collect::<Vec<String>>()
            .join("\n");

        // Attempt to parse as JSON
        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json_value) => {
                // Try to validate as MCP response
                if let Ok(mcp_response) = serde_json::from_value::<McpResponse>(json_value.clone())
                {
                    debug!("Received valid MCP response");

                    // Check if it's a tool call
                    if let Some(result) = mcp_response.result {
                        // Regular response
                        Ok(LlmResponse {
                            id: response.id.clone(),
                            content: result.to_string(),
                            tool_calls: Vec::new(),
                        })
                    } else if mcp_response.error.is_some() {
                        // Error response
                        let error = mcp_response.error.unwrap();
                        Err(anyhow!("LLM returned error: {}", error.message))
                    } else {
                        // Must be a tool call
                        match self.extract_tool_calls(&json_value) {
                            Ok(tool_calls) => Ok(LlmResponse {
                                id: response.id.clone(),
                                content: String::new(), // Empty content for tool calls
                                tool_calls,
                            }),
                            Err(e) => Err(e),
                        }
                    }
                } else {
                    // Not a valid MCP response
                    warn!("Response is JSON but not valid MCP format: {}", content);

                    // Fallback - treat as regular text
                    Ok(LlmResponse {
                        id: response.id.clone(),
                        content,
                        tool_calls: Vec::new(),
                    })
                }
            }
            Err(_) => {
                // Not JSON, treat as regular text response
                debug!("Response is not JSON, treating as regular text");
                Ok(LlmResponse {
                    id: response.id.clone(),
                    content,
                    tool_calls: Vec::new(),
                })
            }
        }
    }

    // Extract tool calls from an MCP request
    fn extract_tool_calls(&self, json_value: &serde_json::Value) -> Result<Vec<ClientToolCall>> {
        if let Ok(mcp_request) = serde_json::from_value::<McpRequest>(json_value.clone()) {
            if mcp_request.method == "mcp.tool_call" {
                if let Some(name) = mcp_request.params.get("name") {
                    if let Some(name_str) = name.as_str() {
                        if let Some(parameters) = mcp_request.params.get("parameters") {
                            let tool_call = ClientToolCall {
                                id: Uuid::new_v4().to_string(),
                                tool: name_str.to_string(),
                                params: parameters.clone(),
                            };
                            return Ok(vec![tool_call]);
                        }
                    }
                }
            }
        }

        Err(anyhow!("Unable to extract tool call from response"))
    }
}

#[async_trait]
impl LlmClient for BedrockClient {
    async fn send_message(&self, context: &ConversationContext) -> Result<LlmResponse> {
        // Generate a request ID and register it for possible cancellation
        let request_id = Uuid::new_v4().to_string();
        {
            let mut active_requests = self.active_requests.lock().unwrap();
            active_requests.insert(request_id.clone(), false);
        }

        // Prepare the Claude-specific payload
        let claude_payload = self.prepare_claude_payload(context);
        let payload_bytes = serde_json::to_vec(&claude_payload)?;

        debug!("Sending request to Bedrock: {}", self.config.model_id);
        debug!(
            "Request payload: {}",
            String::from_utf8_lossy(&payload_bytes)
        );

        // Log detailed request JSON at TRACE level (only shown with LOG_LEVEL=trace)
        trace!(
            "Raw JSON request payload to Bedrock: {}",
            serde_json::to_string_pretty(&claude_payload).unwrap_or_default()
        );

        // Send the request to Bedrock
        let output = match self
            .client
            .invoke_model()
            .body(Blob::new(payload_bytes))
            .model_id(&self.config.model_id)
            .send()
            .await
        {
            Ok(response) => response,
            Err(err) => {
                error!("Bedrock API error: {:?}", err);
                // Remove from active requests
                let mut active_requests = self.active_requests.lock().unwrap();
                active_requests.remove(&request_id);
                return Err(anyhow!(BedrockError::ApiError(err.to_string())));
            }
        };

        // Parse the response
        let response_bytes = output.body;
        let response_str = String::from_utf8(response_bytes.as_ref().to_vec())?;
        // Log full raw response at TRACE level (only shown with LOG_LEVEL=trace)
        trace!("Raw JSON response from Bedrock: {}", response_str);

        // Check if request was cancelled
        {
            let active_requests = self.active_requests.lock().unwrap();
            if let Some(cancelled) = active_requests.get(&request_id) {
                if *cancelled {
                    return Err(anyhow!(BedrockError::Cancelled));
                }
            }
        }

        // Remove from active requests
        {
            let mut active_requests = self.active_requests.lock().unwrap();
            active_requests.remove(&request_id);
        }

        // Parse the Claude response
        match serde_json::from_str::<ClaudeResponse>(&response_str) {
            Ok(claude_response) => {
                debug!(
                    "Successfully parsed Claude response: {:?}",
                    claude_response.id
                );

                // Log structured parsed response at TRACE level
                trace!(
                    "Parsed Claude response structure: {}",
                    serde_json::to_string_pretty(&claude_response).unwrap_or_default()
                );

                self.parse_claude_response(&claude_response)
            }
            Err(err) => {
                warn!("Failed to parse Claude response: {}", err);
                warn!("Response string: {}", response_str);

                // Try alternative parsing strategies
                debug!("Attempting to extract content from non-standard response");

                // Try to parse as JSON even if it's not the expected structure
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&response_str) {
                    // See if there's a content field that contains text
                    if let Some(content) = json_value.get("content") {
                        if let Some(text) = content.as_str() {
                            debug!("Extracted content from non-standard response");
                            return Ok(LlmResponse {
                                id: request_id,
                                content: text.to_string(),
                                tool_calls: Vec::new(),
                            });
                        }
                    }

                    // Return the raw JSON as content as a last resort
                    debug!("Returning raw JSON as content");
                    return Ok(LlmResponse {
                        id: request_id,
                        content: response_str,
                        tool_calls: Vec::new(),
                    });
                }

                Err(anyhow!(BedrockError::ResponseParseError(err.to_string())))
            }
        }
    }

    async fn stream_message(
        &self,
        context: &ConversationContext,
    ) -> Result<Box<dyn Stream<Item = Result<StreamChunk>> + Unpin + Send>> {
        // Generate a request ID and register it for possible cancellation
        let request_id = Uuid::new_v4().to_string();
        {
            let mut active_requests = self.active_requests.lock().unwrap();
            active_requests.insert(request_id.clone(), false);
        }

        // Prepare the Claude-specific payload
        let claude_payload = self.prepare_claude_payload(context);
        let payload_bytes = serde_json::to_vec(&claude_payload)?;

        debug!(
            "Sending streaming request to Bedrock: {}",
            self.config.model_id
        );
        debug!(
            "Request payload: {}",
            String::from_utf8_lossy(&payload_bytes)
        );

        // Log detailed request JSON at TRACE level (only shown with LOG_LEVEL=trace)
        trace!(
            "Raw streaming JSON request payload to Bedrock: {}",
            serde_json::to_string_pretty(&claude_payload).unwrap_or_default()
        );

        // Create a channel for the stream
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<StreamChunk>>(100);

        // Clone needed data for the async task
        let client = self.client.clone();
        let model_id = self.config.model_id.clone();
        let active_requests = self.active_requests.clone();
        let request_id_clone = request_id.clone();

        // We need to ensure we don't use self in the async block
        // Extract what we need from the response to avoid borrowing issues
        let extract_tool_calls = |json_value: &serde_json::Value| -> Option<ClientToolCall> {
            if let Ok(mcp_request) = serde_json::from_value::<McpRequest>(json_value.clone()) {
                if mcp_request.method == "mcp.tool_call" {
                    if let Some(name) = mcp_request.params.get("name") {
                        if let Some(name_str) = name.as_str() {
                            if let Some(parameters) = mcp_request.params.get("parameters") {
                                let tool_call = ClientToolCall {
                                    id: Uuid::new_v4().to_string(),
                                    tool: name_str.to_string(),
                                    params: parameters.clone(),
                                };
                                return Some(tool_call);
                            }
                        }
                    }
                }
            }
            None
        };

        // Spawn a task to run the request and process the response
        tokio::spawn(async move {
            debug!("We're not using streaming for now, using regular invoke_model");
            debug!("In AWS SDK 1.x, the streaming APIs are difficult to work with");

            // Send a regular request instead of streaming
            let output = match client
                .invoke_model()
                .body(Blob::new(payload_bytes))
                .model_id(&model_id)
                .send()
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    error!("Bedrock API error: {:?}", err);

                    // Send error through the channel first
                    let _ = tx
                        .send(Err(anyhow!(BedrockError::ApiError(err.to_string()))))
                        .await;

                    // Remove from active requests
                    {
                        let mut active_requests = active_requests.lock().unwrap();
                        active_requests.remove(&request_id_clone);
                    }
                    return;
                }
            };

            debug!("Received response from Bedrock");

            // Get response body
            let response_bytes = output.body;
            let response_str = String::from_utf8_lossy(response_bytes.as_ref()).to_string();
            // Log full raw response at TRACE level (only shown with LOG_LEVEL=trace)
            trace!("Raw JSON streaming response from Bedrock: {}", response_str);

            // Check if request was cancelled
            {
                let active_requests = active_requests.lock().unwrap();
                if let Some(cancelled) = active_requests.get(&request_id_clone) {
                    if *cancelled {
                        debug!("Request {} was cancelled", request_id_clone);
                        return;
                    }
                }
            }

            // Parse the Claude response
            match serde_json::from_str::<ClaudeResponse>(&response_str) {
                Ok(claude_response) => {
                    debug!(
                        "Successfully parsed Claude response: {:?}",
                        claude_response.id
                    );

                    // Log structured parsed response at TRACE level
                    trace!(
                        "Parsed Claude streaming response structure: {}",
                        serde_json::to_string_pretty(&claude_response).unwrap_or_default()
                    );

                    // Get the text content from Claude
                    let content = claude_response
                        .content
                        .iter()
                        .filter(|c| c.content_type == "text")
                        .map(|c| c.text.clone())
                        .collect::<Vec<String>>()
                        .join("\n");

                    // Emit the content as a stream
                    let stream_chunk = StreamChunk {
                        id: request_id_clone.clone(),
                        content: content.clone(),
                        is_tool_call: false,
                        tool_call: None,
                        is_complete: false,
                    };

                    if let Err(e) = tx.send(Ok(stream_chunk)).await {
                        error!("Failed to send content chunk to stream: {}", e);
                    }

                    // Try to parse as MCP
                    let is_mcp_response = is_valid_json(&content);

                    if is_mcp_response {
                        debug!("Content appears to be valid JSON, checking for tool calls");

                        // Try to extract tool calls
                        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&content)
                        {
                            if let Some(tool_call) = extract_tool_calls(&json_value) {
                                debug!("Found tool call in response");

                                // Send a tool call chunk
                                let final_chunk = StreamChunk {
                                    id: request_id_clone.clone(),
                                    content: String::new(), // Already sent in previous chunk
                                    is_tool_call: true,
                                    tool_call: Some(tool_call),
                                    is_complete: true,
                                };

                                if let Err(e) = tx.send(Ok(final_chunk)).await {
                                    error!("Failed to send tool call chunk to stream: {}", e);
                                }

                                // Clean up and exit
                                {
                                    let mut active_requests = active_requests.lock().unwrap();
                                    active_requests.remove(&request_id_clone);
                                }
                                return;
                            }
                        }
                    }

                    // Send completion
                    let final_chunk = StreamChunk {
                        id: request_id_clone.clone(),
                        content: String::new(), // Already sent in previous chunk
                        is_tool_call: false,
                        tool_call: None,
                        is_complete: true,
                    };

                    if let Err(e) = tx.send(Ok(final_chunk)).await {
                        error!("Failed to send completion chunk to stream: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to parse Claude response: {}", e);

                    // Send the raw content anyway
                    let stream_chunk = StreamChunk {
                        id: request_id_clone.clone(),
                        content: response_str,
                        is_tool_call: false,
                        tool_call: None,
                        is_complete: true,
                    };

                    if let Err(send_err) = tx.send(Ok(stream_chunk)).await {
                        error!("Failed to send raw content: {}", send_err);
                    }
                }
            }

            // Remove from active requests
            {
                let mut active_requests = active_requests.lock().unwrap();
                active_requests.remove(&request_id_clone);
            }
        });

        // Return the receiver as a stream
        Ok(Box::new(ReceiverStream::new(rx)))
    }

    fn cancel_request(&self, request_id: &str) -> Result<()> {
        let mut active_requests = self.active_requests.lock().unwrap();
        if let Some(cancelled) = active_requests.get_mut(request_id) {
            *cancelled = true;
            debug!("Marked request {} as cancelled", request_id);
            Ok(())
        } else {
            Err(anyhow!("Request ID not found: {}", request_id))
        }
    }
}

// Helper function to check if a string is valid JSON
fn is_valid_json(s: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}
