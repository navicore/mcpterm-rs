use mcp_core::context::ConversationContext;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Messages,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub content: String,
    pub message_type: MessageType,
}

pub struct AppState {
    pub context: Arc<RwLock<ConversationContext>>,
    pub messages: Vec<Message>,
    pub input_content: String,
    pub focus: FocusArea,
    pub running: bool,
    pub processing: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            context: Arc::new(RwLock::new(ConversationContext::new())),
            messages: Vec::new(),
            input_content: String::new(),
            focus: FocusArea::Input,
            running: true,
            processing: false,
        }
    }
    
    pub fn add_message(&mut self, content: String, message_type: MessageType) {
        let message = Message {
            content,
            message_type,
        };
        self.messages.push(message.clone());
        
        // Add to conversation context
        if let Ok(mut context) = self.context.write() {
            match message_type {
                MessageType::User => context.add_user_message(&message.content),
                // Other message types will be handled similarly
                _ => {}
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}