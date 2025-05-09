// Fixed implementation based on lessons from step5
// This fixes key handling issues by:
// 1. Using direct key handling from step5 (no boolean returns)
// 2. Simplifying state management and synchronization
// 3. Making focus management more explicit

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use edtui::{
    EditorEventHandler, EditorMode as EdtuiMode, EditorState, EditorTheme, EditorView, Lines, StyleRange, TextStyle,
};
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line as TextLine, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};

// Focus management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Messages,
    Input,
}

// Editor modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    Normal,
    Insert,
    Visual,
}

// Message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageType {
    System,
    User,
    Response,
    Error,
}

// A message in the conversation
#[derive(Debug, Clone)]
struct Message {
    content: String,
    message_type: MessageType,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl Message {
    fn new(content: String, message_type: MessageType) -> Self {
        Self {
            content,
            message_type,
            timestamp: chrono::Utc::now(),
        }
    }
}

// Convert between our mode enum and edtui's mode enum
impl From<EditorMode> for EdtuiMode {
    fn from(mode: EditorMode) -> Self {
        match mode {
            EditorMode::Normal => EdtuiMode::Normal,
            EditorMode::Insert => EdtuiMode::Insert,
            EditorMode::Visual => EdtuiMode::Visual,
        }
    }
}

impl From<EdtuiMode> for EditorMode {
    fn from(mode: EdtuiMode) -> Self {
        match mode {
            EdtuiMode::Normal => EditorMode::Normal,
            EdtuiMode::Insert => EditorMode::Insert,
            EdtuiMode::Visual => EditorMode::Visual,
            EdtuiMode::Search => EditorMode::Normal, // Map search mode to normal mode
        }
    }
}

// Application state
struct AppState {
    messages: Vec<Message>,
    input_content: String,
    focus: Focus,
    editor_mode: EditorMode,
    running: bool,
    messages_scroll: usize,
    last_key: String,  // For debugging
}

impl AppState {
    fn new() -> Self {
        // Create initial messages
        let mut messages = Vec::new();
        messages.push(Message::new(
            "Welcome to the Terminal UI Application".to_string(),
            MessageType::System,
        ));
        messages.push(Message::new(
            "Tab: Switch focus between messages and input".to_string(),
            MessageType::System,
        ));
        messages.push(Message::new(
            "i: Enter insert mode, Esc: Return to normal mode".to_string(),
            MessageType::System,
        ));
        messages.push(Message::new(
            "j/k: Scroll messages (when messages focused)".to_string(),
            MessageType::System,
        ));
        messages.push(Message::new(
            "Enter: Submit message (in input area)".to_string(),
            MessageType::System,
        ));
        messages.push(Message::new(
            "q: Quit application (in normal mode)".to_string(),
            MessageType::System,
        ));
        
        Self {
            messages,
            input_content: String::new(),
            focus: Focus::Input,
            editor_mode: EditorMode::Normal,
            running: true,
            messages_scroll: 0,
            last_key: "None".to_string(),
        }
    }
    
    // Add a message to the conversation
    fn add_message(&mut self, content: String, message_type: MessageType) {
        let message = Message::new(content, message_type);
        self.messages.push(message);
        
        // Reset scroll position to see the new message
        self.messages_scroll = 0;
    }
    
    // Submit the current input as a message
    fn submit_input(&mut self) {
        // Skip empty input
        if self.input_content.trim().is_empty() {
            return;
        }
        
        // Add as user message
        let input = self.input_content.clone();
        self.add_message(input.clone(), MessageType::User);
        
        // Simulate a response
        let response = format!("Echo: {}", input);
        self.add_message(response, MessageType::Response);
        
        // Clear the input
        self.input_content.clear();
    }
    
    // Handle a key event - using direct approach from step5
    fn handle_key(&mut self, key: KeyEvent) {
        // Update last key for debugging
        self.last_key = format!("{:?}", key);
        
        // STEP 1: Handle global keys first
        match key.code {
            KeyCode::Tab => {
                // Toggle focus - always works
                self.focus = match self.focus {
                    Focus::Messages => Focus::Input,
                    Focus::Input => Focus::Messages,
                };
                self.add_message(
                    format!("Focus switched to {:?}", self.focus),
                    MessageType::System,
                );
                return;
            }
            KeyCode::Esc => {
                // Escape always returns to normal mode regardless of focus
                self.editor_mode = EditorMode::Normal;
                self.add_message(
                    "Switched to Normal mode".to_string(),
                    MessageType::System,
                );
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
                return;
            }
            KeyCode::Char('q') if self.editor_mode == EditorMode::Normal => {
                self.running = false;
                return;
            }
            _ => {}
        }
        
        // STEP 2: Handle focus-specific keys
        match self.focus {
            Focus::Messages => {
                // Message viewer controls
                match key.code {
                    KeyCode::Char('j') => {
                        // Scroll down
                        if self.messages_scroll > 0 {
                            self.messages_scroll -= 1;
                        }
                    }
                    KeyCode::Char('k') => {
                        // Scroll up
                        if self.messages_scroll < self.messages.len() {
                            self.messages_scroll += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // Enter switches focus to input
                        self.focus = Focus::Input;
                        self.add_message(
                            "Focus switched to Input".to_string(),
                            MessageType::System,
                        );
                    }
                    _ => {}
                }
            }
            
            Focus::Input => {
                // Handle mode switching
                if key.code == KeyCode::Char('i') && self.editor_mode == EditorMode::Normal {
                    self.editor_mode = EditorMode::Insert;
                    self.add_message(
                        "Switched to Insert mode".to_string(),
                        MessageType::System,
                    );
                    return;
                }
                
                if key.code == KeyCode::Char('v') && self.editor_mode == EditorMode::Normal {
                    self.editor_mode = EditorMode::Visual;
                    self.add_message(
                        "Switched to Visual mode".to_string(),
                        MessageType::System,
                    );
                    return;
                }
                
                // Handle mode-specific keys
                match self.editor_mode {
                    EditorMode::Normal => {
                        // Normal mode commands
                        if key.code == KeyCode::Enter {
                            self.submit_input();
                        }
                    }
                    EditorMode::Insert => {
                        // Insert mode for text editing
                        match key.code {
                            KeyCode::Char(c) => {
                                self.input_content.push(c);
                            }
                            KeyCode::Backspace => {
                                self.input_content.pop();
                            }
                            KeyCode::Enter => {
                                self.submit_input();
                            }
                            _ => {}
                        }
                    }
                    EditorMode::Visual => {
                        // Visual mode for selection
                        match key.code {
                            KeyCode::Char('y') => {
                                // In a real impl, this would copy to clipboard
                                self.add_message(
                                    format!("Copied: {}", self.input_content),
                                    MessageType::System,
                                );
                                self.editor_mode = EditorMode::Normal;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

// Message viewer widget
struct MessageViewer<'a> {
    messages: &'a [Message],
    scroll: usize,
    block: Option<Block<'a>>,
}

impl<'a> MessageViewer<'a> {
    fn new(messages: &'a [Message], scroll: usize) -> Self {
        Self {
            messages,
            scroll,
            block: None,
        }
    }
    
    fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for MessageViewer<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate which messages to show based on scroll
        let messages_offset = self.scroll;
        let messages_to_show = if messages_offset >= self.messages.len() {
            &[]
        } else {
            &self.messages[0..self.messages.len() - messages_offset]
        };
        
        // Create formatted content
        let mut content = String::new();
        
        // Format messages
        for message in messages_to_show {
            // Add proper headers based on message type
            let header = match message.message_type {
                MessageType::System => "System: ",
                MessageType::User => "You: ",
                MessageType::Response => "Assistant: ",
                MessageType::Error => "Error: ",
            };
            
            // Format timestamp
            let timestamp = message.timestamp.format("[%H:%M:%S]");
            
            // Add formatted message
            if !content.is_empty() {
                content.push_str("\n\n");
            }
            
            content.push_str(&format!("{} {}\n", timestamp, header));
            content.push_str(&message.content);
        }
        
        // Create editor state with the formatted content
        let mut state = EditorState::new(Lines::from(content));
        state.mode = EdtuiMode::Normal;
        
        // Create an editor view
        let mut view = EditorView::new(&mut state);
        
        // Create theme with our block
        let theme = if let Some(block) = self.block {
            EditorTheme::default().block(block)
        } else {
            EditorTheme::default()
        };
        
        // Set theme and word wrap
        let view = view.theme(theme).wrap(true);
        
        // Render the view
        view.render(area, buf);
    }
}

// Input editor widget
struct InputEditor<'a> {
    content: &'a str,
    mode: EditorMode,
    block: Option<Block<'a>>,
}

impl<'a> InputEditor<'a> {
    fn new(content: &'a str, mode: EditorMode) -> Self {
        Self {
            content,
            mode,
            block: None,
        }
    }
    
    fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for InputEditor<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a new editor state with the content
        let mut state = EditorState::new(Lines::from(self.content));
        
        // Set the mode
        state.mode = self.mode.into();
        
        // Create an editor view
        let mut view = EditorView::new(&mut state);
        
        // Create theme with our block
        let theme = if let Some(block) = self.block {
            EditorTheme::default().block(block)
        } else {
            EditorTheme::default()
        };
        
        // Set theme and word wrap
        let view = view.theme(theme).wrap(true);
        
        // Render the view
        view.render(area, buf);
    }
}

// The UI function - kept as a separate function like in step5
fn ui(f: &mut ratatui::Frame, state: &AppState) {
    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());
    
    // Determine message area border style based on focus
    let message_border_style = if state.focus == Focus::Messages {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    // Create messages block
    let messages_block = Block::default()
        .title(format!("Messages (scroll: {})", state.messages_scroll))
        .borders(Borders::ALL)
        .border_style(message_border_style);
    
    // Create and render message viewer
    let message_viewer = MessageViewer::new(&state.messages, state.messages_scroll)
        .block(messages_block);
    f.render_widget(message_viewer, chunks[0]);
    
    // Determine input area border style based on focus
    let input_border_style = if state.focus == Focus::Input {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    // Create input block with mode info
    let mode_str = match state.editor_mode {
        EditorMode::Normal => "Normal Mode - 'i' to insert, 'v' for visual, 'q' to quit",
        EditorMode::Insert => "Insert Mode - ESC for normal mode",
        EditorMode::Visual => "Visual Mode - 'y' to copy, ESC for normal mode",
    };
    
    let input_block = Block::default()
        .title(format!("Input ({}) | Last key: {}", mode_str, state.last_key))
        .borders(Borders::ALL)
        .border_style(input_border_style);
    
    // Create and render input editor
    let input_editor = InputEditor::new(&state.input_content, state.editor_mode)
        .block(input_block);
    f.render_widget(input_editor, chunks[1]);
    
    // Position cursor in input field if input is focused and in insert mode
    if state.focus == Focus::Input && state.editor_mode == EditorMode::Insert {
        // Calculate cursor position - account for border and content length
        let cursor_x = chunks[1].x + 1 + state.input_content.len() as u16;
        let cursor_y = chunks[1].y + 1;
        
        f.set_cursor(cursor_x, cursor_y);
    }
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Create app state
    let mut state = AppState::new();
    
    // Main loop
    while state.running {
        // Render the UI
        terminal.draw(|f| ui(f, &state))?;
        
        // Handle input with a short timeout to avoid CPU spinning
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Only process press events (not releases or repeats)
                    if key.kind == KeyEventKind::Press {
                        // Handle the key directly - no boolean returns or two-stage handling
                        state.handle_key(key);
                    }
                }
                Event::Resize(_, _) => {
                    // Just redraw on resize, handled by terminal.draw()
                }
                _ => {}
            }
        }
    }
    
    // Clean up
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    Ok(())
}