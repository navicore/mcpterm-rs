use anyhow::Result;
use mcp_core::context::ConversationContext;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Trait that all event types should implement
pub trait EventType: Send + std::fmt::Debug {}

/// Event handler trait for async event processing
pub trait EventHandlerTrait<T>: Send + Sync {
    fn handle(&self, event: T) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
    fn clone_box(&self) -> Box<dyn EventHandlerTrait<T>>;
}

// Make EventHandlerTrait objects cloneable
impl<T> Clone for Box<dyn EventHandlerTrait<T>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// Define a struct that wraps a closure
pub struct FnEventHandler<T, F>
where
    F: Fn(T) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + Clone + 'static,
    T: Clone + Send + Sync + 'static,
{
    f: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> FnEventHandler<T, F>
where
    F: Fn(T) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + Clone + 'static,
    T: Clone + Send + Sync + 'static,
{
    pub fn new(f: F) -> Self {
        Self {
            f,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F> EventHandlerTrait<T> for FnEventHandler<T, F>
where
    F: Fn(T) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + Clone + 'static,
    T: Clone + Send + Sync + 'static,
{
    fn handle(&self, event: T) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        (self.f)(event)
    }

    fn clone_box(&self) -> Box<dyn EventHandlerTrait<T>> {
        Box::new(Self {
            f: self.f.clone(),
            _phantom: std::marker::PhantomData,
        })
    }
}

// Type alias for easier use
pub type EventHandler<T> = Box<dyn EventHandlerTrait<T>>;

// Helper function to create an event handler from a closure
pub fn create_handler<T, F>(f: F) -> EventHandler<T>
where
    F: Fn(T) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + Clone + 'static,
    T: Clone + Send + Sync + 'static,
{
    Box::new(FnEventHandler::new(f))
}

/// UI events originate from user interactions with the terminal interface
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// User pressed a key
    KeyPress(KeyEvent),

    /// User submitted input
    UserInput(String),

    /// User requested cancellation of current operation
    RequestCancellation,

    /// User wants to quit the application
    Quit,

    /// User wants to scroll the message view
    Scroll(ScrollDirection, u16),

    /// User wants to clear the conversation
    ClearConversation,

    /// User toggles focus between components
    ToggleFocus,
}

/// Model events are related to the conversation model and context
#[derive(Debug, Clone)]
pub enum ModelEvent {
    /// Process a new user message
    ProcessUserMessage(String),

    /// A tool has returned a result
    ToolResult(String, Value),

    /// Reset the conversation context
    ResetContext,

    /// New message received from LLM
    LlmMessage(String),

    /// Stream chunk received from LLM
    LlmStreamChunk(String),

    /// Request to update the conversation context
    UpdateContext(Arc<ConversationContext>),

    /// LLM has requested a tool execution
    ToolRequest(String, Value),

    /// LLM response completed
    LlmResponseComplete,
}

/// API events are related to external API calls and responses
#[derive(Debug, Clone)]
pub enum ApiEvent {
    /// Send a request to the LLM
    SendRequest(String),

    /// Process a stream from the LLM
    ProcessStream(String),

    /// Cancel an ongoing request
    CancelRequest(String),

    /// API connection established
    ConnectionEstablished,

    /// API connection lost
    ConnectionLost(String),

    /// API error occurred
    Error(String),
}

/// Direction for scrolling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
}

/// Simple struct to represent a key event
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// Simplified key code enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    F(u8),
}

/// Simplified key modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

// Implement EventType for all our event enums
impl EventType for UiEvent {}
impl EventType for ModelEvent {}
impl EventType for ApiEvent {}

// Default implementations for KeyModifiers

// Helper methods for KeyCode
impl KeyCode {
    pub fn is_enter(&self) -> bool {
        matches!(self, KeyCode::Enter)
    }

    pub fn is_esc(&self) -> bool {
        matches!(self, KeyCode::Esc)
    }

    pub fn is_backspace(&self) -> bool {
        matches!(self, KeyCode::Backspace)
    }

    pub fn is_tab(&self) -> bool {
        matches!(self, KeyCode::Tab)
    }
}
