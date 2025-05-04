use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use mcp_core::context::ConversationContext;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub id: String,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub tool: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub content: String,
    pub is_tool_call: bool,
    pub tool_call: Option<ToolCall>,
    pub is_complete: bool,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn send_message(&self, context: &ConversationContext) -> Result<LlmResponse>;

    async fn stream_message(
        &self,
        context: &ConversationContext,
    ) -> Result<Box<dyn Stream<Item = Result<StreamChunk>> + Unpin + Send>>;

    fn cancel_request(&self, request_id: &str) -> Result<()>;
}
