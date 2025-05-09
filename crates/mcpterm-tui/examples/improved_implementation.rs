// Improved implementation combining the best of step5 and final
// This preserves step5's direct key handling approach while incorporating 
// valuable features from the final implementation
//
// Key improvements:
// 1. Direct key handling with reliable Tab and j/k navigation
// 2. Real-time input display with cursor positioning
// 3. Rich message formatting with message types and timestamps
// 4. Proper focus management with active highlighting

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use edtui::{
    EditorMode as EdtuiMode, EditorState, EditorTheme, EditorView, Lines,
};
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
    Terminal,
};
use std::io;
use std::time::Duration;
use chrono::{DateTime, Utc};

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
    Tool,
}

// A message in the conversation
#[derive(Debug, Clone)]
struct Message {
    content: String,
    message_type: MessageType,
    timestamp: DateTime<Utc>,
}

impl Message {
    fn new(content: String, message_type: MessageType) -> Self {
        Self {
            content,
            message_type,
            timestamp: Utc::now(),
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

// Simple application state
struct AppState {
    messages: Vec<Message>,
    input: String,
    focus: Focus,
    mode: EditorMode,
    running: bool,
    message_scroll: usize,
    last_key: String,  // For debugging
    history: Vec<String>, // Command history
    history_index: usize, // Current position in history
}

impl AppState {
    fn new() -> Self {
        // Initialize with some help messages
        let mut messages = vec![
            Message::new(
                "Terminal UI with Direct Keyboard Handling".to_string(),
                MessageType::System,
            ),
            Message::new(
                "==========================================".to_string(),
                MessageType::System,
            ),
            Message::new(
                "Tab: Switch focus".to_string(),
                MessageType::System,
            ),
            Message::new(
                "i: Enter insert mode".to_string(),
                MessageType::System,
            ), 
            Message::new(
                "Esc: Return to normal mode".to_string(),
                MessageType::System,
            ),
            Message::new(
                "j/k: Scroll messages (when messages focused)".to_string(),
                MessageType::System,
            ),
            Message::new(
                "Enter: Submit message (in input area)".to_string(),
                MessageType::System,
            ),
            Message::new(
                "Up/Down: Navigate input history (in input area)".to_string(),
                MessageType::System,
            ),
            Message::new(
                "q: Quit (in normal mode)".to_string(),
                MessageType::System,
            ),
        ];
        
        Self {
            messages,
            input: String::new(),
            focus: Focus::Input,
            mode: EditorMode::Normal,
            running: true,
            message_scroll: 0,
            last_key: "None".to_string(),
            history: Vec::new(),
            history_index: 0,
        }
    }
    
    // Add a message to the list
    fn add_message(&mut self, content: String, message_type: MessageType) {
        let message = Message::new(content, message_type);
        self.messages.push(message);
        // Reset scroll position to see the new message
        self.message_scroll = 0;
    }
    
    // Submit the current input
    fn submit_input(&mut self) {
        if !self.input.is_empty() {
            let input_text = self.input.clone();
            
            // Add to history (avoid duplicates)
            if self.history.is_empty() || self.history.last().unwrap() != &input_text {
                self.history.push(input_text.clone());
            }
            self.history_index = self.history.len();
            
            // Add user message
            self.add_message(input_text.clone(), MessageType::User);
            
            // Add a simulated response message
            self.add_message(format!("Echo: {}", input_text), MessageType::Response);
            
            // Clear the input
            self.input.clear();
        }
    }
    
    // Navigate history with up/down keys
    fn navigate_history(&mut self, direction: KeyCode) {
        if self.history.is_empty() {
            return;
        }
        
        match direction {
            KeyCode::Up => {
                if self.history_index > 0 {
                    self.history_index -= 1;
                    self.input = self.history[self.history_index].clone();
                }
            }
            KeyCode::Down => {
                if self.history_index < self.history.len() - 1 {
                    self.history_index += 1;
                    self.input = self.history[self.history_index].clone();
                } else if self.history_index == self.history.len() - 1 {
                    // At the end of history, clear input
                    self.history_index = self.history.len();
                    self.input.clear();
                }
            }
            _ => {}
        }
    }
    
    // Handle a key event
    fn handle_key(&mut self, key: KeyEvent) {
        // Update last key for debugging
        self.last_key = format!("{:?}", key);
        
        // Handle global keys first
        match key.code {
            KeyCode::Tab => {
                // Toggle focus
                self.focus = match self.focus {
                    Focus::Messages => Focus::Input,
                    Focus::Input => Focus::Messages,
                };
                self.add_message(format!("Focus switched to {:?}", self.focus), MessageType::System);
                return;
            }
            KeyCode::Esc => {
                // Escape always returns to normal mode regardless of focus
                self.mode = EditorMode::Normal;
                self.add_message("Switched to Normal mode".to_string(), MessageType::System);
                return;
            }
            KeyCode::Char('q') if self.mode == EditorMode::Normal => {
                self.running = false;
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
                return;
            }
            _ => {}
        }
        
        // Handle focus-specific keys
        match self.focus {
            Focus::Messages => {
                // Message viewer controls
                match key.code {
                    KeyCode::Char('j') => {
                        // Scroll down
                        if self.message_scroll > 0 {
                            self.message_scroll -= 1;
                        }
                    }
                    KeyCode::Char('k') => {
                        // Scroll up
                        if self.message_scroll < self.messages.len() {
                            self.message_scroll += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // Enter switches focus to input
                        self.focus = Focus::Input;
                        self.add_message("Focus switched to Input".to_string(), MessageType::System);
                    }
                    _ => {}
                }
            }
            Focus::Input => {
                // Handle mode switching
                if key.code == KeyCode::Char('i') && self.mode == EditorMode::Normal {
                    self.mode = EditorMode::Insert;
                    self.add_message("Switched to Insert mode".to_string(), MessageType::System);
                    return;
                }
                
                if key.code == KeyCode::Char('v') && self.mode == EditorMode::Normal {
                    self.mode = EditorMode::Visual;
                    self.add_message("Switched to Visual mode".to_string(), MessageType::System);
                    return;
                }
                
                // Handle mode-specific keys
                match self.mode {
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
                                self.input.push(c);
                            }
                            KeyCode::Backspace => {
                                self.input.pop();
                            }
                            KeyCode::Enter => {
                                self.submit_input();
                            }
                            KeyCode::Up | KeyCode::Down => {
                                self.navigate_history(key.code);
                            }
                            _ => {}
                        }
                    }
                    EditorMode::Visual => {
                        // Visual mode for selection (simplified here)
                        match key.code {
                            KeyCode::Char('y') => {
                                self.add_message(format!("Copied: {}", self.input), MessageType::System);
                                self.mode = EditorMode::Normal;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

// Custom InputEditor widget that uses edtui
struct InputEditor<'a> {
    block: Option<Block<'a>>,
    content: &'a str,
    mode: EditorMode,
}

impl<'a> InputEditor<'a> {
    fn new(content: &'a str, mode: EditorMode) -> Self {
        Self {
            block: None,
            content,
            mode,
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
        .title(format!("Messages (scroll: {})", state.message_scroll))
        .borders(Borders::ALL)
        .border_style(message_border_style);
    
    // Calculate which messages to show based on scroll
    let messages_offset = state.message_scroll;
    let messages_to_show = if messages_offset >= state.messages.len() {
        &[]
    } else {
        &state.messages[0..state.messages.len() - messages_offset]
    };
    
    // Create a Text widget for messages with proper styling
    let message_items: Vec<Line> = messages_to_show
        .iter()
        .map(|m| {
            // Format timestamp
            let timestamp = m.timestamp.format("[%H:%M:%S]");
            
            // Different styling based on message type
            let (prefix, style) = match m.message_type {
                MessageType::System => ("System: ", Style::default().fg(Color::Blue)),
                MessageType::User => ("You: ", Style::default().fg(Color::Yellow)),
                MessageType::Response => ("Assistant: ", Style::default().fg(Color::Green)),
                MessageType::Error => ("Error: ", Style::default().fg(Color::Red)),
                MessageType::Tool => ("Tool: ", Style::default().fg(Color::Magenta)),
            };
            
            // Create a line with multiple spans for formatted output
            Line::from(vec![
                Span::styled(format!("{} ", timestamp), Style::default().fg(Color::DarkGray)),
                Span::styled(prefix, style),
                Span::styled(&m.content, Style::default()),
            ])
        })
        .collect();
    
    // Create a Text object from the message items
    let messages_text = Text::from(message_items);
    
    // Create messages paragraph
    let messages_widget = Paragraph::new(messages_text)
        .block(messages_block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    // Render messages widget
    f.render_widget(messages_widget, chunks[0]);
    
    // Determine input area border style based on focus
    let input_border_style = if state.focus == Focus::Input {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    // Create input block with mode info
    let mode_str = match state.mode {
        EditorMode::Normal => "Normal Mode - 'i' to insert, 'v' for visual, 'q' to quit",
        EditorMode::Insert => "Insert Mode - ESC for normal mode",
        EditorMode::Visual => "Visual Mode - 'y' to copy, ESC for normal mode",
    };
    
    let input_block = Block::default()
        .title(format!("Input ({}) | Last key: {}", mode_str, state.last_key))
        .borders(Borders::ALL)
        .border_style(input_border_style);
    
    // Create the input editor widget using edtui
    let input_editor = InputEditor::new(&state.input, state.mode)
        .block(input_block);
    
    // Render input editor widget
    f.render_widget(input_editor, chunks[1]);
    
    // Position cursor in input field if input is focused and in insert mode
    if state.focus == Focus::Input && state.mode == EditorMode::Insert {
        f.set_cursor(
            chunks[1].x + state.input.len() as u16 + 1, // +1 for the block border
            chunks[1].y + 1,                           // +1 for the block border
        );
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
                        // Handle the key
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