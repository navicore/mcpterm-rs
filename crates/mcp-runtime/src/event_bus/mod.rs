use crossbeam_channel::{Receiver, Sender};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone)]
pub enum UiEvent {
    KeyPress(KeyEvent),
    UserInput(String),
    RequestCancellation,
    Quit,
}

#[derive(Debug, Clone)]
pub enum ModelEvent {
    ProcessUserMessage(String),
    ToolResult(String, Value),
    ResetContext,
    // Add more events as needed
}

#[derive(Debug, Clone)]
pub enum ApiEvent {
    SendRequest(String),
    ProcessStream(String),
    CancelRequest,
    // Add more events as needed
}

// Simple struct to represent a key event
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

// Simplified key code enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Backspace,
    // Add more key codes as needed
}

// Simplified key modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl fmt::Display for UiEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UiEvent::KeyPress(_) => write!(f, "KeyPress"),
            UiEvent::UserInput(input) => write!(f, "UserInput({})", input),
            UiEvent::RequestCancellation => write!(f, "RequestCancellation"),
            UiEvent::Quit => write!(f, "Quit"),
        }
    }
}

// EventBus manages communication channels between components
pub struct EventBus;

impl EventBus {
    pub fn new_ui_channel() -> (Sender<UiEvent>, Receiver<UiEvent>) {
        crossbeam_channel::unbounded()
    }

    pub fn new_model_channel() -> (Sender<ModelEvent>, Receiver<ModelEvent>) {
        crossbeam_channel::unbounded()
    }

    pub fn new_api_channel() -> (Sender<ApiEvent>, Receiver<ApiEvent>) {
        crossbeam_channel::unbounded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_event_channel_creation() {
        let (tx, rx) = EventBus::new_ui_channel();
        
        // Test sending an event
        tx.send(UiEvent::Quit).unwrap();
        
        // Test receiving the event
        let event = rx.recv().unwrap();
        assert!(matches!(event, UiEvent::Quit));
    }
}