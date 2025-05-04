use anyhow::{anyhow, Result};
use futures::{Stream, StreamExt};
use mcp_core::context::{ConversationContext, MessageRole};
use mcp_core::{api_log, debug_log};
use mcp_llm::{BedrockClient, BedrockConfig, LlmClient, StreamChunk};
use mcp_metrics::{count, gauge, time};
use mcp_tools::{
    filesystem::{FilesystemConfig, ListDirectoryTool, ReadFileTool, WriteFileTool},
    search::{FindConfig, FindTool, GrepConfig, GrepTool},
    shell::{ShellConfig, ShellTool},
    ToolManager, ToolResult, ToolStatus,
};
use serde_json::Value;
use std::io::Write as IoWrite;
use std::sync::Arc;
use tracing::{debug, error, info, trace, warn};

use crate::formatter::{format_llm_response, ResponseFormatter};

// ========== Helper structs ==========

// Represents a tool execution result including the result or error
struct FormattedToolResult {
    // The JSON-RPC formatted result string for adding to context
    formatted_result: String,
    // The original tool result for display formatting
    original_result: ToolResult,
}

// Represents a follow-up response after tool execution
struct FollowUpResponse {
    content: String,
    is_empty_or_tool_call: bool,
}

// Export our modules
pub mod cli_main;
pub mod formatter;
pub mod mock;

pub struct CliApp {
    context: ConversationContext,
    llm_client: Option<Arc<dyn LlmClient + Send + Sync>>,
    config: CliConfig,
    tool_manager: ToolManager,
}

#[derive(Debug)]
pub struct CliConfig {
    pub model: String,
    pub use_mcp: bool,
    pub region: Option<String>,
    pub streaming: bool,
    pub enable_tools: bool,
    pub require_tool_confirmation: bool,
    pub auto_approve_tools: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            model: "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
            use_mcp: true, // Enable MCP by default for tool execution
            region: None,
            streaming: true,
            enable_tools: true,              // Enable tool execution by default
            require_tool_confirmation: true, // Require user confirmation for tool execution
            auto_approve_tools: false,       // Don't auto-approve tools by default
        }
    }
}

impl CliApp {
    pub fn new() -> Self {
        // Create a new tool manager
        let mut tool_manager = ToolManager::new();

        // Register the shell tool with configuration
        let shell_config = ShellConfig {
            default_timeout_ms: 30000, // 30 seconds default timeout
            max_timeout_ms: 300000,    // 5 minutes maximum timeout
            allowed_commands: None,    // No specific whitelist
            denied_commands: Some(vec![
                "rm -rf".to_string(),   // Prevent dangerous recursive deletion
                "sudo".to_string(),     // Prevent sudo commands
                "chmod".to_string(),    // Prevent permission changes
                "chown".to_string(),    // Prevent ownership changes
                "mkfs".to_string(),     // Prevent formatting
                "dd".to_string(),       // Prevent raw disk operations
                "shutdown".to_string(), // Prevent shutdown
                "reboot".to_string(),   // Prevent reboot
                "halt".to_string(),     // Prevent halt
            ]),
        };

        let shell_tool = ShellTool::with_config(shell_config);
        tool_manager.register_tool(Box::new(shell_tool));

        // Register filesystem tools with default configuration
        let filesystem_config = FilesystemConfig {
            // Use default denied paths to protect sensitive areas
            denied_paths: Some(vec![
                "/etc/".to_string(),
                "/var/".to_string(),
                "/usr/".to_string(),
                "/bin/".to_string(),
                "/sbin/".to_string(),
                "/.ssh/".to_string(),
                "/.aws/".to_string(),
                "/.config/".to_string(),
                "C:\\Windows\\".to_string(),
                "C:\\Program Files\\".to_string(),
                "C:\\Program Files (x86)\\".to_string(),
            ]),
            allowed_paths: None, // Allow all paths not explicitly denied
            max_file_size: 10 * 1024 * 1024, // 10 MB max file size
        };

        let read_file_tool = ReadFileTool::with_config(filesystem_config.clone());
        tool_manager.register_tool(Box::new(read_file_tool));

        let write_file_tool = WriteFileTool::with_config(filesystem_config.clone());
        tool_manager.register_tool(Box::new(write_file_tool));

        let list_dir_tool = ListDirectoryTool::with_config(filesystem_config.clone());
        tool_manager.register_tool(Box::new(list_dir_tool));

        // Register search tools
        let grep_config = GrepConfig {
            denied_paths: filesystem_config.denied_paths.clone(),
            allowed_paths: filesystem_config.allowed_paths.clone(),
            ..GrepConfig::default()
        };
        let grep_tool = GrepTool::with_config(grep_config);
        tool_manager.register_tool(Box::new(grep_tool));

        let find_config = FindConfig {
            denied_paths: filesystem_config.denied_paths.clone(),
            allowed_paths: filesystem_config.allowed_paths.clone(),
            ..FindConfig::default()
        };
        let find_tool = FindTool::with_config(find_config);
        tool_manager.register_tool(Box::new(find_tool));

        // Register diff and patch tools
        let diff_tool = mcp_tools::diff::DiffTool::new();
        tool_manager.register_tool(Box::new(diff_tool));

        let patch_tool = mcp_tools::diff::PatchTool::new();
        tool_manager.register_tool(Box::new(patch_tool));

        // Register project navigator tool
        let project_navigator = mcp_tools::analysis::ProjectNavigator::new();
        tool_manager.register_tool(Box::new(project_navigator));

        Self {
            context: ConversationContext::new(),
            llm_client: None,
            config: CliConfig::default(),
            tool_manager,
        }
    }

    pub fn with_config(mut self, config: CliConfig) -> Self {
        self.config = config;
        self
    }

    // Add a method to handle tool calls
    async fn execute_tool(&mut self, tool_id: &str, params: Value) -> Result<ToolResult> {
        // Check if tools are enabled
        if !self.config.enable_tools {
            return Ok(ToolResult {
                tool_id: tool_id.to_string(),
                status: ToolStatus::Failure,
                output: serde_json::json!({
                    "error": "Tool execution is disabled"
                }),
                error: Some("Tool execution is disabled in the configuration".to_string()),
            });
        }

        debug!("Executing tool: {} with params: {}", tool_id, params);

        // Get user confirmation if required and auto-approve is not enabled
        if self.config.require_tool_confirmation && !self.config.auto_approve_tools {
            println!("\n[Tool Execution Request]");
            println!("Tool: {}", tool_id);
            println!("Parameters: {}", params);
            print!("Allow execution? [y/N]: ");
            std::io::stdout().flush().unwrap();

            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();

            if !input.trim().eq_ignore_ascii_case("y") {
                debug!("Tool execution denied by user");
                return Ok(ToolResult {
                    tool_id: tool_id.to_string(),
                    status: ToolStatus::Failure,
                    output: serde_json::json!({
                        "error": "Tool execution denied by user"
                    }),
                    error: Some("User denied tool execution".to_string()),
                });
            }
        } else if self.config.auto_approve_tools {
            // If auto-approve is enabled, log and inform
            debug!("Tool execution auto-approved: {}", tool_id);
            if self.config.require_tool_confirmation {
                println!("\n[Tool Execution Auto-Approved]");
                println!("Tool: {}", tool_id);
                println!("Parameters: {}", params);
            }
        }

        // Track metrics
        count!("tool.executions.total");
        count!(format!("tool.executions.{}", tool_id).as_str());

        // Execute the tool with timing
        let result = time!(format!("tool.execution_time.{}", tool_id).as_str(), {
            self.tool_manager.execute_tool(tool_id, params).await
        });

        // Track result metrics
        match &result {
            Ok(tool_result) => match tool_result.status {
                ToolStatus::Success => count!("tool.executions.success"),
                ToolStatus::Failure => count!("tool.executions.failure"),
                ToolStatus::Timeout => count!("tool.executions.timeout"),
            },
            Err(_) => {
                count!("tool.executions.error");
            }
        }

        result
    }

    // Add a method to set a custom LLM client (useful for testing)
    pub fn with_llm_client<T>(mut self, client: T) -> Self
    where
        T: LlmClient + Send + Sync + 'static,
    {
        self.llm_client = Some(Arc::new(client));
        self
    }

    // Helper method to handle Bedrock client initialization errors
    fn handle_bedrock_client_error(&self, e: anyhow::Error) -> Result<()> {
        // Print helpful error message about credentials
        eprintln!("Failed to initialize AWS Bedrock client: {}", e);
        eprintln!("Please ensure you have valid AWS credentials configured.");
        eprintln!("You can set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY environment variables,");
        eprintln!("or configure credentials in ~/.aws/credentials file.");
        eprintln!(
            "Also verify that the model ID '{}' is available in your AWS region.",
            self.config.model
        );

        Err(e)
    }

    pub async fn initialize(&mut self) -> Result<()> {
        // Check if we already have a client (could be a mock for testing)
        if self.llm_client.is_some() {
            debug_log("LLM client already initialized, skipping initialization");
            return Ok(());
        }

        // Create a BedrockConfig
        let mut bedrock_config = BedrockConfig::new(&self.config.model)
            .with_max_tokens(4096)
            .with_temperature(0.7);

        // Add region if provided
        if let Some(region) = &self.config.region {
            debug_log(&format!("Using AWS region: {}", region));
            bedrock_config = bedrock_config.with_region(region.clone());
        } else {
            debug_log("No AWS region specified, using default from AWS config");
        }

        // Add a system prompt based on whether MCP is enabled
        let system_prompt = if self.config.use_mcp {
            "You are Claude, a helpful AI assistant by Anthropic. You will follow the Model Context Protocol (MCP) for structured communication.".to_string()
        } else {
            "You are Claude, a helpful AI assistant by Anthropic.".to_string()
        };

        bedrock_config = bedrock_config.with_system_prompt(system_prompt);

        // Initialize the AWS SDK and create the Bedrock client
        debug_log(&format!(
            "Initializing Bedrock client with model: {}",
            self.config.model
        ));
        api_log(&format!("Bedrock config: {:?}", bedrock_config));

        // Record metrics
        count!("app.initialization");
        gauge!("app.mcp_enabled", if self.config.use_mcp { 1 } else { 0 });

        // Create the Bedrock client with dynamic tool documentation if MCP is enabled
        debug_log("Creating BedrockClient");
        let client = if self.config.use_mcp {
            // Generate tool documentation from the tool manager
            let tools_doc = self.tool_manager.generate_tool_documentation();
            debug_log(&format!(
                "Generated tool documentation with {} characters",
                tools_doc.len()
            ));
            trace!("Tool documentation: {}", tools_doc);

            // Create client with tool documentation
            match BedrockClient::with_tool_documentation(bedrock_config, tools_doc).await {
                Ok(client) => {
                    debug_log("Successfully created BedrockClient with dynamic tool documentation");
                    client
                }
                Err(e) => {
                    debug_log(&format!("Failed to create BedrockClient: {}", e));
                    self.handle_bedrock_client_error(e)?;
                    unreachable!(); // This line won't be reached as handle_bedrock_client_error always returns Err
                }
            }
        } else {
            // Create client without tool documentation for non-MCP mode
            match BedrockClient::new(bedrock_config).await {
                Ok(client) => {
                    debug_log("Successfully created BedrockClient");
                    client
                }
                Err(e) => {
                    debug_log(&format!("Failed to create BedrockClient: {}", e));
                    self.handle_bedrock_client_error(e)?;
                    unreachable!(); // This line won't be reached as handle_bedrock_client_error always returns Err
                }
            }
        };

        self.llm_client = Some(Arc::new(client));
        Ok(())
    }

    // ========== Main run method ==========

    pub async fn run(&mut self, prompt: &str) -> Result<String> {
        // Make sure the client is initialized
        if self.llm_client.is_none() {
            debug_log("Client not initialized, initializing now");
            self.initialize().await?;
        }

        //let _client = self.llm_client.as_ref().unwrap();

        // Add the user message to the conversation context
        debug_log(&format!("Adding user message: {}", prompt));
        self.context.add_user_message(prompt);

        // Record metrics
        count!("llm.requests.total");
        count!("llm.requests.bedrock");

        // Use streaming or regular response based on config
        if self.config.streaming {
            self.handle_streaming_response().await
        } else {
            self.handle_non_streaming_response().await
        }
    }

    // ========== Streaming response handling ==========

    async fn handle_streaming_response(&mut self) -> Result<String> {
        debug_log("Using streaming response");
        let response_content;
        let client = self.llm_client.as_ref().unwrap();

        // Record the time taken for the streaming response
        time!("llm.streaming_response_time", {
            debug_log("Sending streaming request to Bedrock");
            let stream_result = client.stream_message(&self.context).await;

            match stream_result {
                Ok(mut stream) => {
                    response_content = self.process_streaming_response(&mut stream).await?;
                }
                Err(e) => {
                    debug_log(&format!("Failed to start streaming: {}", e));
                    count!("llm.errors", 1);
                    return Err(anyhow!("Failed to start streaming: {}", e));
                }
            }
        });

        // Add the full response to the conversation
        self.context.add_assistant_message(&response_content);

        // Return the complete response
        Ok(response_content)
    }

    async fn process_streaming_response(
        &mut self,
        stream: &mut Box<dyn Stream<Item = Result<StreamChunk>> + Unpin + Send>,
    ) -> Result<String> {
        debug_log("Stream response received, processing chunks");

        // Show a subtle indicator that we're receiving a response
        println!(); // Start with a clean line

        // Track if we've received any content at all
        let mut received_content = false;
        let mut response_content = String::new();
        let mut had_tool_call = false;

        // Buffer to accumulate chunks until we know if they're part of a tool call
        let mut content_buffer = String::new();
        let mut is_current_buffer_tool_call = false;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    debug_log(&format!("Received chunk, {} bytes", chunk.content.len()));
                    api_log(&format!("Chunk content: {}", chunk.content));

                    if !chunk.content.is_empty() {
                        received_content = true;
                        response_content.push_str(&chunk.content);
                        count!("llm.stream_chunks", 1);

                        // Check if this content looks like a tool call JSON-RPC (do this before buffering)
                        let content_is_likely_tool_call = chunk.content.contains("\"jsonrpc\"")
                            && (chunk.content.contains("\"method\"")
                                || chunk.content.contains("\"mcp.tool_call\""));

                        if content_is_likely_tool_call {
                            // Mark as tool call preemptively to avoid displaying JSON-RPC
                            is_current_buffer_tool_call = true;
                            debug_log(&format!(
                                "Detected likely tool call in content: {}",
                                chunk.content
                            ));
                            // Don't add to buffer to avoid printing JSON-RPC
                        } else {
                            // Add to buffer only if not a likely tool call
                            content_buffer.push_str(&chunk.content);
                        }
                    }

                    // Check if this chunk is marked as a tool call
                    if chunk.is_tool_call {
                        is_current_buffer_tool_call = true;
                        had_tool_call = true;

                        // Empty the buffer without printing, since it's a tool call
                        content_buffer.clear();

                        if let Some(tool_call) = &chunk.tool_call {
                            self.handle_tool_call(tool_call).await?;
                        }
                    }

                    // If we have completed a chunk or this is the final chunk, process the buffer
                    if chunk.is_complete
                        || (!is_current_buffer_tool_call && chunk.content.contains("\n"))
                    {
                        // Only print if it's NOT part of a tool call
                        if !is_current_buffer_tool_call && !content_buffer.is_empty() {
                            debug_log(&format!(
                                "Printing chunk content (not a tool call): {}",
                                content_buffer
                            ));
                            self.print_chunk_content(&content_buffer);
                            content_buffer.clear();
                        } else if is_current_buffer_tool_call {
                            // Clear the buffer but don't print it if it's a tool call
                            debug_log("Skipping printing tool call JSON-RPC");
                            content_buffer.clear();
                        }

                        // Reset the flag for the next buffer
                        is_current_buffer_tool_call = false;
                    }

                    // If this is the final chunk, we're done
                    if chunk.is_complete {
                        debug!("Final chunk received");
                        println!(); // Add a newline after completion

                        // Check if we had a tool call and need a follow-up
                        if had_tool_call {
                            // Get a follow-up response with the tool results
                            let follow_up_result = self.get_streaming_follow_up_response().await?;
                            return Ok(follow_up_result);
                        }

                        break;
                    }
                }
                Err(e) => {
                    debug_log(&format!("Error in stream: {}", e));
                    count!("llm.errors", 1);
                    eprintln!("Error receiving response: {}", e);
                    return Err(anyhow!("Error in stream: {}", e));
                }
            }
        }

        // If we haven't received any content, that's an error
        if !received_content {
            debug_log("No content received from stream");
            return Err(anyhow!("No content received from Bedrock. Please check your AWS credentials and model availability."));
        }

        Ok(response_content)
    }

    fn print_chunk_content(&self, content: &str) {
        // Format the content to extract JSON-RPC result if present
        let formatted_content = format_llm_response(content);

        // Note: Colors class will automatically check if colors are supported
        print!("{}", formatted_content);
        let _ = std::io::stdout().flush();
    }

    async fn get_streaming_follow_up_response(&mut self) -> Result<String> {
        debug!("Getting follow-up response with tool results...");

        // Get a fresh reference to the client
        let follow_up_client = self.llm_client.as_ref().unwrap();
        let follow_up_result = follow_up_client.stream_message(&self.context).await;

        match follow_up_result {
            Ok(mut follow_up_stream) => {
                let follow_up = self
                    .collect_streaming_follow_up(&mut follow_up_stream)
                    .await?;

                // Check if the follow-up content is empty or contains another tool call
                if follow_up.is_empty_or_tool_call {
                    self.handle_problematic_streaming_follow_up().await
                } else {
                    // We have a valid follow-up response
                    self.context.add_assistant_message(&follow_up.content);
                    debug!(
                        "Received valid follow-up response after tool execution: length={} chars",
                        follow_up.content.len()
                    );

                    // Sleep to ensure all outputs are processed
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                    Ok(follow_up.content)
                }
            }
            Err(e) => {
                error!("Failed to get follow-up response: {}", e);

                // Create a fallback response
                let fallback_message =
                    "The command was executed, but there was an error getting a detailed response.";
                self.context.add_assistant_message(fallback_message);
                Ok(fallback_message.to_string())
            }
        }
    }

    async fn collect_streaming_follow_up(
        &self,
        follow_up_stream: &mut Box<dyn Stream<Item = Result<StreamChunk>> + Unpin + Send>,
    ) -> Result<FollowUpResponse> {
        println!(); // Start with a clean line
        let mut follow_up_content = String::new();

        while let Some(follow_up_chunk_result) = follow_up_stream.next().await {
            match follow_up_chunk_result {
                Ok(follow_up_chunk) => {
                    if !follow_up_chunk.content.is_empty() {
                        // Format the content to extract JSON-RPC result if present
                        let formatted_content = format_llm_response(&follow_up_chunk.content);
                        print!("{}", formatted_content);
                        let _ = std::io::stdout().flush();
                        follow_up_content.push_str(&follow_up_chunk.content);
                    }

                    if follow_up_chunk.is_complete {
                        debug!("Follow-up response complete");
                        println!();
                        break;
                    }
                }
                Err(e) => {
                    error!("Error in follow-up stream: {}", e);
                    break;
                }
            }
        }

        // Check if the follow-up is empty or contains a tool call
        let is_empty_or_tool_call =
            follow_up_content.trim().is_empty() || follow_up_content.contains("mcp.tool_call");

        Ok(FollowUpResponse {
            content: follow_up_content,
            is_empty_or_tool_call,
        })
    }

    async fn handle_problematic_streaming_follow_up(&mut self) -> Result<String> {
        debug!("FOLLOW-UP RESPONSE WAS EMPTY OR CONTAINS ANOTHER TOOL CALL! Retrying...");

        // Extract the tool result for the fallback message
        let tool_result_output = self.extract_last_tool_result();

        // Add explicit instruction to help Claude understand
        let instruction = self.create_tool_result_instruction(&tool_result_output);
        self.context.add_assistant_message(&instruction);

        // Try one more time with an explicit request for a response
        debug!("Getting follow-up response with explicit instruction...");

        // Sleep a bit before retry
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let retry_client = self.llm_client.as_ref().unwrap();
        let retry_result = retry_client.stream_message(&self.context).await;

        match retry_result {
            Ok(mut retry_stream) => {
                let retry_response = self.collect_streaming_retry(&mut retry_stream).await?;

                // Check if the retry was successful
                if !retry_response.is_empty() && !retry_response.contains("mcp.tool_call") {
                    debug!("RETRY FOLLOW-UP RESPONSE RECEIVED: {}", retry_response);
                    self.context.add_assistant_message(&retry_response);
                    Ok(retry_response)
                } else {
                    // Retry still problematic, use fallback
                    debug!("RETRY STILL PROBLEMATIC! Using fallback response.");
                    let fallback_message = self.create_fallback_message(&tool_result_output);

                    println!("Response: {}", fallback_message);
                    self.context.add_assistant_message(&fallback_message);
                    Ok(fallback_message)
                }
            }
            Err(e) => {
                error!("Error in retry follow-up stream: {}", e);
                // Create a fallback response
                let fallback_message =
                    "The command was executed, but there was an error getting a detailed response.";
                println!("Response: {}", fallback_message);
                self.context.add_assistant_message(fallback_message);
                Ok(fallback_message.to_string())
            }
        }
    }

    async fn collect_streaming_retry(
        &self,
        retry_stream: &mut Box<dyn Stream<Item = Result<StreamChunk>> + Unpin + Send>,
    ) -> Result<String> {
        println!(); // Start with a clean line
        let mut retry_content = String::new();

        while let Some(retry_chunk_result) = retry_stream.next().await {
            if let Ok(retry_chunk) = retry_chunk_result {
                if !retry_chunk.content.is_empty() {
                    // Format the content to extract JSON-RPC result if present
                    let formatted_content = format_llm_response(&retry_chunk.content);
                    print!("{}", formatted_content);
                    let _ = std::io::stdout().flush();
                    retry_content.push_str(&retry_chunk.content);
                }

                if retry_chunk.is_complete {
                    println!();
                    break;
                }
            }
        }

        Ok(retry_content)
    }

    // ========== Non-streaming response handling ==========

    async fn handle_non_streaming_response(&mut self) -> Result<String> {
        debug_log("Using standard (non-streaming) response");
        let client = self.llm_client.as_ref().unwrap();

        // Record the time taken for the regular response
        let response = time!("llm.response_time", {
            debug_log("Sending request to Bedrock");
            match client.send_message(&self.context).await {
                Ok(resp) => {
                    debug_log("Response received from Bedrock");
                    api_log(&format!("Response content: {}", resp.content));
                    resp
                }
                Err(e) => {
                    debug_log(&format!("Error from Bedrock: {}", e));
                    count!("llm.errors", 1);

                    // Print a user-friendly error message
                    eprintln!("Error communicating with Bedrock: {}", e);
                    eprintln!("Please check your AWS credentials and model availability.");

                    return Err(anyhow!("Error from Bedrock: {}", e));
                }
            }
        });

        // Add the response to the conversation
        debug_log("Adding assistant message to conversation");
        self.context.add_assistant_message(&response.content);

        // Handle tool calls if any
        if !response.tool_calls.is_empty() {
            // Process and handle tool calls
            self.handle_non_streaming_tool_calls(&response.tool_calls)
                .await?;

            // Get a follow-up response after tool execution
            return self.get_non_streaming_follow_up().await;
        }

        // No tool calls, just return the formatted response
        trace!("Raw response content: {}", response.content);
        let formatted_response = format_llm_response(&response.content);
        println!("{}", formatted_response); // Print formatted response (uncommented)
        debug_log("Request completed successfully");
        Ok(response.content)
    }

    async fn handle_non_streaming_tool_calls(
        &mut self,
        tool_calls: &[mcp_llm::ToolCall],
    ) -> Result<()> {
        debug!("Found {} tool calls", tool_calls.len());
        count!("llm.tool_calls", tool_calls.len() as u64);

        // Process each tool call
        for tool_call in tool_calls {
            debug!("Tool call: {}", tool_call.tool);
            let metric_name = &format!("llm.tool_calls.{}", tool_call.tool);
            count!(metric_name, 1);

            // Process the tool call
            let tool_name = &tool_call.tool;
            let params = &tool_call.params;

            debug!("Processing tool call: {}", tool_name);

            // Execute the tool
            self.handle_tool_call_execution(tool_name, params.clone())
                .await?;
        }

        Ok(())
    }

    async fn get_non_streaming_follow_up(&mut self) -> Result<String> {
        debug!("Getting follow-up response with tool results...");

        // Get a fresh reference to the LLM client
        let client = self.llm_client.as_ref().unwrap();
        let follow_up_response = client.send_message(&self.context).await?;

        // Check if the follow-up content is empty or contains another tool call
        if follow_up_response.content.trim().is_empty()
            || follow_up_response.content.contains("mcp.tool_call")
        {
            debug!("FOLLOW-UP RESPONSE WAS EMPTY OR CONTAINS ANOTHER TOOL CALL! Retrying...");

            // Handle problematic follow-up response
            self.handle_problematic_non_streaming_follow_up().await
        } else {
            // We have a valid non-empty follow-up response
            self.handle_successful_non_streaming_follow_up(&follow_up_response.content)
                .await
        }
    }

    async fn handle_problematic_non_streaming_follow_up(&mut self) -> Result<String> {
        // Extract the tool result for the fallback message
        let tool_result_output = self.extract_last_tool_result();

        // Add explicit instruction to help Claude understand
        let instruction = self.create_tool_result_instruction(&tool_result_output);
        self.context.add_assistant_message(&instruction);

        // Try one more time with an explicit request for a response
        debug!("Getting follow-up response with explicit instruction...");

        // Sleep a bit before retry
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let retry_client = self.llm_client.as_ref().unwrap();
        let retry_response = retry_client.send_message(&self.context).await?;

        // Check if the retry was successful
        if !retry_response.content.trim().is_empty()
            && !retry_response.content.contains("mcp.tool_call")
        {
            debug!(
                "RETRY FOLLOW-UP RESPONSE RECEIVED: {}",
                retry_response.content
            );

            self.context.add_assistant_message(&retry_response.content);

            // Format and print the retry response
            //let formatted_response = format_llm_response(&retry_response.content);
            //println!("{}", formatted_response);
            debug!("Tool call flow completed successfully with retry");
            Ok(retry_response.content)
        } else {
            warn!("Retry still problematic. Using fallback response.");

            // Create a fallback response based on the tool result
            let fallback_message = self.create_fallback_message(&tool_result_output);

            self.context.add_assistant_message(&fallback_message);

            // Format and print the fallback message
            //println!("{}", fallback_message);
            debug!("Tool call flow completed with intelligent fallback message");
            Ok(fallback_message)
        }
    }

    async fn handle_successful_non_streaming_follow_up(&mut self, content: &str) -> Result<String> {
        // Add the valid follow-up response to the context
        self.context.add_assistant_message(content);

        // Log the response details
        debug!(
            "Received valid follow-up response after tool execution: length={} chars, content: {}",
            content.len(),
            content
        );
        debug!(
            "Received follow-up response after tool execution: length={} chars",
            content.len()
        );

        // Sleep for a longer time to ensure all outputs are properly processed
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Format and print the follow-up response
        //let formatted_response = format_llm_response(content);
        //println!("{}", formatted_response);
        debug!("Tool call flow completed successfully");
        Ok(content.to_string())
    }

    // ========== Tool execution helpers ==========

    async fn handle_tool_call(&mut self, tool_call: &mcp_llm::ToolCall) -> Result<()> {
        debug!("Tool call received: {:?}", tool_call);
        count!("llm.tool_calls", 1);

        // Extract tool name and parameters
        let tool_name = &tool_call.tool;
        let params = &tool_call.params;

        info!("Processing tool call: {}", tool_name);

        // Execute the tool
        self.handle_tool_call_execution(tool_name, params.clone())
            .await
    }

    async fn handle_tool_call_execution(&mut self, tool_name: &str, params: Value) -> Result<()> {
        // Execute the tool
        let tool_result = self.execute_tool(tool_name, params).await;

        match tool_result {
            Ok(result) => {
                // Process successful tool execution
                let formatted = self.format_tool_result(result);

                // Just show a brief status indicator instead of the full result
                println!(
                    "\nProcessing {} command...",
                    formatted.original_result.tool_id
                );

                // Add the tool result to the context
                debug!(
                    "Adding tool result to context: {}",
                    formatted.formatted_result
                );

                // Log tool result at trace level for detailed debugging
                trace!(
                    ">>> TOOL RESULT JSON-RPC TO LLM >>>\n{}",
                    serde_json::to_string_pretty(&formatted.original_result).unwrap_or_default()
                );

                // Add the tool result to the conversation context
                self.context.add_tool_message(&formatted.formatted_result);

                Ok(())
            }
            Err(e) => {
                // Handle tool execution error
                error!("Error executing tool: {}", e);

                // Just show a brief error indicator
                println!("\nError processing {} command", tool_name);

                // Format as a standard MCP error response using JSON-RPC 2.0
                let error_result = format!(
                    "{{\"jsonrpc\": \"2.0\", \"error\": {{\"code\": -32000, \"message\": \"Failed to execute tool: {}\"}}, \"id\": \"tool_result\"}}",
                    e
                );
                debug!("Adding tool error to context: {}", error_result);
                self.context.add_tool_message(&error_result);

                Ok(())
            }
        }
    }

    fn format_tool_result(&self, result: ToolResult) -> FormattedToolResult {
        // Format the result as JSON-RPC 2.0
        let output_json = serde_json::to_string(&result.output)
            .unwrap_or_else(|_| "\"Failed to serialize result\"".to_string());

        // Format as a standard MCP tool response
        let formatted_result = format!(
            "{{\"jsonrpc\": \"2.0\", \"result\": {}, \"id\": \"tool_result\"}}",
            output_json
        );

        FormattedToolResult {
            formatted_result,
            original_result: result,
        }
    }

    // ========== Utility functions ==========

    fn extract_last_tool_result(&self) -> String {
        match self.context.messages.last() {
            Some(message) if matches!(message.role, MessageRole::Tool) => message.content.clone(),
            _ => "\"Tool result not found\"".to_string(),
        }
    }

    fn create_tool_result_instruction(&self, tool_result: &str) -> String {
        format!(
            "I've executed the requested tool and received the following result: {}. \
            Now I need to provide a direct human-readable answer based on this result. \
            The tool has already been executed successfully. \
            I will not make another tool call. \
            Instead, I'll synthesize a concise answer for the user based on the tool result above.",
            tool_result
        )
    }

    fn create_fallback_message(&self, tool_result_output: &str) -> String {
        // Try to use our fancy formatter to parse the JSON-RPC result
        if let Some(formatted_output) = ResponseFormatter::extract_from_jsonrpc(tool_result_output)
        {
            return formatted_output;
        }

        // Fallback to basic formatting if our fancy formatter fails
        let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(tool_result_output);

        match parsed_result {
            Ok(json) => {
                if let Some(result) = json.get("result") {
                    if let Some(stdout) = result.get("stdout") {
                        if let Some(stdout_str) = stdout.as_str() {
                            format!(
                                "Command executed successfully. Result: {}",
                                stdout_str.trim()
                            )
                        } else {
                            "Command executed successfully.".to_string()
                        }
                    } else {
                        format!("Command executed. Result: {}", result)
                    }
                } else if let Some(err) = json.get("error") {
                    format!("Command execution error: {}", err)
                } else {
                    format!("Tool executed with result: {}", json)
                }
            }
            Err(_) => "Tool executed successfully.".to_string(),
        }
    }
}

impl Default for CliApp {
    fn default() -> Self {
        Self::new()
    }
}

// Debug helpers
impl CliApp {
    // Get the current conversation context size
    pub fn debug_context_size(&self) -> usize {
        self.context.messages.len()
    }

    // Get the roles of the last n messages
    pub fn debug_last_message_roles(&self, n: usize) -> String {
        let mut roles = Vec::new();

        let start = if self.context.messages.len() > n {
            self.context.messages.len() - n
        } else {
            0
        };

        for i in start..self.context.messages.len() {
            match self.context.messages[i].role {
                MessageRole::User => roles.push("User"),
                MessageRole::Assistant => roles.push("Assistant"),
                MessageRole::Tool => roles.push("Tool"),
                MessageRole::System => roles.push("System"),
            }
        }

        // Return comma-separated roles
        roles.join(", ")
    }
}
