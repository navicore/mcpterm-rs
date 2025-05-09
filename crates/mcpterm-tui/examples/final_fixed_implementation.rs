// Fixed implementation with reliable scrolling
// Based on the successful step5 model but with improved scrolling behavior

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
    layout::{Constraint, Direction, Layout, Rect, Margin},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget, Scrollbar, ScrollbarOrientation, ScrollbarState},
    symbols::scrollbar::VERTICAL,
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
    view_start_index: usize, // The index of the first message to display
    visible_message_count: usize, // How many messages can be displayed at once
    last_key: String,  // For debugging
    history: Vec<String>, // Command history
    history_index: usize, // Current position in history
    autoscroll: bool,  // Whether to automatically scroll to see new messages
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
                "a: Toggle autoscroll in message viewer".to_string(), 
                MessageType::System,
            ),
            Message::new(
                "G: Scroll to bottom of messages".to_string(),
                MessageType::System,
            ),
            Message::new(
                "q: Quit (in normal mode)".to_string(),
                MessageType::System,
            ),
            // Example messages of each type (to demonstrate styling)
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
            view_start_index: 0,
            visible_message_count: 10, // Estimated - will be calculated during rendering
            last_key: "None".to_string(),
            history: Vec::new(),
            history_index: 0,
            autoscroll: true, // Enable autoscroll by default
        }
    }
    
    // Add a message to the list
    fn add_message(&mut self, content: String, message_type: MessageType) {
        let message = Message::new(content, message_type);
        self.messages.push(message);
        
        // If autoscroll is enabled, scroll to the bottom
        if self.autoscroll {
            self.scroll_to_bottom();
        }
    }
    
    // Calculate and return the total number of messages
    fn message_count(&self) -> usize {
        self.messages.len()
    }
    
    // Get visible messages for the current scroll position
    fn visible_messages(&self) -> &[Message] {
        if self.messages.is_empty() {
            return &[];
        }
        
        // Calculate the range of messages to display
        let start = self.view_start_index;
        let end = (start + self.visible_message_count)
            .min(self.messages.len());
        
        &self.messages[start..end]
    }
    
    // Scroll down by one message (shows newer messages)
    fn scroll_down(&mut self) {
        if self.view_start_index > 0 {
            self.view_start_index -= 1;
        }
    }
    
    // Scroll up by one message (shows older messages)
    fn scroll_up(&mut self) {
        let max_start = self.messages.len().saturating_sub(self.visible_message_count);
        if self.view_start_index < max_start {
            self.view_start_index += 1;
        }
    }
    
    // Scroll to the bottom (showing the newest messages)
    fn scroll_to_bottom(&mut self) {
        self.view_start_index = 0;
    }
    
    // Scroll to the top (showing the oldest messages)
    fn scroll_to_top(&mut self) {
        let max_start = self.messages.len().saturating_sub(self.visible_message_count);
        self.view_start_index = max_start;
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
            
            // Ensure we're scrolled to see the new messages if autoscroll is enabled
            if self.autoscroll {
                self.scroll_to_bottom();
            }
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
    
    // Update the number of messages that can be displayed based on the area height
    fn update_visible_message_count(&mut self, area_height: u16) {
        // Estimate 3 lines per message (including padding)
        // Subtract 2 for the borders
        self.visible_message_count = (area_height.saturating_sub(2) / 3) as usize;
        
        // Ensure valid scroll position after resizing
        let max_start = self.messages.len().saturating_sub(self.visible_message_count);
        if self.view_start_index > max_start {
            self.view_start_index = max_start;
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
                        // Scroll down - show newer messages
                        self.scroll_down();
                    }
                    KeyCode::Char('k') => {
                        // Scroll up - show older messages
                        self.scroll_up();
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
                        // Toggle autoscroll
                        self.autoscroll = !self.autoscroll;
                        self.add_message(
                            format!("Autoscroll {}", if self.autoscroll { "enabled" } else { "disabled" }),
                            MessageType::System,
                        );
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

// UI rendering function
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
    
    // Get the current scroll status for display
    let autoscroll_status = if state.autoscroll { "AUTO" } else { "FIXED" };
    let max_scroll = state.messages.len().saturating_sub(state.visible_message_count);
    
    // Create messages block with scroll status
    let messages_block = Block::default()
        .title(format!("Messages ({}/{} - {})",
                      state.view_start_index, 
                      max_scroll,
                      autoscroll_status))
        .borders(Borders::ALL)
        .border_style(message_border_style);
    
    // Get the visible messages based on current scroll position
    let visible_messages = state.visible_messages();
    
    // Create message content with styling
    let mut message_text = String::new();
    
    // Format each message with proper styling
    for message in visible_messages.iter().rev() { // Reversed to show newest at bottom
        // Add message header based on type
        let header = match message.message_type {
            MessageType::System => "System: ",
            MessageType::User => "You: ",
            MessageType::Response => "Assistant: ",
            MessageType::Error => "Error: ",
            MessageType::Tool => "Tool: ",
        };
        
        // Format timestamp
        let timestamp = message.timestamp.format("[%H:%M:%S]");
        
        // Add formatted message
        if !message_text.is_empty() {
            message_text.push_str("\n\n");
        }
        
        message_text.push_str(&format!("{} {}\n", timestamp, header));
        message_text.push_str(&message.content);
    }
    
    // Create styled spans for each line
    let message_lines = message_text.lines()
        .map(|line| {
            // Apply styling based on content
            if line.contains("You:") {
                Line::from(Span::styled(line, Style::default().fg(Color::Yellow)))
            } else if line.contains("Assistant:") {
                Line::from(Span::styled(line, Style::default().fg(Color::Green)))
            } else if line.contains("System:") {
                Line::from(Span::styled(line, Style::default().fg(Color::Blue)))
            } else if line.contains("Error:") {
                Line::from(Span::styled(line, Style::default().fg(Color::Red)))
            } else if line.contains("Tool:") {
                Line::from(Span::styled(line, Style::default().fg(Color::Magenta)))
            } else if line.contains("[") && line.contains("]") {
                // Timestamp styling
                let time_end = line.find(']').unwrap_or(0) + 1;
                let timestamp = &line[0..time_end];
                let rest = &line[time_end..];
                
                Line::from(vec![
                    Span::styled(timestamp, Style::default().fg(Color::DarkGray)),
                    Span::raw(rest),
                ])
            } else {
                Line::from(line)
            }
        })
        .collect::<Vec<Line>>();
    
    // Create messages text
    let message_paragraph = Paragraph::new(Text::from(message_lines))
        .block(messages_block)
        .wrap(ratatui::widgets::Wrap { trim: true })
        .scroll((0, 0)); // No built-in scrolling - we handle it ourselves
    
    // Render messages
    f.render_widget(message_paragraph, chunks[0]);
    
    // Create scrollbar
    let scrollbar_state = ScrollbarState::default()
        .content_length(state.messages.len().max(1))
        .position(state.view_start_index);
    
    // Render scrollbar
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .symbols(VERTICAL)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None),
        chunks[0].inner(Margin { vertical: 1, horizontal: 0 }),
        &mut scrollbar_state.clone(),
    );
    
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
    // Setup terminal with error handling
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