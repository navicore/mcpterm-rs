# MCP LLM Clients

This crate provides LLM provider adapters for the Model Context Protocol (MCP), including clients for different LLM services.

## Modules

- `client-trait`: Common interface for LLM clients
- `anthropic`: Claude implementation
- `bedrock`: AWS Bedrock implementation
- `streaming`: Streaming response handling

## Features

- Unified client interface
- Streaming response processing
- Context management
- Tool invocation parsing
- Error handling
- Configurable request parameters

## Usage

```rust
use mcp_core::context::ConversationContext;
use mcp_llm::LlmClient;
use mcp_llm::bedrock::BedrockClient;

// Create a client
let config = BedrockConfig::new("anthropic.claude-3-sonnet-20240229-v1:0");
let client = BedrockClient::new(config).await?;

// Process a message
let context = ConversationContext::new();
context.add_user_message("Hello, world!");

let response = client.send_message(&context).await?;
println!("Response: {}", response.content);
```