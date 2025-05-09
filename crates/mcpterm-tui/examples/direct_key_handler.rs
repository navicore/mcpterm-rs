// A simplified implementation with direct key handling
// This approach bypasses the more complex event system to handle key events directly
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;
use std::time::Duration;

// Focus areas for our application
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

// Simple state for our application
struct AppState {
    messages: Vec<String>,
    input: String,
    focus: Focus,
    mode: EditorMode,
    running: bool,
    message_scroll: usize,
    last_key: String,
}

impl AppState {
    fn new() -> Self {
        Self {
            messages: vec![
                "Welcome to Direct Key Handler demo".to_string(),
                "Press Tab to switch focus".to_string(),
                "In Message area: j/k to scroll".to_string(),
                "In Input area: i to enter insert mode, Esc to return to normal mode".to_string(),
                "Press Enter in normal mode to send message".to_string(),
                "Press q in normal mode to quit".to_string(),
            ],
            input: String::new(),
            focus: Focus::Input,
            mode: EditorMode::Normal,
            running: true,
            message_scroll: 0,
            last_key: "None".to_string(),
        }
    }

    // Add a message to the message list
    fn add_message(&mut self, content: String) {
        self.messages.push(content);
        self.message_scroll = 0; // Reset scroll to show newest message
    }

    // Submit the current input
    fn submit_input(&mut self) {
        if !self.input.is_empty() {
            self.add_message(format!("> {}", self.input));
            self.add_message(format!("You entered: {}", self.input));
            self.input.clear();
        }
    }

    // Handle a key event
    fn handle_key(&mut self, key: KeyEvent) {
        // Debug info
        self.last_key = format!("{:?}", key);
        
        // ALWAYS handle Tab key first to switch focus
        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::Input => Focus::Messages,
                Focus::Messages => Focus::Input,
            };
            return;
        }

        // Global quit key
        if key.code == KeyCode::Char('q') && self.mode == EditorMode::Normal {
            self.running = false;
            return;
        }

        // Then handle based on focus area
        match self.focus {
            Focus::Messages => {
                match key.code {
                    KeyCode::Char('j') => {
                        if self.message_scroll < self.messages.len() {
                            self.message_scroll += 1;
                        }
                    },
                    KeyCode::Char('k') => {
                        if self.message_scroll > 0 {
                            self.message_scroll -= 1;
                        }
                    },
                    KeyCode::Enter => {
                        // Enter switches focus to input
                        self.focus = Focus::Input;
                    },
                    _ => {}
                }
            },
            Focus::Input => {
                // Mode-switching keys
                if key.code == KeyCode::Char('i') && self.mode == EditorMode::Normal {
                    self.mode = EditorMode::Insert;
                    return;
                }
                if key.code == KeyCode::Esc && self.mode == EditorMode::Insert {
                    self.mode = EditorMode::Normal;
                    return;
                }

                // Handle based on mode
                match self.mode {
                    EditorMode::Normal => {
                        match key.code {
                            KeyCode::Enter => {
                                self.submit_input();
                            },
                            _ => {}
                        }
                    },
                    EditorMode::Insert => {
                        match key.code {
                            KeyCode::Char(c) => {
                                self.input.push(c);
                            },
                            KeyCode::Backspace => {
                                self.input.pop();
                            },
                            KeyCode::Enter => {
                                self.submit_input();
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, state: &AppState) {
    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    // Calculate which messages to show based on scroll
    let messages_offset = state.message_scroll;
    let messages_to_show = if messages_offset >= state.messages.len() {
        &[]
    } else {
        &state.messages[0..state.messages.len() - messages_offset]
    };

    // Create message text
    let message_text = messages_to_show.join("\n");

    // Render messages panel
    let messages_block = Block::default()
        .title(format!("Messages (scroll: {})", state.message_scroll))
        .borders(Borders::ALL)
        .border_style(if state.focus == Focus::Messages {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });

    let messages_widget = Paragraph::new(message_text)
        .block(messages_block)
        .wrap(ratatui::widgets::Wrap { trim: true });

    f.render_widget(messages_widget, chunks[0]);

    // Render input panel
    let mode_str = match state.mode {
        EditorMode::Normal => "Normal",
        EditorMode::Insert => "Insert",
    };
    
    let input_block = Block::default()
        .title(format!("Input ({} Mode) | Last key: {}", mode_str, state.last_key))
        .borders(Borders::ALL)
        .border_style(if state.focus == Focus::Input {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });

    let input_widget = Paragraph::new(state.input.as_str())
        .block(input_block);

    f.render_widget(input_widget, chunks[1]);
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut state = AppState::new();

    // Main loop
    loop {
        // Render UI
        terminal.draw(|f| ui(f, &state))?;

        // Break if user requested exit
        if !state.running {
            break;
        }

        // Handle events with a short timeout to avoid CPU spinning
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Only respond to key press events (not releases or repeats)
                    if key.kind == KeyEventKind::Press {
                        state.handle_key(key);
                    }
                }
                Event::Resize(_, _) => {
                    // Just redraw on resize
                }
                _ => {}
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}