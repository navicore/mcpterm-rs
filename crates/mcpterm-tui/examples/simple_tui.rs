use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
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

struct App {
    input: String,
    messages: Vec<String>,
    running: bool,
    mode: Mode,
    cursor_position: usize,
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            messages: vec!["Welcome to Simple TUI".to_string()],
            running: true,
            mode: Mode::Normal,
            cursor_position: 0,
        }
    }

    fn handle_key_event(&mut self, key: event::KeyEvent) {
        match self.mode {
            Mode::Normal => match key.code {
                KeyCode::Char('i') => {
                    self.mode = Mode::Insert;
                }
                KeyCode::Char('q') => {
                    self.running = false;
                }
                KeyCode::Enter => {
                    if !self.input.is_empty() {
                        self.submit_message();
                    }
                }
                _ => {}
            },
            Mode::Insert => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                }
                KeyCode::Enter => {
                    if !self.input.is_empty() {
                        self.submit_message();
                    }
                }
                KeyCode::Char(c) => {
                    self.input.insert(self.cursor_position, c);
                    self.cursor_position += 1;
                }
                KeyCode::Backspace => {
                    if self.cursor_position > 0 {
                        self.cursor_position -= 1;
                        self.input.remove(self.cursor_position);
                    }
                }
                KeyCode::Left => {
                    if self.cursor_position > 0 {
                        self.cursor_position -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.cursor_position < self.input.len() {
                        self.cursor_position += 1;
                    }
                }
                _ => {}
            },
        }
    }

    fn submit_message(&mut self) {
        let message = format!("> {}", self.input);
        self.messages.push(message);
        self.input.clear();
        self.cursor_position = 0;
    }
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Main loop
    loop {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
                .split(size);

            // Render messages
            let messages_text = app
                .messages
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<&str>>()
                .join("\n");

            let messages_block = Block::default()
                .title("Messages")
                .borders(Borders::ALL);

            let messages_widget = Paragraph::new(messages_text)
                .block(messages_block)
                .wrap(ratatui::widgets::Wrap { trim: true });

            f.render_widget(messages_widget, chunks[0]);

            // Render input
            let input_title = match app.mode {
                Mode::Normal => "Input (Normal Mode - press 'i' to type)",
                Mode::Insert => "Input (Insert Mode - press Esc for normal mode)",
            };

            let input_block = Block::default()
                .title(input_title)
                .borders(Borders::ALL)
                .border_style(match app.mode {
                    Mode::Normal => Style::default(),
                    Mode::Insert => Style::default().fg(Color::Yellow),
                });

            let input = Paragraph::new(app.input.as_str())
                .block(input_block)
                .style(Style::default());

            f.render_widget(input, chunks[1]);

            // Show cursor in insert mode
            if app.mode == Mode::Insert {
                let x = chunks[1].x + app.cursor_position as u16 + 1;
                let y = chunks[1].y + 1;
                f.set_cursor_position((x, y));
            }
        })?;

        // Handle events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Skip key release events
                if key.kind == KeyEventKind::Press {
                    // Handle keys
                    app.handle_key_event(key);

                    // Exit loop if app is no longer running
                    if !app.running {
                        break;
                    }
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

    Ok(())
}