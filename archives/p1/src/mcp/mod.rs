// MCP (Model Context Protocol) module
// Implements the JSON-RPC based protocol for AI model interactions

pub mod handler;
pub mod logging;
pub mod protocol;
pub mod resources;
pub mod setup;
pub mod tools;
// Will expand with these modules later:
// pub mod security;
// pub mod transport;

// Re-export key types
pub use handler::McpHandler;
pub use logging::{debug_log, get_log_path, init_debug_log, set_verbose_logging, ui_log};
pub use protocol::error::{Error, ErrorCode};
pub use protocol::message::{Id, Request, Response, Version};
pub use resources::{AccessMode, Resource, ResourceManager, ResourceMetadata, ResourceType};
pub use setup::init_mcp;
pub use tools::{Tool, ToolCategory, ToolManager, ToolMetadata, ToolResult, ToolStatus};
