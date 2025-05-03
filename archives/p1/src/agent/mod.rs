mod bedrock;
mod bedrock_sync;
mod mcp_agent;
mod stub;

pub use bedrock_sync::BedrockAgentSync;
pub use mcp_agent::McpAgent;
pub use stub::StubAgent;

use std::any::Any;

pub trait Agent: Send + Sync + 'static {
    fn process_message(&self, input: &str) -> String;

    /// Returns self as Any for downcasting to specific agent types
    fn as_any(&self) -> &dyn Any;

    /// Clone implementation for Agent trait objects
    fn clone_box(&self) -> Box<dyn Agent>;
}

impl Clone for Box<dyn Agent> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
