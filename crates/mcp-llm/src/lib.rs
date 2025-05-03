pub mod client_trait;
pub mod anthropic;
pub mod bedrock;
pub mod streaming;
pub mod schema;

pub use client_trait::{LlmClient, LlmResponse, StreamChunk, ToolCall};
pub use schema::McpSchemaManager;

// Re-export specific implementations
pub use anthropic::AnthropicClient;
pub use bedrock::{BedrockClient, BedrockConfig, BedrockError};