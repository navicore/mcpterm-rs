// Step 3: Two panels with focus switching and basic VI-style modes
// Minimal implementation with direct terminal handling

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
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

// Simple application state
struct AppState {
    messages: Vec<String>,
    input: String,
    focus: Focus,
    mode: EditorMode,
    running: bool,
    message_scroll: usize,
}

impl AppState {
    fn new() -> Self {
        Self {
            messages: vec![
                "Basic Two-Panel Test with Modes".to_string(),
                "=============================".to_string(),
                "Tab: Switch focus".to_string(),
                "i: Enter insert mode".to_string(), 
                "Esc: Return to normal mode".to_string(),
                "j/k: Scroll messages (when messages focused)".to_string(),
                "Enter: Submit message (in input area)".to_string(),
                "q: Quit (in normal mode)".to_string(),
            ],
            input: String::new(),
            focus: Focus::Input,
            mode: EditorMode::Normal,
            running: true,
            message_scroll: 0,
        }
    }
    
    // Add a message to the list
    fn add_message(&mut self, msg: String) {
        self.messages.push(msg);
        // Ensure we don't keep too many messages (simple truncation)
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
        // Reset scroll position to see the new message
        self.message_scroll = 0;
    }
    
    // Submit the current input
    fn submit_input(&mut self) {
        if !self.input.is_empty() {
            let input_text = self.input.clone();
            // Add user message
            self.add_message(format!("> {}", input_text));
            // Add a simulated response
            self.add_message(format!("Echo: {}", input_text));
            self.input.clear();
        }
    }
    
    // Handle a key event
    fn handle_key(&mut self, key: KeyEvent) {
        // Add debug message
        //self.add_message(format!("Key: {:?}, Focus: {:?}, Mode: {:?}", key, self.focus, self.mode));
        
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

// Render the UI
fn render(state: &AppState) -> Result<()> {
    // Get terminal size
    let (width, height) = crossterm::terminal::size()?;
    
    // Calculate panel heights (70% for messages, 30% for input)
    let message_height = (height as f32 * 0.7) as u16;
    let input_height = height - message_height;
    
    // Clear the screen
    execute!(
        io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0)
    )?;
    
    // Draw messages panel border
    let message_border_style = if state.focus == Focus::Messages {
        crossterm::style::Attribute::Bold
    } else {
        crossterm::style::Attribute::Reset
    };
    
    execute!(
        io::stdout(),
        crossterm::style::SetAttribute(message_border_style),
        crossterm::style::Print("┌"),
        crossterm::style::Print("─".repeat((width - 2) as usize)),
        crossterm::style::Print("┐"),
        crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)
    )?;
    
    // Draw messages title
    let scroll_info = if state.message_scroll > 0 {
        format!(" Messages (scroll: {}) ", state.message_scroll)
    } else {
        " Messages ".to_string()
    };
    
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(2, 0),
        crossterm::style::Print(scroll_info)
    )?;
    
    // Calculate which messages to show based on scroll
    let messages_offset = state.message_scroll;
    let visible_messages = std::cmp::min(
        if messages_offset >= state.messages.len() { 0 } else { state.messages.len() - messages_offset },
        (message_height - 2) as usize
    );
    let start_idx = state.messages.len().saturating_sub(visible_messages + messages_offset);
    
    // Draw messages content
    for (i, message) in state.messages.iter().skip(start_idx).take(visible_messages).enumerate() {
        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(1, (i + 1) as u16),
            crossterm::style::Print(message)
        )?;
    }
    
    // Draw messages panel bottom border
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(0, message_height - 1),
        crossterm::style::SetAttribute(message_border_style),
        crossterm::style::Print("└"),
        crossterm::style::Print("─".repeat((width - 2) as usize)),
        crossterm::style::Print("┘"),
        crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)
    )?;
    
    // Draw input panel top border
    let input_border_style = if state.focus == Focus::Input {
        crossterm::style::Attribute::Bold
    } else {
        crossterm::style::Attribute::Reset
    };
    
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(0, message_height),
        crossterm::style::SetAttribute(input_border_style),
        crossterm::style::Print("┌"),
        crossterm::style::Print("─".repeat((width - 2) as usize)),
        crossterm::style::Print("┐"),
        crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)
    )?;
    
    // Draw input title with mode info
    let mode_str = match state.mode {
        EditorMode::Normal => "Normal",
        EditorMode::Insert => "Insert",
    };
    
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(2, message_height),
        crossterm::style::Print(format!(" Input ({} Mode) ", mode_str))
    )?;
    
    // Draw input content
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(1, message_height + 1),
        crossterm::style::Print(&state.input)
    )?;
    
    // Draw input panel bottom border
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(0, height - 1),
        crossterm::style::SetAttribute(input_border_style),
        crossterm::style::Print("└"),
        crossterm::style::Print("─".repeat((width - 2) as usize)),
        crossterm::style::Print("┘"),
        crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)
    )?;
    
    // Show focus and mode info
    let status = format!("Focus: {:?} | Mode: {:?}", state.focus, state.mode);
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(width - status.len() as u16 - 2, height - 1),
        crossterm::style::Print(status)
    )?;
    
    // Position cursor in input field if input is focused
    if state.focus == Focus::Input {
        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(1 + state.input.len() as u16, message_height + 1)
        )?;
    } else {
        // Hide cursor when not in input field
        execute!(io::stdout(), crossterm::cursor::Hide)?;
    }
    
    io::stdout().flush()?;
    
    Ok(())
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    
    // Create app state
    let mut state = AppState::new();
    
    // Main loop
    let mut debug_counter = 0;
    
    while state.running {
        // Render the UI
        render(&state)?;
        
        // Handle input with a short timeout to avoid CPU spinning
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Only process press events (not releases or repeats)
                    if key.kind == KeyEventKind::Press {
                        // Debug info
                        debug_counter += 1;
                        // Handle the key
                        state.handle_key(key);
                        
                        // Add debug message every 10 keys
                        if debug_counter % 10 == 0 {
                            state.add_message(format!("Debug: {} keys processed", debug_counter));
                        }
                    }
                }
                Event::Resize(_, _) => {
                    // Just redraw on resize
                }
                _ => {}
            }
        }
    }
    
    // Clean up
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    
    Ok(())
}