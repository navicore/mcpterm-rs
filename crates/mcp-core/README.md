# MCP Core

This crate provides the core types and protocol definition for the Model Context Protocol (MCP).

## Modules

- `protocol`: Definitions for the JSON-RPC 2.0 based MCP protocol, including message types, validation, and error handling
- `context`: Conversation context management, tracking the state of interactions between the user, the LLM, and tools

## Usage

```rust
use mcp_core::protocol::{Request, Response, Error};
use mcp_core::context::ConversationContext;

// Example: Create a conversation context
let mut context = ConversationContext::new();
context.add_user_message("Hello, world!");
```

## References

- MCP Protocol Specification: https://modelcontextprotocol.io/llms-full.txt
- MCP Schema JSON: https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/refs/heads/main/schema/2025-03-26/schema.json