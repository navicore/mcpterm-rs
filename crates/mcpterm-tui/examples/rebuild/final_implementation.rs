// Final implementation that brings together all the lessons learned
// This demonstrates a complete solution with:
// - Direct keyboard handling for reliable input
// - Clear focus management
// - Integration with ratatui and edtui
// - Support for VI-style modes
// - Message viewer with scrolling
// - Full edit capabilities in the input area

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
    message_view_state: EditorState, // For message viewer
    input_editor_state: EditorState, // For input editor
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

        // Create initial message view state
        let message_view_state = EditorState::default();
        
        // Create initial input editor state
        let input_editor_state = EditorState::default();
        
        Self {
            messages,
            input_content: String::new(),
            focus: Focus::Input,
            editor_mode: EditorMode::Normal,
            running: true,
            messages_scroll: 0,
            last_key: "None".to_string(),
            message_view_state,
            input_editor_state,
        }
    }
    
    // Add a message to the conversation
    fn add_message(&mut self, content: String, message_type: MessageType) {
        let message = Message::new(content, message_type);
        self.messages.push(message);
        
        // Reset scroll position to see the new message
        self.messages_scroll = 0;
        
        // Update message view
        self.update_message_view();
    }
    
    // Update the message view state
    fn update_message_view(&mut self) {
        // Create a formatted representation of the messages
        let mut content = String::new();
        
        // Calculate which messages to show based on scroll
        let messages_offset = self.messages_scroll;
        let messages_to_show = if messages_offset >= self.messages.len() {
            &[]
        } else {
            &self.messages[0..self.messages.len() - messages_offset]
        };
        
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
        
        // Update the message view state
        self.message_view_state = EditorState::new(Lines::from(content));
        self.message_view_state.mode = EdtuiMode::Normal;
        
        // Apply styling based on message types
        // This would be more complex in a real implementation
    }
    
    // Submit the current input as a message
    fn submit_input(&mut self) {
        // Get current input
        let input = std::mem::take(&mut self.input_content);
        
        // Skip empty input
        if input.trim().is_empty() {
            return;
        }
        
        // Add as user message
        self.add_message(input.clone(), MessageType::User);
        
        // Simulate a response
        let response = format!("Echo: {}", input);
        self.add_message(response, MessageType::Response);
        
        // Clear the input editor state
        self.input_editor_state = EditorState::new(Lines::from(""));
    }
    
    // Handle a key event
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Update last key for debugging
        self.last_key = format!("{:?}", key);
        
        // STEP 1: Handle global keys first
        
        // Tab key always switches focus
        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::Messages => Focus::Input,
                Focus::Input => Focus::Messages,
            };
            return true;
        }
        
        // Esc key always returns to normal mode
        if key.code == KeyCode::Esc {
            self.editor_mode = EditorMode::Normal;
            return true;
        }
        
        // Ctrl+C always quits
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.running = false;
            return true;
        }
        
        // 'q' in normal mode quits
        if key.code == KeyCode::Char('q') && self.editor_mode == EditorMode::Normal {
            self.running = false;
            return true;
        }
        
        // STEP 2: Handle focus-specific keys
        
        match self.focus {
            Focus::Messages => {
                // Message area keys
                match key.code {
                    KeyCode::Char('j') => {
                        if self.messages_scroll > 0 {
                            self.messages_scroll -= 1;
                            self.update_message_view();
                        }
                        return true;
                    }
                    KeyCode::Char('k') => {
                        if self.messages_scroll < self.messages.len() {
                            self.messages_scroll += 1;
                            self.update_message_view();
                        }
                        return true;
                    }
                    KeyCode::Enter => {
                        // Switch focus to input
                        self.focus = Focus::Input;
                        return true;
                    }
                    _ => return false, // Unhandled key
                }
            }
            
            Focus::Input => {
                // STEP 3: Handle mode-switching keys
                
                // 'i' in normal mode -> insert mode
                if key.code == KeyCode::Char('i') && self.editor_mode == EditorMode::Normal {
                    self.editor_mode = EditorMode::Insert;
                    return true;
                }
                
                // 'v' in normal mode -> visual mode
                if key.code == KeyCode::Char('v') && self.editor_mode == EditorMode::Normal {
                    self.editor_mode = EditorMode::Visual;
                    return true;
                }
                
                // STEP 4: Handle mode-specific keys
                
                match self.editor_mode {
                    EditorMode::Normal => {
                        // Normal mode keys
                        match key.code {
                            KeyCode::Enter => {
                                self.submit_input();
                                return true;
                            }
                            _ => return false, // Pass to edtui
                        }
                    }
                    
                    EditorMode::Insert => {
                        // Simply let edtui handle the key for simplicity
                        // In a real implementation, we would handle special cases here
                        match key.code {
                            KeyCode::Char(c) => {
                                self.input_content.push(c);
                                return true;
                            }
                            KeyCode::Backspace => {
                                self.input_content.pop();
                                return true;
                            }
                            KeyCode::Enter => {
                                self.submit_input();
                                return true;
                            }
                            _ => return false, // Pass to edtui
                        }
                    }
                    
                    EditorMode::Visual => {
                        // Visual mode keys
                        match key.code {
                            KeyCode::Char('y') => {
                                // In a real impl, this would copy to clipboard
                                self.add_message(
                                    format!("Copied: {}", self.input_content),
                                    MessageType::System,
                                );
                                self.editor_mode = EditorMode::Normal;
                                return true;
                            }
                            _ => return false, // Pass to edtui
                        }
                    }
                }
            }
        }
    }
    
    // Update input content from editor state
    fn update_input_from_editor(&mut self) {
        // Extract text from the editor state
        let mut result = String::new();
        let lines = &self.input_editor_state.lines;
        
        // Create a string by iterating through the lines
        let mut first_line = true;
        for line in lines.iter_row() {
            if !first_line {
                result.push('\n');
            }
            first_line = false;
            
            for ch in line {
                result.push(*ch);
            }
        }
        
        self.input_content = result;
    }
}

// Message viewer widget
struct MessageViewer<'a> {
    state: &'a mut EditorState,
    block: Option<Block<'a>>,
}

impl<'a> MessageViewer<'a> {
    fn new(state: &'a mut EditorState) -> Self {
        Self {
            state,
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
        // Create an editor view
        let mut view = EditorView::new(self.state);
        
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
    state: &'a mut EditorState,
    block: Option<Block<'a>>,
    event_handler: &'a mut EditorEventHandler,
}

impl<'a> InputEditor<'a> {
    fn new(state: &'a mut EditorState, event_handler: &'a mut EditorEventHandler) -> Self {
        Self {
            state,
            block: None,
            event_handler,
        }
    }
    
    fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
    
    // Get the current text content
    fn get_text(&self) -> String {
        // Extract text from the editor state
        let mut result = String::new();
        let lines = &self.state.lines;
        
        // Create a string by iterating through the lines
        let mut first_line = true;
        for line in lines.iter_row() {
            if !first_line {
                result.push('\n');
            }
            first_line = false;
            
            for ch in line {
                result.push(*ch);
            }
        }
        
        result
    }
    
    // Set the content
    fn set_content(&mut self, content: &str) {
        *self.state = EditorState::new(Lines::from(content));
    }
    
    // Clear the editor content
    fn clear(&mut self) {
        *self.state = EditorState::new(Lines::from(""));
    }
    
    // Set the editor mode
    fn set_mode(&mut self, mode: EditorMode) {
        self.state.mode = mode.into();
    }
}

impl<'a> Widget for InputEditor<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create an editor view
        let mut view = EditorView::new(self.state);
        
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

fn ui(
    f: &mut ratatui::Frame,
    state: &mut AppState,
    message_viewer_ref: &mut MessageViewer,
    input_editor_ref: &mut InputEditor,
) {
    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.size());
    
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
    
    // Set block for message viewer
    message_viewer_ref.block = Some(messages_block);
    
    // Render message viewer
    f.render_widget(message_viewer_ref.clone(), chunks[0]);
    
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
    
    // Set block for input editor
    input_editor_ref.block = Some(input_block);
    
    // Set correct mode for input editor
    input_editor_ref.set_mode(state.editor_mode);
    
    // Render input editor
    f.render_widget(input_editor_ref.clone(), chunks[1]);
    
    // Position cursor for edtui to handle
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
    
    // Initialize message viewer state
    state.update_message_view();
    
    // Create editor event handler
    let mut editor_event_handler = EditorEventHandler::default();
    
    // Main loop
    while state.running {
        // Render the UI
        let message_viewer = MessageViewer::new(&mut state.message_view_state);
        let input_editor = InputEditor::new(
            &mut state.input_editor_state,
            &mut editor_event_handler,
        );
        
        terminal.draw(|f| {
            ui(f, &mut state, &mut message_viewer.clone(), &mut input_editor.clone())
        })?;
        
        // Sync state with editor
        state.update_input_from_editor();
        
        // Handle input with a short timeout to avoid CPU spinning
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Only process press events (not releases or repeats)
                    if key.kind == KeyEventKind::Press {
                        // Handle the key in our state first
                        let handled = state.handle_key(key);
                        
                        // If not handled and input is focused, pass to edtui
                        if !handled && state.focus == Focus::Input {
                            // Convert KeyEvent to ratatui KeyEvent (this may need adjustment)
                            let ratatui_key = convert_key_event_to_ratatui(key);
                            
                            // Let edtui handle the key event
                            editor_event_handler.on_key_event(
                                ratatui_key,
                                &mut state.input_editor_state,
                            );
                            
                            // Extract text from editor state
                            state.update_input_from_editor();
                        }
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

// Convert crossterm KeyEvent to ratatui KeyEvent
fn convert_key_event_to_ratatui(
    key: crossterm::event::KeyEvent,
) -> ratatui::crossterm::event::KeyEvent {
    // Convert KeyCode
    let code = match key.code {
        KeyCode::Backspace => ratatui::crossterm::event::KeyCode::Backspace,
        KeyCode::Enter => ratatui::crossterm::event::KeyCode::Enter,
        KeyCode::Left => ratatui::crossterm::event::KeyCode::Left,
        KeyCode::Right => ratatui::crossterm::event::KeyCode::Right,
        KeyCode::Up => ratatui::crossterm::event::KeyCode::Up,
        KeyCode::Down => ratatui::crossterm::event::KeyCode::Down,
        KeyCode::Home => ratatui::crossterm::event::KeyCode::Home,
        KeyCode::End => ratatui::crossterm::event::KeyCode::End,
        KeyCode::PageUp => ratatui::crossterm::event::KeyCode::PageUp,
        KeyCode::PageDown => ratatui::crossterm::event::KeyCode::PageDown,
        KeyCode::Tab => ratatui::crossterm::event::KeyCode::Tab,
        KeyCode::BackTab => ratatui::crossterm::event::KeyCode::BackTab,
        KeyCode::Delete => ratatui::crossterm::event::KeyCode::Delete,
        KeyCode::Insert => ratatui::crossterm::event::KeyCode::Insert,
        KeyCode::F(n) => ratatui::crossterm::event::KeyCode::F(n),
        KeyCode::Char(c) => ratatui::crossterm::event::KeyCode::Char(c),
        KeyCode::Null => ratatui::crossterm::event::KeyCode::Null,
        KeyCode::Esc => ratatui::crossterm::event::KeyCode::Esc,
        _ => ratatui::crossterm::event::KeyCode::Null,
    };
    
    // Convert KeyModifiers
    let mut modifiers = ratatui::crossterm::event::KeyModifiers::empty();
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        modifiers.insert(ratatui::crossterm::event::KeyModifiers::SHIFT);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        modifiers.insert(ratatui::crossterm::event::KeyModifiers::CONTROL);
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        modifiers.insert(ratatui::crossterm::event::KeyModifiers::ALT);
    }
    
    // Create the ratatui KeyEvent
    ratatui::crossterm::event::KeyEvent {
        code,
        modifiers,
        kind: ratatui::crossterm::event::KeyEventKind::Press,
        state: ratatui::crossterm::event::KeyEventState::NONE,
    }
}