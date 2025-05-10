use chrono::{DateTime, Utc};
use crossterm::event::KeyEvent;
use mcp_core::context::{ConversationContext, Message as CoreMessage, MessageRole};
use mcp_llm::client_trait::LlmResponse;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// Areas of the UI that can have focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Messages,
    Input,
}

/// Mode for the input editor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Visual,
}

/// Types of messages in the conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    System,
    User,
    Assistant,
    Error,
    Tool,
}

impl From<MessageRole> for MessageType {
    fn from(role: MessageRole) -> Self {
        match role {
            MessageRole::System => MessageType::System,
            MessageRole::User => MessageType::User,
            MessageRole::Assistant => MessageType::Assistant,
            MessageRole::Tool => MessageType::Tool,
        }
    }
}

impl Into<MessageRole> for MessageType {
    fn into(self) -> MessageRole {
        match self {
            MessageType::System => MessageRole::System,
            MessageType::User => MessageRole::User,
            MessageType::Assistant => MessageRole::Assistant,
            MessageType::Tool | MessageType::Error => MessageRole::Tool,
        }
    }
}

/// A message in the conversation with metadata
#[derive(Debug, Clone)]
pub struct Message {
    pub content: String,
    pub message_type: MessageType,
    pub timestamp: DateTime<Utc>,
    pub id: String,
}

impl Message {
    pub fn new(content: String, message_type: MessageType) -> Self {
        Self {
            content,
            message_type,
            timestamp: Utc::now(),
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Convert to a core message for the conversation context
    pub fn to_core_message(&self) -> CoreMessage {
        CoreMessage {
            role: self.message_type.into(),
            content: self.content.clone(),
            tool_calls: None,
            tool_results: None,
        }
    }
}

/// Processing status of a user request
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingStatus {
    Idle,
    Connecting,
    Processing { start_time: Instant, status: String },
    Error(String),
}

impl ProcessingStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, ProcessingStatus::Connecting | ProcessingStatus::Processing { .. })
    }

    pub fn as_display_message(&self) -> Option<(String, MessageType)> {
        match self {
            ProcessingStatus::Idle => None,
            ProcessingStatus::Connecting => Some((
                "⏳ Connecting to AI service...".to_string(),
                MessageType::System,
            )),
            ProcessingStatus::Processing { start_time, status } => {
                let elapsed = start_time.elapsed();
                Some((
                    format!(
                        "⏳ {} ({:?} elapsed)",
                        status,
                        Duration::from_secs(elapsed.as_secs())
                    ),
                    MessageType::System,
                ))
            }
            ProcessingStatus::Error(msg) => Some((format!("❌ Error: {}", msg), MessageType::Error)),
        }
    }
}

/// Input history for the editor
#[derive(Debug, Clone)]
pub struct InputHistory {
    pub entries: VecDeque<String>,
    pub current_index: Option<usize>,
    pub current_input: Option<String>,
    pub max_entries: usize,
}

impl InputHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries + 1),
            current_index: None,
            current_input: None,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: String) {
        // Don't add empty entries or duplicates at the end
        if entry.trim().is_empty() || self.entries.back().map_or(false, |e| e == &entry) {
            return;
        }

        // Add the entry to the history
        self.entries.push_back(entry);

        // If we've exceeded the max size, remove the oldest entry
        if self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        // Reset the navigation
        self.current_index = None;
        self.current_input = None;
    }

    pub fn previous(&mut self, current: &str) -> Option<String> {
        // If we're not navigating yet, save the current input
        if self.current_index.is_none() {
            self.current_input = Some(current.to_string());
            self.current_index = Some(self.entries.len());
        }

        // Get the current index
        let idx = self.current_index?;

        // If we're at the beginning, stay there
        if idx == 0 {
            return Some(self.entries[0].clone());
        }

        // Move to the previous entry
        let new_idx = idx - 1;
        self.current_index = Some(new_idx);
        Some(self.entries[new_idx].clone())
    }

    pub fn next(&mut self) -> Option<String> {
        // If we're not navigating, do nothing
        let idx = self.current_index?;

        // If we're at the end, return to the original input
        if idx >= self.entries.len() - 1 {
            let result = self.current_input.clone();
            self.current_index = None;
            self.current_input = None;
            return result;
        }

        // Move to the next entry
        let new_idx = idx + 1;
        self.current_index = Some(new_idx);
        Some(self.entries[new_idx].clone())
    }

    pub fn reset(&mut self) {
        self.current_index = None;
        self.current_input = None;
    }
}

impl Default for InputHistory {
    fn default() -> Self {
        Self::new(50)
    }
}

/// Main application state
pub struct AppState {
    // Core conversation state
    pub context: Arc<RwLock<ConversationContext>>,
    pub messages: Vec<Message>,
    
    // UI state
    pub input_content: String,
    pub input_cursor: usize,
    pub input_history: InputHistory,
    pub focus: FocusArea,
    pub editor_mode: EditorMode,
    pub running: bool,
    pub processing: ProcessingStatus,
    
    // Scroll state
    pub messages_scroll: usize,
    pub auto_scroll: bool,  // Whether to automatically scroll to show new messages
    
    // Metrics tracking
    pub message_count: usize,
    pub request_count: usize,
    pub error_count: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            context: Arc::new(RwLock::new(ConversationContext::new())),
            messages: Vec::new(),
            input_content: String::new(),
            input_cursor: 0,
            input_history: InputHistory::default(),
            focus: FocusArea::Input,
            editor_mode: EditorMode::Normal,
            running: true,
            processing: ProcessingStatus::Idle,
            messages_scroll: 0,
            auto_scroll: true, // Enable auto-scroll by default
            message_count: 0,
            request_count: 0,
            error_count: 0,
        }
    }

    /// Add a welcome message with version info
    pub fn add_welcome_message(&mut self) {
        let welcome = format!(
            "Welcome to MCPTerm TUI v{}!\n\n\
            Press 'i' to enter insert mode and type your message.\n\
            Press <Esc> to return to normal mode.\n\
            Press <Enter> in normal mode to send your message.\n\
            Press 'q' in normal mode to quit.\n\
            Press <Tab> to switch focus between input and message areas.",
            env!("CARGO_PKG_VERSION")
        );
        
        self.add_message(welcome, MessageType::System);
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, content: String, message_type: MessageType) {
        // Create and add the message
        let message = Message::new(content, message_type);
        self.messages.push(message.clone());
        self.message_count += 1;

        // Add to conversation context if appropriate
        if let Ok(mut context) = self.context.write() {
            match message_type {
                MessageType::User => context.add_user_message(&message.content),
                MessageType::Assistant => context.add_assistant_message(&message.content),
                MessageType::System => context.add_system_message(&message.content),
                MessageType::Tool => {
                    // Tool messages require special handling
                    // We could parse as JSON tool result if needed
                    debug!("Adding tool message to context: {}", message.content);
                    context.add_tool_message(&message.content);
                }
                MessageType::Error => {
                    // Error messages are just for display, not for context
                    debug!("Error message (not added to context): {}", message.content);
                }
            }
        } else {
            error!("Failed to acquire write lock on conversation context");
        }

        // Reset scroll position if auto-scroll is enabled
        if self.auto_scroll {
            self.messages_scroll = 0;
        }
    }
    
    /// Toggle auto-scroll feature
    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
        // Reset scroll position when enabling auto-scroll
        if self.auto_scroll {
            self.messages_scroll = 0;
        }
    }

    /// Submit the current input as a user message
    pub fn submit_input(&mut self) -> Option<String> {
        let input = std::mem::take(&mut self.input_content);
        self.input_cursor = 0;
        
        // Don't process empty input
        if input.trim().is_empty() {
            return None;
        }
        
        // Add to history
        self.input_history.add(input.clone());
        
        // Add as user message
        self.add_message(input.clone(), MessageType::User);
        
        // Start processing
        self.processing = ProcessingStatus::Connecting;
        self.request_count += 1;
        
        Some(input)
    }

    /// Process an LLM response
    pub fn process_llm_response(&mut self, response: Result<LlmResponse, anyhow::Error>) {
        match response {
            Ok(llm_response) => {
                // Process the response content
                if !llm_response.content.is_empty() {
                    self.add_message(llm_response.content, MessageType::Assistant);
                }
                
                // Process any tool calls
                for tool_call in llm_response.tool_calls {
                    // In a real implementation, we would execute the tool
                    // For now, just log it
                    let tool_msg = format!(
                        "Tool call: {} with parameters: {:?}",
                        tool_call.tool,
                        tool_call.params
                    );
                    debug!("{}", tool_msg);
                    self.add_message(tool_msg, MessageType::Tool);
                }
                
                // Reset processing status
                self.processing = ProcessingStatus::Idle;
            }
            Err(e) => {
                // Handle the error
                let error_msg = format!("Error processing request: {}", e);
                error!("{}", error_msg);
                self.add_message(error_msg, MessageType::Error);
                self.processing = ProcessingStatus::Error(e.to_string());
                self.error_count += 1;
            }
        }
    }

    /// Handle special key events (navigation, history, etc.)
    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        
        match (self.focus, self.editor_mode, key.code) {
            // Quit
            (_, EditorMode::Normal, KeyCode::Char('q')) => {
                self.running = false;
                true
            }
            
            // Switch focus
            (_, _, KeyCode::Tab) => {
                self.focus = match self.focus {
                    FocusArea::Input => FocusArea::Messages,
                    FocusArea::Messages => FocusArea::Input,
                };
                true
            }
            
            // Mode switching
            (FocusArea::Input, EditorMode::Normal, KeyCode::Char('i')) => {
                self.editor_mode = EditorMode::Insert;
                true
            }
            (FocusArea::Input, EditorMode::Insert, KeyCode::Esc) => {
                self.editor_mode = EditorMode::Normal;
                true
            }
            
            // Submit in normal mode
            (FocusArea::Input, EditorMode::Normal, KeyCode::Enter) => {
                self.submit_input();
                true
            }
            
            // History navigation
            (FocusArea::Input, EditorMode::Normal, KeyCode::Char('k')) |
            (FocusArea::Input, _, KeyCode::Up) => {
                if let Some(prev) = self.input_history.previous(&self.input_content) {
                    self.input_content = prev;
                    self.input_cursor = self.input_content.len();
                }
                true
            }
            (FocusArea::Input, EditorMode::Normal, KeyCode::Char('j')) |
            (FocusArea::Input, _, KeyCode::Down) => {
                if let Some(next) = self.input_history.next() {
                    self.input_content = next;
                    self.input_cursor = self.input_content.len();
                }
                true
            }
            
            // Input editing (simple implementation)
            (FocusArea::Input, EditorMode::Insert, KeyCode::Char(c)) => {
                self.input_content.insert(self.input_cursor, c);
                self.input_cursor += 1;
                true
            }
            (FocusArea::Input, EditorMode::Insert, KeyCode::Backspace) => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input_content.remove(self.input_cursor);
                }
                true
            }
            (FocusArea::Input, EditorMode::Insert, KeyCode::Delete) => {
                if self.input_cursor < self.input_content.len() {
                    self.input_content.remove(self.input_cursor);
                }
                true
            }
            (FocusArea::Input, EditorMode::Insert, KeyCode::Left) => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
                true
            }
            (FocusArea::Input, EditorMode::Insert, KeyCode::Right) => {
                if self.input_cursor < self.input_content.len() {
                    self.input_cursor += 1;
                }
                true
            }
            (FocusArea::Input, EditorMode::Insert, KeyCode::Home) => {
                self.input_cursor = 0;
                true
            }
            (FocusArea::Input, EditorMode::Insert, KeyCode::End) => {
                self.input_cursor = self.input_content.len();
                true
            }
            
            // Message scrolling
            (FocusArea::Messages, _, KeyCode::Up) |
            (FocusArea::Messages, _, KeyCode::Char('k')) => {
                if self.messages_scroll < self.messages.len() {
                    self.messages_scroll += 1;
                }
                true
            }
            (FocusArea::Messages, _, KeyCode::Down) |
            (FocusArea::Messages, _, KeyCode::Char('j')) => {
                if self.messages_scroll > 0 {
                    self.messages_scroll -= 1;
                }
                true
            }
            // Toggle auto-scroll
            (FocusArea::Messages, _, KeyCode::Char('a')) => {
                self.toggle_auto_scroll();
                debug!("Auto-scroll toggled: {}", if self.auto_scroll { "enabled" } else { "disabled" });
                true
            }
            (FocusArea::Messages, _, KeyCode::PageUp) => {
                self.messages_scroll += 10;
                if self.messages_scroll > self.messages.len() {
                    self.messages_scroll = self.messages.len();
                }
                true
            }
            (FocusArea::Messages, _, KeyCode::PageDown) => {
                if self.messages_scroll > 10 {
                    self.messages_scroll -= 10;
                } else {
                    self.messages_scroll = 0;
                }
                true
            }
            (FocusArea::Messages, _, KeyCode::Home) => {
                self.messages_scroll = self.messages.len();
                true
            }
            (FocusArea::Messages, _, KeyCode::End) => {
                self.messages_scroll = 0;
                true
            }
            
            // Return to input area
            (FocusArea::Messages, _, KeyCode::Enter) => {
                self.focus = FocusArea::Input;
                true
            }
            
            // Unhandled
            _ => false,
        }
    }
    
    /// Update processing status with a new message
    pub fn update_processing_status(&mut self, status: String) {
        self.processing = match &self.processing {
            ProcessingStatus::Idle | ProcessingStatus::Error(_) => {
                ProcessingStatus::Processing {
                    start_time: Instant::now(),
                    status,
                }
            }
            ProcessingStatus::Connecting => {
                ProcessingStatus::Processing {
                    start_time: Instant::now(),
                    status,
                }
            }
            ProcessingStatus::Processing { start_time, .. } => {
                ProcessingStatus::Processing {
                    start_time: *start_time,
                    status,
                }
            }
        };
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
