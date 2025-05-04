use serde::{Deserialize, Serialize};
use serde_json::Value;
use mcp_metrics::count;
use tracing::{debug, trace};

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
        debug!("Creating new conversation context");
        Self {
            system_prompt: String::new(),
            messages: Vec::new(),
            current_request_id: None,
        }
    }

    // Helper methods to add messages of different types
    pub fn add_user_message(&mut self, content: &str) {
        debug!("Adding user message to conversation");
        trace!("User message content: {}", content);
        self.messages.push(Message {
            role: MessageRole::User,
            content: content.to_string(),
            tool_calls: None,
            tool_results: None,
        });
        
        // Count message metrics
        count!("conversation.messages.total");
        count!("conversation.messages.user");
        
        debug!("Conversation now has {} messages", self.messages.len());
    }

    pub fn add_assistant_message(&mut self, content: &str) {
        debug!("Adding assistant message to conversation");
        trace!("Assistant message content: {}", content);
        self.messages.push(Message {
            role: MessageRole::Assistant,
            content: content.to_string(),
            tool_calls: None,
            tool_results: None,
        });
        
        // Count message metrics
        count!("conversation.messages.total");
        count!("conversation.messages.assistant");
        
        debug!("Conversation now has {} messages", self.messages.len());
    }

    pub fn add_tool_message(&mut self, content: &str) {
        debug!("Adding tool message to conversation");
        trace!("Tool message content: {}", content);
        self.messages.push(Message {
            role: MessageRole::Tool,
            content: content.to_string(),
            tool_calls: None,
            tool_results: None,
        });
        
        // Count message metrics
        count!("conversation.messages.total");
        count!("conversation.messages.tool");
        
        debug!("Conversation now has {} messages", self.messages.len());
    }

    pub fn add_system_message(&mut self, content: &str) {
        debug!("Adding system message to conversation");
        trace!("System message content: {}", content);
        self.messages.push(Message {
            role: MessageRole::System,
            content: content.to_string(),
            tool_calls: None,
            tool_results: None,
        });
        
        // Count message metrics
        count!("conversation.messages.total");
        count!("conversation.messages.system");
        
        debug!("Conversation now has {} messages", self.messages.len());
    }

    pub fn add_message(&mut self, role: MessageRole, content: &str) {
        debug!("Adding message with role {:?} to conversation", role);
        trace!("Message content: {}", content);
        self.messages.push(Message {
            role: role.clone(),
            content: content.to_string(),
            tool_calls: None,
            tool_results: None,
        });
        
        // Count message metrics
        count!("conversation.messages.total");
        match role {
            MessageRole::User => count!("conversation.messages.user"),
            MessageRole::Assistant => count!("conversation.messages.assistant"),
            MessageRole::Tool => count!("conversation.messages.tool"),
            MessageRole::System => count!("conversation.messages.system"),
        }
        
        debug!("Conversation now has {} messages", self.messages.len());
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new()
    }
}
