// MCP Protocol module
// Handles JSON-RPC 2.0 message formats, serialization, and validation

pub mod error;
pub mod message;
#[cfg(test)]
mod tests;
pub mod validation;

// Re-export key types
pub use error::{Error, ErrorCode};
pub use message::{Id, Request, Response, Version};
