use mcp_core::context::ConversationContext;
use std::sync::{Arc, RwLock};

// Session manages the state of a conversation
pub struct Session {
    context: Arc<RwLock<ConversationContext>>,
    // Additional fields will be added as needed
}

impl Session {
    pub fn new() -> Self {
        Self {
            context: Arc::new(RwLock::new(ConversationContext::new())),
        }
    }
    
    pub fn get_context(&self) -> Arc<RwLock<ConversationContext>> {
        self.context.clone()
    }
    
    pub fn add_user_message(&self, content: &str) {
        if let Ok(mut context) = self.context.write() {
            context.add_user_message(content);
        }
    }
    
    pub fn reset(&self) {
        if let Ok(mut context) = self.context.write() {
            *context = ConversationContext::new();
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_creation() {
        let session = Session::new();
        session.add_user_message("Hello");
        
        let context = session.get_context();
        let context_read = context.read().unwrap();
        assert_eq!(context_read.messages.len(), 1);
    }
}