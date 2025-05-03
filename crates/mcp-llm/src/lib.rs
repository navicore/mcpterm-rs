pub mod client_trait;
pub mod anthropic;
pub mod bedrock;
pub mod streaming;

pub use client_trait::{LlmClient, LlmResponse, StreamChunk};

// Re-export specific implementations
pub use anthropic::AnthropicClient;
pub use bedrock::BedrockClient;