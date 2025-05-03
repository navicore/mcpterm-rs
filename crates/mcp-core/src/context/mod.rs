use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_results: Option<Vec<ToolResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_id: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_id: String,
    pub result: Value,
}

#[derive(Debug, Clone)]
pub struct ConversationContext {
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub current_request_id: Option<String>,
}

impl ConversationContext {
    pub fn new() -> Self {
        Self {
            system_prompt: String::new(),
            messages: Vec::new(),
            current_request_id: None,
        }
    }

    // Example method - will be fully implemented in future
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::User,
            content: content.to_string(),
            tool_calls: None,
            tool_results: None,
        });
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new()
    }
}