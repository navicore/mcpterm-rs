use anyhow::Result;
use mcp_core::context::ConversationContext;

pub struct CliApp {
    context: ConversationContext,
    // Additional fields will be added as needed
}

impl CliApp {
    pub fn new() -> Self {
        Self {
            context: ConversationContext::new(),
        }
    }
    
    pub async fn run(&mut self, prompt: &str) -> Result<String> {
        // Placeholder implementation
        self.context.add_user_message(prompt);
        Ok(format!("Response to: {}", prompt))
    }
}

impl Default for CliApp {
    fn default() -> Self {
        Self::new()
    }
}