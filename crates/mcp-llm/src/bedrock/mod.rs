use crate::client_trait::{LlmClient, LlmResponse, StreamChunk, ToolCall as ClientToolCall};
use crate::schema::McpSchemaManager;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_bedrockruntime::Client as BedrockRuntimeClient;
use aws_smithy_types::Blob;
use futures::Stream;
use mcp_core::context::{ConversationContext, MessageRole};
use mcp_core::prompts::{PromptManager, TemplateEngine};
use mcp_core::protocol::{Request as McpRequest, Response as McpResponse};
use mcp_metrics::{count, time};
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
    client: Option<BedrockRuntimeClient>,
    config: BedrockConfig,
    schema_manager: McpSchemaManager,
    prompt_manager: PromptManager,
    active_requests: RequestMap,
    tools_documentation: Option<String>,
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
            client: Some(client),
            config,
            schema_manager: McpSchemaManager::new(),
            prompt_manager: PromptManager::new(),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
            tools_documentation: None,
        })
    }

    /// Create a new client with custom tool documentation
    pub async fn with_tool_documentation(config: BedrockConfig, tools_doc: String) -> Result<Self> {
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
            client: Some(client),
            config,
            schema_manager: McpSchemaManager::new(),
            prompt_manager: PromptManager::new(),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
            tools_documentation: Some(tools_doc),
        })
    }

    // Helper method to convert conversation context to Claude format
    fn prepare_claude_payload(&self, context: &ConversationContext) -> ClaudePayload {
        let mut claude_messages = Vec::new();

        // Create a template engine with variables for the system prompt
        let mut engine = TemplateEngine::new();

        // Add model-specific variables
        engine.set_var("model_id", &self.config.model_id);
        engine.set_var("max_tokens", &self.config.max_tokens.to_string());

        // Add session info
        engine.set_var("conversation_length", &context.messages.len().to_string());

        // Get the system prompt from the prompt manager with template variables substituted
        let mut system_prompt = self.prompt_manager.get_rendered_system_prompt(&engine);

        // Add custom prompt if provided in config
        if let Some(custom_prompt) = &self.config.system_prompt {
            system_prompt = format!("{}\n\n{}", system_prompt, custom_prompt);
        }

        // Add MCP system prompt - use dynamic tool documentation if available
        if let Some(tools_doc) = &self.tools_documentation {
            // Use the custom tool documentation with MCP system prompt
            let mcp_prompt = self
                .schema_manager
                .get_mcp_system_prompt_with_tools(tools_doc);
            system_prompt = format!("{}\n\n{}", system_prompt, mcp_prompt);
        } else {
            // Use the standard MCP system prompt
            let mcp_prompt = self.schema_manager.get_mcp_system_prompt();
            system_prompt = format!("{}\n\n{}", system_prompt, mcp_prompt);
        }

        debug!(
            "Using system prompt with {} characters",
            system_prompt.len()
        );

        // Convert conversation messages to Claude format
        for message in &context.messages {
            let role = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                // System messages are handled separately in Claude
                MessageRole::System => continue,
                // Tool results require special handling for Claude
                MessageRole::Tool => {
                    // Parse the tool message if it's in JSON-RPC format
                    if let Ok(json_value) =
                        serde_json::from_str::<serde_json::Value>(&message.content)
                    {
                        // Check if it has the JSON-RPC result field
                        if let Some(result) = json_value.get("result") {
                            // Format as a special tool result message for Claude
                            let tool_msg = format!(
                                "I've received the following tool result:\n```json\n{}\n```\n\nNow I need to provide a direct answer based on this result.",
                                serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
                            );

                            let content = ClaudeContent {
                                content_type: "text".to_string(),
                                text: tool_msg,
                            };

                            claude_messages.push(ClaudeMessage {
                                role: "assistant".to_string(),
                                content: vec![content],
                            });

                            continue;
                        }
                    }

                    // Fallback for non-JSON tool results
                    "assistant"
                }
            };

            // For regular messages (or fallback for tool messages)
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
                        // Regular response - extract the actual content
                        // If result is a string, use it directly
                        let content = if let Some(text) = result.as_str() {
                            text.to_string()
                        } else {
                            // Otherwise stringify it, preserving formatting
                            result.to_string()
                        };

                        Ok(LlmResponse {
                            id: response.id.clone(),
                            content,
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

        // Count API calls
        count!("llm.calls.total");
        count!("llm.calls.bedrock");

        // Prepare the Claude-specific payload
        let claude_payload = self.prepare_claude_payload(context);
        let payload_bytes = serde_json::to_vec(&claude_payload)?;

        // Count tokens (approximation)
        let input_tokens = context
            .messages
            .iter()
            .map(|m| m.content.len() / 4)
            .sum::<usize>();
        count!("llm.tokens.input", input_tokens as u64);

        debug!("Sending request to Bedrock: {}", self.config.model_id);
        debug!(
            "Request payload: {}",
            String::from_utf8_lossy(&payload_bytes)
        );

        // Only log the raw request at TRACE level, keeping just this critical logging point
        trace!(
            ">>> RAW REQUEST TO LLM >>>\n{}",
            serde_json::to_string_pretty(&claude_payload).unwrap_or_default()
        );

        // Send the request to Bedrock with timing
        // For tests, we skip the actual API call
        let output = if let Some(client) = &self.client {
            time!("llm.response_time.bedrock", {
                match client
                    .invoke_model()
                    .body(Blob::new(payload_bytes))
                    .model_id(&self.config.model_id)
                    .send()
                    .await
                {
                    Ok(response) => response,
                    Err(err) => {
                        error!("Bedrock API error: {:?}", err);
                        // Count error
                        count!("llm.errors");
                        count!("llm.errors.bedrock");

                        // Remove from active requests
                        let mut active_requests = self.active_requests.lock().unwrap();
                        active_requests.remove(&request_id);
                        return Err(anyhow!(BedrockError::ApiError(err.to_string())));
                    }
                }
            })
        } else {
            // This is a test-only path, create a mock response
            debug!("Using mock response for tests");
            // Remove request from tracking
            let mut active_requests = self.active_requests.lock().unwrap();
            active_requests.remove(&request_id);

            // We're in test mode, return a mock response that parseClaudeResponse can handle
            aws_sdk_bedrockruntime::operation::invoke_model::InvokeModelOutput::builder()
                .content_type("application/json")
                .body(Blob::new(r#"{"id":"test-id","content":[{"type":"text","text":"This is a mock response for testing"}],"type":"message","role":"assistant","model":"claude-3-sonnet-20240229-v1:0"}"#))
                .build()
                .expect("Failed to build mock response")
        };

        // Parse the response
        let response_bytes = output.body;
        let response_str = String::from_utf8(response_bytes.as_ref().to_vec())?;
        // Log full raw response at TRACE level (only shown with LOG_LEVEL=trace)
        trace!("<<< RAW RESPONSE FROM LLM <<<\n{}", response_str);

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

                let response = self.parse_claude_response(&claude_response)?;

                // Count output tokens (rough approximation)
                let output_tokens = response.content.len() / 4;
                count!("llm.tokens.output", output_tokens as u64);

                // Count successful completion
                count!("llm.completions.success");

                // Count tool calls if any
                if !response.tool_calls.is_empty() {
                    count!("llm.tool_calls", response.tool_calls.len() as u64);
                }

                Ok(response)
            }
            Err(err) => {
                warn!("Failed to parse Claude response: {}", err);
                warn!("Response string: {}", response_str);

                // Count parsing error
                count!("llm.errors.parsing");

                // Try alternative parsing strategies
                debug!("Attempting to extract content from non-standard response");

                // Try to parse as JSON even if it's not the expected structure
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&response_str) {
                    // See if there's a content field that contains text
                    if let Some(content) = json_value.get("content") {
                        if let Some(text) = content.as_str() {
                            debug!("Extracted content from non-standard response");

                            // Count output tokens (rough approximation)
                            let output_tokens = text.len() / 4;
                            count!("llm.tokens.output", output_tokens as u64);

                            return Ok(LlmResponse {
                                id: request_id,
                                content: text.to_string(),
                                tool_calls: Vec::new(),
                            });
                        }
                    }

                    // Return the raw JSON as content as a last resort
                    debug!("Returning raw JSON as content");

                    // Count output tokens (rough approximation)
                    let output_tokens = response_str.len() / 4;
                    count!("llm.tokens.output", output_tokens as u64);

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

        // Only log the raw request at TRACE level, keeping just this critical logging point
        trace!(
            ">>> RAW STREAMING REQUEST TO LLM >>>\n{}",
            serde_json::to_string_pretty(&claude_payload).unwrap_or_default()
        );

        // Create a channel for the stream
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<StreamChunk>>(100);

        // Clone needed data for the async task
        let client = self.client.clone();
        let model_id = self.config.model_id.clone();
        let active_requests = self.active_requests.clone();
        let request_id_clone = request_id.clone();
        let is_test_mode = client.is_none();

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
            let output = if is_test_mode {
                // This is a test-only path, create a mock response
                debug!("Using mock streaming response for tests");

                // We're in test mode, return a mock response
                aws_sdk_bedrockruntime::operation::invoke_model::InvokeModelOutput::builder()
                    .content_type("application/json")
                    .body(Blob::new(r#"{"id":"test-id","content":[{"type":"text","text":"This is a mock streaming response for testing"}],"type":"message","role":"assistant","model":"claude-3-sonnet-20240229-v1:0"}"#))
                    .build()
                    .expect("Failed to build mock streaming response")
            } else {
                // This is the normal operation path
                match client
                    .unwrap()
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
                }
            };

            debug!("Received response from Bedrock");

            // Get response body
            let response_bytes = output.body;
            let response_str = String::from_utf8_lossy(response_bytes.as_ref()).to_string();
            // Log full raw response at TRACE level (only shown with LOG_LEVEL=trace)
            trace!("<<< RAW STREAMING RESPONSE FROM LLM <<<\n{}", response_str);

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

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_core::context::{ConversationContext, Message, MessageRole};

    // Test the dynamic tool documentation generation
    #[test]
    fn test_dynamic_tool_documentation() {
        let config = BedrockConfig::claude();
        let tools_doc = "1. \"test_tool\": This is a test tool\n   Parameters: {\n     \"param1\": \"string\",           // Required parameter\n     \"param2\": \"number\",           // Optional: Another parameter\n   }\n";

        // Create a mock BedrockRuntimeClient for testing
        // In tests, we don't need a real client
        let mock_client = None; // Using None to indicate this is a test-only client

        let client = BedrockClient {
            client: mock_client,
            config,
            schema_manager: McpSchemaManager::new(),
            prompt_manager: PromptManager::new(),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
            tools_documentation: Some(tools_doc.to_string()),
        };

        let context = ConversationContext {
            system_prompt: String::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_calls: None,
                tool_results: None,
            }],
            current_request_id: None,
        };

        let payload = client.prepare_claude_payload(&context);

        // Check that the system prompt contains our tool documentation
        assert!(payload.system.contains("test_tool"));
        assert!(payload.system.contains("This is a test tool"));
        assert!(payload.system.contains("Required parameter"));
        assert!(payload.system.contains("Optional: Another parameter"));
    }

    // Test that regular (non-dynamic) tool documentation is used when no custom docs provided
    #[test]
    fn test_default_tool_documentation() {
        let config = BedrockConfig::claude();

        // Create a mock BedrockRuntimeClient for testing
        // In tests, we don't need a real client
        let mock_client = None; // Using None to indicate this is a test-only client

        let client = BedrockClient {
            client: mock_client,
            config,
            schema_manager: McpSchemaManager::new(),
            prompt_manager: PromptManager::new(),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
            tools_documentation: None,
        };

        let context = ConversationContext {
            system_prompt: String::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_calls: None,
                tool_results: None,
            }],
            current_request_id: None,
        };

        let payload = client.prepare_claude_payload(&context);

        // Check that the system prompt contains the default tools
        assert!(payload.system.contains("\"shell\""));
        assert!(payload.system.contains("\"file_read\""));
        assert!(payload.system.contains("Model Context Protocol (MCP)"));
    }
}
