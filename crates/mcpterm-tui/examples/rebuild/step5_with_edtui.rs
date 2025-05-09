// Step 5: Using ratatui and edtui for rendering while keeping direct keyboard handling
// This adds edtui for the input editor component

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
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
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
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
    Visual,
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
        }
    }
}

// Simple application state
struct AppState {
    messages: Vec<String>,
    input: String,
    focus: Focus,
    mode: EditorMode,
    running: bool,
    message_scroll: usize,
    last_key: String,  // For debugging
}

impl AppState {
    fn new() -> Self {
        Self {
            messages: vec![
                "Ratatui + Edtui UI with Direct Keyboard Handling".to_string(),
                "==========================================".to_string(),
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
            last_key: "None".to_string(),
        }
    }
    
    // Add a message to the list
    fn add_message(&mut self, msg: String) {
        self.messages.push(msg);
        // Reset scroll position to see the new message
        self.message_scroll = 0;
    }
    
    // Submit the current input
    fn submit_input(&mut self) {
        if !self.input.is_empty() {
            self.add_message(format!("> {}", self.input.clone()));
            self.input.clear();
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
                self.add_message(format!("Focus switched to {:?}", self.focus));
                return;
            }
            KeyCode::Esc => {
                // Escape always returns to normal mode regardless of focus
                self.mode = EditorMode::Normal;
                self.add_message("Switched to Normal mode".to_string());
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
                            self.add_message(format!("Scrolled down, offset: {}", self.message_scroll));
                        }
                    }
                    KeyCode::Char('k') => {
                        // Scroll up
                        if self.message_scroll < self.messages.len() {
                            self.message_scroll += 1;
                            self.add_message(format!("Scrolled up, offset: {}", self.message_scroll));
                        }
                    }
                    KeyCode::Enter => {
                        // Enter switches focus to input
                        self.focus = Focus::Input;
                        self.add_message("Focus switched to Input".to_string());
                    }
                    _ => {}
                }
            }
            Focus::Input => {
                // Handle mode switching
                if key.code == KeyCode::Char('i') && self.mode == EditorMode::Normal {
                    self.mode = EditorMode::Insert;
                    self.add_message("Switched to Insert mode".to_string());
                    return;
                }
                
                if key.code == KeyCode::Char('v') && self.mode == EditorMode::Normal {
                    self.mode = EditorMode::Visual;
                    self.add_message("Switched to Visual mode".to_string());
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
                    EditorMode::Visual => {
                        // Visual mode for selection (simplified here)
                        match key.code {
                            KeyCode::Char('y') => {
                                self.add_message(format!("Copied: {}", self.input));
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
        let mut view = EditorView::new(&mut state);
        
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

fn ui(f: &mut ratatui::Frame, state: &AppState) {
    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.size());
    
    // Determine message area border style based on focus
    let message_border_style = if state.focus == Focus::Messages {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    // Create messages block
    let messages_block = Block::default()
        .title(format!("Messages (scroll: {})", state.message_scroll))
        .borders(Borders::ALL)
        .border_style(message_border_style);
    
    // Calculate which messages to show based on scroll
    let messages_offset = state.message_scroll;
    let messages_to_show = if messages_offset >= state.messages.len() {
        &[]
    } else {
        &state.messages[0..state.messages.len() - messages_offset]
    };
    
    // Create a Text widget for messages with proper styling
    let message_items: Vec<Line> = messages_to_show
        .iter()
        .map(|m| {
            // Special handling for system messages vs user input
            if m.starts_with('>') {
                Line::from(Span::styled(m, Style::default().fg(Color::Yellow)))
            } else {
                Line::from(Span::raw(m))
            }
        })
        .collect();
    
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
        f.set_cursor(
            chunks[1].x + state.input.len() as u16 + 1, // +1 for the block border
            chunks[1].y + 1,                           // +1 for the block border
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
        // Render the UI
        terminal.draw(|f| ui(f, &state))?;
        
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
                    // Just redraw on resize, handled by terminal.draw()
                }
                _ => {}
            }
        }
    }
    
    // Clean up
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    Ok(())
}