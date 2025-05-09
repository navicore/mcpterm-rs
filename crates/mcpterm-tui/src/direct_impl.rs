use anyhow::Result;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::time::Duration;

struct StateParts {
    messages: Vec<String>,
    input: String,
    cursor_pos: usize,
    running: bool,
    insert_mode: bool,
}

impl StateParts {
    fn new() -> Self {
        Self {
            messages: vec!["Welcome to Direct TUI implementation".to_string()],
            input: String::new(),
            cursor_pos: 0,
            running: true,
            insert_mode: false,
        }
    }

    fn add_message(&mut self, msg: String) {
        self.messages.push(msg);
    }

    fn submit_input(&mut self) {
        if !self.input.is_empty() {
            self.add_message(format!("> {}", self.input));
            self.input.clear();
            self.cursor_pos = 0;
        }
    }
}

fn ui(f: &mut ratatui::Frame, state: &StateParts) {
    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    // Render messages panel
    let messages_string = state.messages.join("\n");
    let messages_title = "Messages";
    let messages_block = Block::default()
        .title(messages_title)
        .borders(Borders::ALL);

    let messages_widget = Paragraph::new(messages_string)
        .block(messages_block)
        .wrap(ratatui::widgets::Wrap { trim: true });

    f.render_widget(messages_widget, chunks[0]);

    // Render input panel
    let input_title = if state.insert_mode {
        "Input (Insert Mode - press Esc for normal mode)"
    } else {
        "Input (Normal Mode - press 'i' to type)"
    };

    let input_block = Block::default()
        .title(input_title)
        .borders(Borders::ALL)
        .border_style(if state.insert_mode {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let input_widget = Paragraph::new(state.input.as_str())
        .block(input_block)
        .style(Style::default());

    f.render_widget(input_widget, chunks[1]);

    // Show cursor in insert mode
    if state.insert_mode {
        let x = chunks[1].x + state.cursor_pos as u16 + 1;
        let y = chunks[1].y + 1;
        f.set_cursor_position((x, y));
    }
}

pub fn run_direct() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut state = StateParts::new();

    // Main loop
    loop {
        // Render UI
        terminal.draw(|f| ui(f, &state))?;

        // Handle events
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Only respond to key press events
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            // Universal keys (work in any mode)
                            KeyCode::Char('q') if !state.insert_mode => {
                                state.running = false;
                                break;
                            }
                            
                            // Mode-specific keys
                            KeyCode::Char('i') if !state.insert_mode => {
                                state.insert_mode = true;
                            }
                            KeyCode::Esc if state.insert_mode => {
                                state.insert_mode = false;
                            }
                            
                            // Insert mode keys
                            KeyCode::Char(c) if state.insert_mode => {
                                state.input.insert(state.cursor_pos, c);
                                state.cursor_pos += 1;
                            }
                            KeyCode::Backspace if state.insert_mode => {
                                if state.cursor_pos > 0 {
                                    state.cursor_pos -= 1;
                                    state.input.remove(state.cursor_pos);
                                }
                            }
                            KeyCode::Left if state.insert_mode => {
                                if state.cursor_pos > 0 {
                                    state.cursor_pos -= 1;
                                }
                            }
                            KeyCode::Right if state.insert_mode => {
                                if state.cursor_pos < state.input.len() {
                                    state.cursor_pos += 1;
                                }
                            }
                            KeyCode::Enter => {
                                state.submit_input();
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        // Check for exit
        if !state.running {
            break;
        }
    }

    // Clean up properly
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}