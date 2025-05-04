#[cfg(test)]
mod tests {
    use mcp_core::context::{ConversationContext, MessageRole};

    #[test]
    fn test_add_user_message() {
        let mut context = ConversationContext::new();
        context.add_user_message("Hello, world!");

        assert_eq!(context.messages.len(), 1);
        assert!(matches!(context.messages[0].role, MessageRole::User));
        assert_eq!(context.messages[0].content, "Hello, world!");
    }
}
