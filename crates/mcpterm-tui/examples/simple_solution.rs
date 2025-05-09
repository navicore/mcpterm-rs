// A simple but reliable implementation focused on basic functionality
// Ensures that:
// 1. Input in the input field displays as you type
// 2. Messages scroll properly when new messages are added
// 3. UI components focus correctly with Tab

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;
use std::time::Duration;

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
}

// Message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageType {
    System,
    User,
    Response,
}

// A message in the conversation
#[derive(Debug, Clone)]
struct Message {
    content: String,
    message_type: MessageType,
}

impl Message {
    fn new(content: String, message_type: MessageType) -> Self {
        Self {
            content,
            message_type,
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
}

impl AppState {
    fn new() -> Self {
        // Initialize with some help messages
        let mut messages = vec![
            Message::new(
                "Terminal UI with simple scrolling".to_string(),
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
                "Enter: Submit message (in input area)".to_string(),
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
            scroll: 0,
            auto_scroll: true, // Auto-scroll by default
        }
    }
    
    // Add a message to the list
    fn add_message(&mut self, content: String, message_type: MessageType) {
        let message = Message::new(content, message_type);
        self.messages.push(message);
        
        // If auto-scroll is enabled, scroll to show the newest message
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }
    
    // Scroll to the bottom to show newest messages
    fn scroll_to_bottom(&mut self) {
        // Calculate how many messages can be displayed based on size
        // This is a very simple estimate that could be improved
        let estimated_visible_count = 10;
        
        // Calculate the ideal scroll position to show the most recent messages
        let max_scroll = self.messages.len().saturating_sub(estimated_visible_count);
        self.scroll = 0.max(max_scroll);
    }
    
    // Scroll to the top to show oldest messages
    fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }
    
    // Submit the current input
    fn submit_input(&mut self) {
        if !self.input.is_empty() {
            // Add user message
            self.add_message(format!("> {}", self.input.clone()), MessageType::User);
            
            // Add a simulated response
            self.add_message(format!("Echo: {}", self.input.clone()), MessageType::Response);
            
            // Clear the input
            self.input.clear();
        }
    }
    
    // Handle a key event
    fn handle_key(&mut self, key: KeyEvent) {
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
                        // This means moving the scroll "window" down
                        let max_scroll = self.messages.len().saturating_sub(1);
                        if self.scroll < max_scroll {
                            self.scroll += 1;
                        }
                    }
                    KeyCode::Char('k') => {
                        // Scroll up (show older messages)
                        // This means moving the scroll "window" up
                        if self.scroll > 0 {
                            self.scroll -= 1;
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
                    _ => {}
                }
            }
            
            Focus::Input => {
                // Handle mode switching
                if key.code == KeyCode::Char('i') && self.mode == EditorMode::Normal {
                    self.mode = EditorMode::Insert;
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
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, state: &AppState) {
    // Create a vertical layout with 70% for messages and 30% for input
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ])
        .split(f.area());
    
    // Determine message area border style based on focus
    let message_border_style = if state.focus == Focus::Messages {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    // Create a block for the messages area with a border
    let auto_scroll_indicator = if state.auto_scroll { "AUTO" } else { "MANUAL" };
    let messages_block = Block::default()
        .title(format!("Messages (scroll: {}/{}) - {}", 
                     state.scroll, 
                     state.messages.len().saturating_sub(1),
                     auto_scroll_indicator))
        .borders(Borders::ALL)
        .border_style(message_border_style);
    
    // Calculate which messages to show based on scroll
    let messages_to_show = if state.messages.is_empty() {
        &[]
    } else if state.scroll >= state.messages.len() {
        &[] // Safety check
    } else {
        // Show messages starting from the scroll index
        &state.messages[state.scroll..]
    };
    
    // Format messages with styling based on type
    let message_items: Vec<Line> = messages_to_show
        .iter()
        .map(|m| {
            let style = match m.message_type {
                MessageType::System => Style::default().fg(Color::Blue),
                MessageType::User => Style::default().fg(Color::Yellow),
                MessageType::Response => Style::default().fg(Color::Green),
            };
            
            Line::from(Span::styled(&m.content, style))
        })
        .collect();
    
    // Create a Text widget with all the message lines
    let messages_text = Text::from(message_items);
    
    // Create a paragraph widget for the messages
    let messages_paragraph = Paragraph::new(messages_text)
        .block(messages_block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    // Render the messages widget in the top chunk
    f.render_widget(messages_paragraph, chunks[0]);
    
    // Determine input area border style based on focus
    let input_border_style = if state.focus == Focus::Input {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    // Create a block for the input area with a border
    let input_block = Block::default()
        .title(format!("Input ({} mode)", if state.mode == EditorMode::Normal { "Normal" } else { "Insert" }))
        .borders(Borders::ALL)
        .border_style(input_border_style);
    
    // Create a paragraph widget for the input text
    let input_paragraph = Paragraph::new(state.input.clone())
        .block(input_block);
    
    // Render the input widget in the bottom chunk
    f.render_widget(input_paragraph, chunks[1]);
    
    // If in Insert mode and focused on input, position the cursor
    if state.focus == Focus::Input && state.mode == EditorMode::Insert {
        // Position the cursor at the end of the input text
        f.set_cursor(
            chunks[1].x + state.input.len() as u16 + 1, // +1 for the border
            chunks[1].y + 1, // +1 for the border
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
        // Draw the UI
        terminal.draw(|f| ui(f, &state))?;
        
        // Poll for events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only process key press events
                if key.kind == KeyEventKind::Press {
                    state.handle_key(key);
                }
            }
        }
    }
    
    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    Ok(())
}