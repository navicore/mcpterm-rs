use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    User,
    Assistant,
    System,
}

#[allow(dead_code)]
//#[derive(Debug, Clone)]
pub struct Message {
    pub content: String,
    pub message_type: MessageType,
    pub timestamp: u64,
}

#[allow(dead_code)]
impl Message {
    pub fn new(content: String, message_type: MessageType) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            content,
            message_type,
            timestamp,
        }
    }
}
