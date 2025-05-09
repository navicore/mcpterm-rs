// A test focused specifically on key event handling and focus management
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode, KeyEventKind},
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

enum FocusArea {
    Top,
    Bottom,
}

struct AppState {
    focus: FocusArea,
    top_content: String,
    bottom_content: String,
    last_key: String,
    running: bool,
}

impl AppState {
    fn new() -> Self {
        Self {
            focus: FocusArea::Top,
            top_content: "This is the top panel. Press j/k to navigate when focused.".to_string(),
            bottom_content: "This is the bottom panel. Press i to enter text mode.".to_string(),
            last_key: "No key pressed yet".to_string(),
            running: true,
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        // Always update last key display for debugging
        self.last_key = format!("{:?}", key);
        
        // Handle Tab key separately - always toggles focus
        if key.code == KeyCode::Tab {
            println!("TAB key detected - toggling focus");
            self.focus = match self.focus {
                FocusArea::Top => FocusArea::Bottom,
                FocusArea::Bottom => FocusArea::Top,
            };
            return;
        }
        
        // Handle global keys (work in any focus state)
        match key.code {
            KeyCode::Char('q') => {
                self.running = false;
            }
            _ => {}
        }
        
        // Handle focus-specific keys
        match self.focus {
            FocusArea::Top => {
                match key.code {
                    KeyCode::Char('j') => {
                        self.top_content = format!("{}\nMoved down in top panel", self.top_content);
                    }
                    KeyCode::Char('k') => {
                        self.top_content = format!("{}\nMoved up in top panel", self.top_content);
                    }
                    _ => {}
                }
            }
            FocusArea::Bottom => {
                match key.code {
                    KeyCode::Char(c) => {
                        self.bottom_content.push(c);
                    }
                    KeyCode::Backspace => {
                        self.bottom_content.pop();
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, state: &AppState) {
    // Create layout with two equal panels
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(f.area());

    // Create top panel (message area)
    let top_block = Block::default()
        .title("Message Area")
        .borders(Borders::ALL)
        .border_style(match state.focus {
            FocusArea::Top => Style::default().fg(Color::Green),
            _ => Style::default(),
        });

    let top_paragraph = Paragraph::new(state.top_content.clone())
        .block(top_block);

    f.render_widget(top_paragraph, chunks[0]);

    // Create bottom panel (input area)
    let bottom_block = Block::default()
        .title("Input Area")
        .borders(Borders::ALL)
        .border_style(match state.focus {
            FocusArea::Bottom => Style::default().fg(Color::Green),
            _ => Style::default(),
        });

    let bottom_text = format!("{}\nLast key: {}", state.bottom_content, state.last_key);
    let bottom_paragraph = Paragraph::new(bottom_text)
        .block(bottom_block);

    f.render_widget(bottom_paragraph, chunks[1]);
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

    // Print instructions to console for visibility
    println!("Key Event Test");
    println!("=============");
    println!("- Press Tab to switch focus");
    println!("- Press j/k to navigate in top panel");
    println!("- Type characters in bottom panel");
    println!("- Press q to quit");
    println!("Starting test...");

    // Main loop
    while state.running {
        // Draw UI
        terminal.draw(|f| ui(f, &state))?;

        // Handle input with a short timeout to avoid blocking
        if event::poll(Duration::from_millis(100))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                // Only process key press events (not releases)
                if key.kind == KeyEventKind::Press {
                    // Handle the key in app state
                    state.handle_key(key);
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("Test finished.");
    Ok(())
}