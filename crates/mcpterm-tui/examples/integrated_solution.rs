// Integrated solution that combines:
// 1. Working message scrolling from simple_solution.rs
// 2. EdTUI-based input editor from rebuild_step5_with_edtui.rs
//
// This provides a solid foundation for integration into the main codebase

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
    style::{Color, Style},
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

// Application state
struct AppState {
    messages: Vec<Message>,
    input: String,
    focus: Focus,
    mode: EditorMode,
    running: bool,
    scroll: usize, // Index of first message to show (0 = oldest)
    auto_scroll: bool, // Whether to automatically scroll to the bottom when new messages are added
    visible_message_count: usize, // Approximate number of messages that can be shown
    last_key: String, // For debugging
    history: Vec<String>, // Command history
    history_index: usize, // Current position in history
}

impl AppState {
    fn new() -> Self {
        // Initialize with some help messages
        let mut messages = vec![
            Message::new(
                "Terminal UI with Integrated Solution".to_string(),
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
                "g/G: Jump to oldest/newest messages".to_string(),
                MessageType::System,
            ),
            Message::new(
                "a: Toggle auto-scroll".to_string(),
                MessageType::System,
            ),
            Message::new(
                "Up/Down: Navigate input history (in input area)".to_string(),
                MessageType::System,
            ),
            Message::new(
                "Enter: Submit message (in input area)".to_string(),
                MessageType::System,
            ),
            Message::new(
                "q: Quit (in normal mode)".to_string(),
                MessageType::System,
            ),
            // Example messages of different types
            Message::new(
                "This is an example error message".to_string(),
                MessageType::Error,
            ),
            Message::new(
                "This is an example tool output message".to_string(),
                MessageType::Tool,
            ),
        ];
        
        Self {
            messages,
            input: String::new(),
            focus: Focus::Input,
            mode: EditorMode::Normal,
            running: true,
            scroll: 0,
            auto_scroll: true, // Auto-scroll by default
            visible_message_count: 10, // Will be calculated dynamically
            last_key: "None".to_string(),
            history: Vec::new(),
            history_index: 0,
        }
    }
    
    // Add a message to the list
    fn add_message(&mut self, content: String, message_type: MessageType) {
        let message = Message::new(content, message_type);
        self.messages.push(message);
        
        // If auto-scroll is enabled, scroll to the bottom
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }
    
    // Scroll to the bottom to show newest messages
    fn scroll_to_bottom(&mut self) {
        // Calculate the ideal scroll position to show the most recent messages
        let max_scroll = self.messages.len().saturating_sub(self.visible_message_count);
        self.scroll = max_scroll;
    }
    
    // Scroll to the top to show oldest messages
    fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }
    
    // Calculate how many messages can be displayed based on area height
    fn update_visible_message_count(&mut self, area_height: u16) {
        // Use a more conservative estimate: each message takes approximately 2 lines
        // Subtract 2 for the borders
        let available_lines = area_height.saturating_sub(2);
        self.visible_message_count = (available_lines as usize).max(1);
        
        // Ensure at least one message can be shown
        self.visible_message_count = self.visible_message_count.max(1);
        
        // Ensure valid scroll position after resizing
        let max_scroll = self.messages.len().saturating_sub(self.visible_message_count);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }
    
    // Submit the current input
    fn submit_input(&mut self) {
        if !self.input.is_empty() {
            // Get the current input
            let input_text = self.input.clone();
            
            // Add to history (avoid duplicates)
            if self.history.is_empty() || self.history.last().unwrap() != &input_text {
                self.history.push(input_text.clone());
            }
            self.history_index = self.history.len();
            
            // Add user message
            self.add_message(input_text.clone(), MessageType::User);
            
            // Add a simulated response
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
                return;
            }
            KeyCode::Esc => {
                // Escape always returns to normal mode regardless of focus
                self.mode = EditorMode::Normal;
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
                        // Scroll down (show newer messages)
                        if self.scroll > 0 {
                            self.scroll -= 1;
                        }
                    }
                    KeyCode::Char('k') => {
                        // Scroll up (show older messages)
                        let max_scroll = self.messages.len().saturating_sub(self.visible_message_count);
                        if self.scroll < max_scroll {
                            self.scroll += 1;
                        }
                    }
                    KeyCode::Char('g') => {
                        // Go to top (oldest messages)
                        self.scroll_to_top();
                    }
                    KeyCode::Char('G') => {
                        // Go to bottom (newest messages)
                        self.scroll_to_bottom();
                    }
                    KeyCode::Char('a') => {
                        // Toggle auto-scroll
                        self.auto_scroll = !self.auto_scroll;
                        if self.auto_scroll {
                            self.scroll_to_bottom();
                        }
                    }
                    KeyCode::Enter => {
                        // Enter switches focus to input
                        self.focus = Focus::Input;
                    }
                    _ => {}
                }
            }
            
            Focus::Input => {
                // Handle mode switching
                if key.code == KeyCode::Char('i') && self.mode == EditorMode::Normal {
                    self.mode = EditorMode::Insert;
                    return;
                }
                
                if key.code == KeyCode::Char('v') && self.mode == EditorMode::Normal {
                    self.mode = EditorMode::Visual;
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
        let view = EditorView::new(&mut state);
        
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

fn ui(f: &mut ratatui::Frame, state: &mut AppState) {
    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());
    
    // Update the number of messages that can be displayed in this area
    state.update_visible_message_count(chunks[0].height);
    
    // Determine message area border style based on focus
    let message_border_style = if state.focus == Focus::Messages {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    // Create messages block with scroll status
    let auto_scroll_indicator = if state.auto_scroll { "AUTO" } else { "MANUAL" };
    let max_scroll = state.messages.len().saturating_sub(state.visible_message_count);
    
    let messages_block = Block::default()
        .title(format!("Messages ({}/{}) - {}", 
                      state.scroll, 
                      max_scroll,
                      auto_scroll_indicator))
        .borders(Borders::ALL)
        .border_style(message_border_style);
    
    // Calculate which messages to show based on scroll
    // We want to fill the entire visible area with messages
    // So we'll show as many messages as possible starting from scroll position
    let start_idx = state.scroll;
    let end_idx = state.messages.len();  // Show all messages from start_idx to the end
    let messages_to_show = &state.messages[start_idx..end_idx];
    
    // Create message content with styling
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
    
    // Create a Text widget for messages
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
        f.set_cursor_position(
            (
                chunks[1].x + state.input.len() as u16 + 1, // +1 for the block border
                chunks[1].y + 1,                           // +1 for the block border
            )
        );
    }
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    
    // Enter alternate screen - catch errors that might occur in tmux or other environments
    match execute!(stdout, EnterAlternateScreen) {
        Ok(_) => {},
        Err(e) => {
            eprintln!("Warning: Failed to enter alternate screen: {}", e);
            // Continue anyway
        }
    }
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            // Try to clean up if we fail
            let _ = disable_raw_mode();
            return Err(anyhow::anyhow!("Failed to create terminal: {}", e));
        }
    };
    
    // Create app state
    let mut state = AppState::new();
    
    // Main loop
    while state.running {
        // Render the UI - passing mutable state to update visible message count
        terminal.draw(|f| ui(f, &mut state))?;
        
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
                    // Just redraw on resize, handled in terminal.draw()
                }
                _ => {}
            }
        }
    }
    
    // Clean up with error handling
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
    
    Ok(())
}