use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::{Client, Error};
use serde_json::{json, Value};
use std::env;
use tokio::runtime::Runtime;

use super::Agent;

pub struct BedrockAgent {
    model_id: String,
    runtime: Runtime,
}

impl BedrockAgent {
    pub fn new() -> Self {
        // Get model ID from environment or use default
        let model_id = env::var("BEDROCK_MODEL_ID")
            //.unwrap_or_else(|_| "anthropic.claude-3-sonnet-20240229-v1:0".to_string());
            .unwrap_or_else(|_| "us.anthropic.claude-3-5-haiku-20241022-v1:0".to_string());
        // Create a dedicated runtime for this agent to avoid nesting issues
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");

        Self { model_id, runtime }
    }
}

impl Agent for BedrockAgent {
    fn process_message(&self, input: &str) -> String {
        // Use our dedicated runtime to run the async code
        match self.runtime.block_on(async {
            // Create a fresh client for each request to avoid any state issues
            let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
            let client = Client::new(&config);

            // Call the invoke_model function with the newly created client
            self.invoke_model(&client, input).await
        }) {
            Ok(response) => response,
            Err(e) => format!("Error invoking Bedrock: {}", e),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(BedrockAgent {
            model_id: self.model_id.clone(),
            runtime: Runtime::new().expect("Failed to create Tokio runtime"),
        })
    }
}

impl BedrockAgent {
    async fn invoke_model(&self, client: &Client, input: &str) -> Result<String, Error> {
        // For Claude models, we need to create a specific request body
        let request_body = match self.model_id.starts_with("anthropic.claude") {
            true => {
                // Claude 3 format
                json!({
                    "anthropic_version": "bedrock-2023-05-31",
                    "max_tokens": 1000,
                    "temperature": 0.7,
                    "system": "You are MCP, a helpful terminal-based assistant. Be concise in your responses.",
                    "messages": [
                        {
                            "role": "user",
                            "content": input
                        }
                    ]
                })
            }
            false => {
                // Generic format for other models
                json!({
                    "prompt": input,
                    "max_tokens": 500,
                    "temperature": 0.7
                })
            }
        };

        // Convert to bytes and into a Blob
        let body_bytes = request_body.to_string().into_bytes();

        // Create and send the request
        let response = client
            .invoke_model()
            .model_id(&self.model_id)
            .body(body_bytes.into()) // Convert Vec<u8> into Blob
            .send()
            .await?;

        // Process the response - if we get a response body, try to parse it
        let response_body = response.body();

        // Convert the body to a string
        let response_str = String::from_utf8_lossy(response_body.as_ref());

        // If the response is empty, return an error message
        if response_str.is_empty() {
            return Ok("Empty response from model".to_string());
        }

        // Try to parse the JSON response
        match serde_json::from_str::<Value>(&response_str) {
            Ok(response_json) => {
                // Extract response based on model type
                let extracted_text = if self.model_id.starts_with("anthropic.claude") {
                    // Claude response format
                    response_json["content"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|obj| obj["text"].as_str())
                        .unwrap_or("No text in response")
                } else {
                    // Generic response format, adjust as needed
                    response_json["completion"].as_str().unwrap_or(
                        response_json["generated_text"]
                            .as_str()
                            .unwrap_or("Could not extract response text"),
                    )
                };

                Ok(extracted_text.to_string())
            }
            Err(e) => {
                // If we can't parse the JSON, return the error and the raw response
                Ok(format!(
                    "Error parsing response: {}\nRaw response: {}",
                    e, response_str
                ))
            }
        }
    }
}

impl Default for BedrockAgent {
    fn default() -> Self {
        Self::new()
    }
}
