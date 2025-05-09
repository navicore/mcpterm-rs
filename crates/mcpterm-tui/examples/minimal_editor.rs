use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use edtui::{EditorMode, EditorState, EditorView};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders, Widget},
    Terminal,
};
use std::io;

struct App {
    editor_state: EditorState,
    input: String,
    running: bool,
}

impl App {
    fn new() -> Self {
        Self {
            editor_state: EditorState::default(),
            input: String::new(),
            running: true,
        }
    }

    fn handle_input(&mut self, event: Event) -> Result<()> {
        if let Event::Key(key) = event {
            // Only process key press events
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }

            // Convert from crossterm to ratatui
            let ratatui_key = ratatui::crossterm::event::KeyEvent {
                code: match key.code {
                    KeyCode::Backspace => ratatui::crossterm::event::KeyCode::Backspace,
                    KeyCode::Enter => ratatui::crossterm::event::KeyCode::Enter,
                    KeyCode::Left => ratatui::crossterm::event::KeyCode::Left,
                    KeyCode::Right => ratatui::crossterm::event::KeyCode::Right,
                    KeyCode::Up => ratatui::crossterm::event::KeyCode::Up,
                    KeyCode::Down => ratatui::crossterm::event::KeyCode::Down,
                    KeyCode::Home => ratatui::crossterm::event::KeyCode::Home,
                    KeyCode::End => ratatui::crossterm::event::KeyCode::End,
                    KeyCode::PageUp => ratatui::crossterm::event::KeyCode::PageUp,
                    KeyCode::PageDown => ratatui::crossterm::event::KeyCode::PageDown,
                    KeyCode::Tab => ratatui::crossterm::event::KeyCode::Tab,
                    KeyCode::BackTab => ratatui::crossterm::event::KeyCode::BackTab,
                    KeyCode::Delete => ratatui::crossterm::event::KeyCode::Delete,
                    KeyCode::Insert => ratatui::crossterm::event::KeyCode::Insert,
                    KeyCode::F(n) => ratatui::crossterm::event::KeyCode::F(n),
                    KeyCode::Char(c) => ratatui::crossterm::event::KeyCode::Char(c),
                    KeyCode::Null => ratatui::crossterm::event::KeyCode::Null,
                    KeyCode::Esc => ratatui::crossterm::event::KeyCode::Esc,
                    _ => ratatui::crossterm::event::KeyCode::Null,
                },
                modifiers: ratatui::crossterm::event::KeyModifiers::from_bits_truncate(key.modifiers.bits()),
                kind: ratatui::crossterm::event::KeyEventKind::Press,
                state: ratatui::crossterm::event::KeyEventState::NONE,
            };

            // Special case handling
            match key.code {
                // Quit on Ctrl+C or 'q'
                KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    self.running = false;
                    return Ok(());
                }
                KeyCode::Char('q') => {
                    self.running = false;
                    return Ok(());
                }

                // Toggle between normal and insert mode with Esc and 'i'
                KeyCode::Esc => {
                    self.editor_state.mode = EditorMode::Normal;
                    return Ok(());
                }
                KeyCode::Char('i') if self.editor_state.mode == EditorMode::Normal => {
                    self.editor_state.mode = EditorMode::Insert;
                    return Ok(());
                }

                // Direct character handling in insert mode
                KeyCode::Char(c) if self.editor_state.mode == EditorMode::Insert => {
                    // Get current cursor position
                    let col = self.editor_state.cursor.col;
                    let current_content = self.input.clone();

                    // Insert character
                    if col >= current_content.len() {
                        self.input.push(c);
                    } else {
                        self.input.insert(col, c);
                    }

                    // Move cursor forward
                    self.editor_state.cursor.col += 1;

                    // Update editor state
                    self.editor_state = EditorState::new(edtui::Lines::from(&self.input));
                    self.editor_state.cursor.col = col + 1;
                    return Ok(());
                }

                // Handle backspace in insert mode
                KeyCode::Backspace if self.editor_state.mode == EditorMode::Insert => {
                    let col = self.editor_state.cursor.col;
                    if col > 0 && !self.input.is_empty() {
                        self.input.remove(col - 1);
                        self.editor_state = EditorState::new(edtui::Lines::from(&self.input));
                        self.editor_state.cursor.col = col - 1;
                    }
                    return Ok(());
                }

                _ => {}
            }

            // For normal navigation, let edtui handle it
            let mut event_handler = edtui::EditorEventHandler::default();
            event_handler.on_key_event(ratatui_key, &mut self.editor_state);

            // Sync input string with editor state after editing
            let lines = &self.editor_state.lines;
            let mut new_input = String::new();
            
            // Use edtui lines to string conversion
            for (i, row) in lines.iter_row().enumerate() {
                if i > 0 {
                    new_input.push('\n');
                }
                for c in row {
                    new_input.push(*c);
                }
            }
            self.input = new_input;
        }

        Ok(())
    }
}

// Implement Widget for a reference to App
impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        // Create editor view with current state
        let mut editor_state = self.editor_state.clone();
        let mut view = EditorView::new(&mut editor_state);

        // Set up theme
        let theme = edtui::EditorTheme::default()
            .block(Block::default().title("Input Editor").borders(Borders::ALL));

        view = view.theme(theme).wrap(true);

        // Render the view
        view.render(area, buf);
    }
}

fn main() -> Result<()> {
    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    loop {
        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Min(1)].as_ref())
                .split(f.area());

            // Render app
            f.render_widget(&app, chunks[0]);
        })?;

        // Handle input
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            let event = crossterm::event::read()?;
            app.handle_input(event)?;
        }

        // Check if we should exit
        if !app.running {
            break;
        }
    }

    // Clean up
    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}