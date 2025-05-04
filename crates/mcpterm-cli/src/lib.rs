use anyhow::{anyhow, Result};
use futures::StreamExt;
use mcp_core::context::ConversationContext;
use mcp_core::{api_log, debug_log};
use mcp_llm::{BedrockClient, BedrockConfig, LlmClient};
use mcp_metrics::{count, gauge, time};
use std::io::Write;
use std::sync::Arc;

// Export our mock implementation for tests
pub mod mock;

pub struct CliApp {
    context: ConversationContext,
    llm_client: Option<Arc<dyn LlmClient + Send + Sync>>,
    config: CliConfig,
}

#[derive(Debug)]
pub struct CliConfig {
    pub model: String,
    pub use_mcp: bool,
    pub region: Option<String>,
    pub streaming: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            model: "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
            use_mcp: false,
            region: None,
            streaming: true,
        }
    }
}

impl CliApp {
    pub fn new() -> Self {
        Self {
            context: ConversationContext::new(),
            llm_client: None,
            config: CliConfig::default(),
        }
    }

    pub fn with_config(mut self, config: CliConfig) -> Self {
        self.config = config;
        self
    }

    // Add a method to set a custom LLM client (useful for testing)
    pub fn with_llm_client<T>(mut self, client: T) -> Self
    where
        T: LlmClient + Send + Sync + 'static,
    {
        self.llm_client = Some(Arc::new(client));
        self
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

        // Create the Bedrock client - it will initialize AWS SDK config internally
        debug_log("Creating BedrockClient");
        let client = match BedrockClient::new(bedrock_config).await {
            Ok(client) => {
                debug_log("Successfully created BedrockClient");
                client
            }
            Err(e) => {
                debug_log(&format!("Failed to create BedrockClient: {}", e));

                // Print helpful error message about credentials
                eprintln!("Failed to initialize AWS Bedrock client: {}", e);
                eprintln!("Please ensure you have valid AWS credentials configured.");
                eprintln!("You can set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY environment variables,");
                eprintln!("or configure credentials in ~/.aws/credentials file.");
                eprintln!(
                    "Also verify that the model ID '{}' is available in your AWS region.",
                    self.config.model
                );

                return Err(e);
            }
        };

        self.llm_client = Some(Arc::new(client));
        Ok(())
    }

    pub async fn run(&mut self, prompt: &str) -> Result<String> {
        // Make sure the client is initialized
        if self.llm_client.is_none() {
            debug_log("Client not initialized, initializing now");
            self.initialize().await?;
        }

        let client = self.llm_client.as_ref().unwrap();

        // Add the user message to the conversation context
        debug_log(&format!("Adding user message: {}", prompt));
        self.context.add_user_message(prompt);

        // Record metrics
        count!("llm.requests.total");
        count!("llm.requests.bedrock");

        // Use streaming or regular response based on config
        if self.config.streaming {
            debug_log("Using streaming response");
            // Process a streaming response
            let mut response_content = String::new();

            // Record the time taken for the streaming response
            time!("llm.streaming_response_time", {
                debug_log("Sending streaming request to Bedrock");
                let stream_result = client.stream_message(&self.context).await;

                match stream_result {
                    Ok(mut stream) => {
                        debug_log("Stream response received, processing chunks");
                        // Process the stream chunks
                        println!("Response: ");

                        // Track if we've received any content at all
                        let mut received_content = false;

                        while let Some(chunk_result) = stream.next().await {
                            match chunk_result {
                                Ok(chunk) => {
                                    debug_log(&format!(
                                        "Received chunk, {} bytes",
                                        chunk.content.len()
                                    ));
                                    api_log(&format!("Chunk content: {}", chunk.content));

                                    if !chunk.content.is_empty() {
                                        received_content = true;

                                        // Print the chunk content and add to total content
                                        print!("{}", chunk.content);
                                        let _ = std::io::stdout().flush();

                                        response_content.push_str(&chunk.content);

                                        // Record metrics
                                        count!("llm.stream_chunks", 1);
                                    }

                                    // If this is a tool call, handle it (not implemented yet)
                                    if chunk.is_tool_call {
                                        debug_log(&format!(
                                            "Tool call received: {:?}",
                                            chunk.tool_call
                                        ));
                                        count!("llm.tool_calls", 1);
                                    }

                                    // If this is the final chunk, we're done
                                    if chunk.is_complete {
                                        debug_log("Final chunk received");
                                        println!(); // Add a newline after completion
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
        } else {
            debug_log("Using standard (non-streaming) response");
            // Process a regular response
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

            // Record metrics for tool calls if any
            if !response.tool_calls.is_empty() {
                debug_log(&format!("Found {} tool calls", response.tool_calls.len()));
                count!("llm.tool_calls", response.tool_calls.len() as u64);
                for tool_call in &response.tool_calls {
                    debug_log(&format!("Tool call: {}", tool_call.tool));
                    let metric_name = &format!("llm.tool_calls.{}", tool_call.tool);
                    count!(metric_name, 1);
                }
            }

            // Print and return the response
            println!("Response: {}", response.content);
            debug_log("Request completed successfully");
            Ok(response.content)
        }
    }
}

impl Default for CliApp {
    fn default() -> Self {
        Self::new()
    }
}
