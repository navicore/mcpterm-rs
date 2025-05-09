// Step 2: Two panels with basic focus switching
// No complex components or modes yet, just basic layout and focus

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

// Simple application state
struct AppState {
    messages: Vec<String>,
    input: String,
    focus: Focus,
    running: bool,
}

impl AppState {
    fn new() -> Self {
        Self {
            messages: vec![
                "Basic Two-Panel Test".to_string(),
                "=================".to_string(),
                "Tab: Switch focus".to_string(),
                "Type in Input area".to_string(),
                "Enter: Submit message".to_string(),
                "q: Quit".to_string(),
            ],
            input: String::new(),
            focus: Focus::Input,
            running: true,
        }
    }
    
    // Add a message to the list
    fn add_message(&mut self, msg: String) {
        self.messages.push(msg);
        // Ensure we don't keep too many messages (simple truncation)
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
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
            KeyCode::Char('q') => {
                self.running = false;
                return;
            }
            _ => {}
        }
        
        // Handle focus-specific keys
        match self.focus {
            Focus::Messages => {
                // Navigation keys could go here
                // For now, we just support Enter to switch to input
                if key.code == KeyCode::Enter {
                    self.focus = Focus::Input;
                }
            }
            Focus::Input => {
                // Handle input keys
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
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(2, 0),
        crossterm::style::Print(" Messages ")
    )?;
    
    // Draw messages content
    let message_count = state.messages.len();
    let visible_messages = std::cmp::min(message_count, (message_height - 2) as usize);
    let start_idx = message_count.saturating_sub(visible_messages);
    
    for (i, message) in state.messages.iter().skip(start_idx).enumerate() {
        if i < (message_height - 2) as usize {
            execute!(
                io::stdout(),
                crossterm::cursor::MoveTo(1, (i + 1) as u16),
                crossterm::style::Print(message)
            )?;
        }
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
    
    // Draw input title
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(2, message_height),
        crossterm::style::Print(" Input ")
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
    
    // Display focused element
    let focus_text = format!("Focus: {:?}", state.focus);
    execute!(
        io::stdout(),
        crossterm::cursor::MoveTo(width - focus_text.len() as u16 - 2, height - 1),
        crossterm::style::Print(focus_text)
    )?;
    
    // Position cursor in input field if input is focused
    if state.focus == Focus::Input {
        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(1 + state.input.len() as u16, message_height + 1)
        )?;
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
    while state.running {
        // Render the UI
        render(&state)?;
        
        // Handle input with a short timeout to avoid CPU spinning
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only process press events (not releases or repeats)
                if key.kind == KeyEventKind::Press {
                    // Add debug message to verify key handling
                    state.add_message(format!("Key: {:?}", key));
                    
                    // Handle the key
                    state.handle_key(key);
                }
            }
        }
    }
    
    // Clean up
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    
    Ok(())
}