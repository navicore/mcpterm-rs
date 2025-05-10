use crate::state::{AppState, EditorMode, FocusArea, MessageType, ProcessingStatus};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;
use std::time::Duration;

/// Ultra-simple implementation using internal state
/// This implementation is kept for reference and testing
pub fn run_direct() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create simple state
    let mut state = StateParts::new();
    
    // Draw once initially
    terminal.draw(|f| simple_ui(f, &state))?;

    // Main loop - as simple as possible
    'main: loop {
        // Handle events with a longer timeout to avoid CPU spinning
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                // Only process key press events
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        // Universal keys (work in any mode)
                        KeyCode::Char('q') if !state.insert_mode => {
                            break 'main;
                        }
                        
                        // Mode switching
                        KeyCode::Char('i') if !state.insert_mode => {
                            state.insert_mode = true;
                        }
                        KeyCode::Esc if state.insert_mode => {
                            state.insert_mode = false;
                        }
                        
                        // Input handling in insert mode
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
                        KeyCode::Enter => {
                            state.submit_input();
                        }
                        _ => {}
                    }
                    
                    // Only redraw after processing the key
                    terminal.draw(|f| simple_ui(f, &state))?;
                }
            }
        }
    }

    // Clean up properly
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

/// Simple state for the direct implementation
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

/// Ultra-simple UI renderer for basic testing
fn simple_ui(f: &mut ratatui::Frame, state: &StateParts) {
    // Create layout - just two basic panes
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    // Render messages as simple block of text
    let messages_string = state.messages.join("\n");
    let messages_widget = Paragraph::new(messages_string)
        .block(Block::default().title("Messages").borders(Borders::ALL));
    f.render_widget(messages_widget, chunks[0]);

    // Render input with current mode indicator
    let input_title = if state.insert_mode { "Insert Mode" } else { "Normal Mode" };
    let input_widget = Paragraph::new(state.input.as_str())
        .block(Block::default().title(input_title).borders(Borders::ALL));
    f.render_widget(input_widget, chunks[1]);

    // Always set cursor position in input area
    let x = chunks[1].x + state.cursor_pos as u16 + 1;
    let y = chunks[1].y + 1;
    f.set_cursor_position((x, y));
}

/// Render the UI with direct handling
/// This is a simplified version that focuses on reliable key handling and uses the main AppState
///
/// This function is the main entry point for the direct implementation mode that bypasses
/// the complex event system to provide more reliable keyboard input handling.
pub fn run_direct_ui() -> Result<()> {
    // Setup terminal - do this only once
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Create app state
    let mut state = AppState::new();
    
    // Start in normal mode - more consistent with vi behavior
    state.editor_mode = EditorMode::Normal;
    
    // Add welcome message
    state.add_welcome_message();
    
    // Add help message
    state.add_message(
        "Running in direct mode with improved keyboard handling.".to_string(),
        MessageType::System,
    );
    
    // Draw once initially
    terminal.draw(|f| render_ui(f, &mut state))?;
    
    // Main loop - simplified to be more like the working example
    'main: loop {
        // Only poll for events with a long timeout to avoid CPU spinning
        if event::poll(Duration::from_millis(250))? {
            // Only process key events
            if let Event::Key(key) = event::read()? {
                // Skip everything except press events
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                
                // Global quit handling
                if (key.code == KeyCode::Char('q') && state.editor_mode == EditorMode::Normal) || 
                   (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)) {
                    break 'main;
                }
                
                // Escape always goes to normal mode
                if key.code == KeyCode::Esc {
                    state.editor_mode = EditorMode::Normal;
                }
                // Tab always toggles focus
                else if key.code == KeyCode::Tab {
                    state.focus = match state.focus {
                        FocusArea::Messages => FocusArea::Input,
                        FocusArea::Input => FocusArea::Messages,
                    };
                } 
                // Handle 'i' in normal mode to enter insert mode
                else if key.code == KeyCode::Char('i') && state.editor_mode == EditorMode::Normal {
                    state.editor_mode = EditorMode::Insert;
                }
                // Handle Enter for submission
                else if key.code == KeyCode::Enter {
                    if state.focus == FocusArea::Input {
                        if !state.input_content.is_empty() {
                            if let Some(_input) = state.submit_input() {
                                state.add_message(
                                    "Direct mode echo response.".to_string(),
                                    MessageType::System
                                );
                                state.processing = ProcessingStatus::Idle;
                            }
                        }
                    } else {
                        state.focus = FocusArea::Input;
                    }
                }
                // Message scrolling
                else if state.focus == FocusArea::Messages {
                    match key.code {
                        KeyCode::Char('j') => {
                            if state.messages_scroll > 0 {
                                state.messages_scroll -= 1;
                            }
                        }
                        KeyCode::Char('k') => {
                            if state.messages_scroll < state.messages.len() {
                                state.messages_scroll += 1;
                            }
                        }
                        KeyCode::Char('a') => {
                            state.toggle_auto_scroll();
                        }
                        _ => {}
                    }
                }
                // Text input
                else if state.focus == FocusArea::Input && state.editor_mode == EditorMode::Insert {
                    match key.code {
                        KeyCode::Char(c) => {
                            state.input_content.insert(state.input_cursor, c);
                            state.input_cursor += 1;
                        }
                        KeyCode::Backspace => {
                            if state.input_cursor > 0 {
                                state.input_cursor -= 1;
                                state.input_content.remove(state.input_cursor);
                            }
                        }
                        KeyCode::Left => {
                            if state.input_cursor > 0 {
                                state.input_cursor -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if state.input_cursor < state.input_content.len() {
                                state.input_cursor += 1;
                            }
                        }
                        _ => {}
                    }
                }
                
                // Redraw after handling each key
                terminal.draw(|f| render_ui(f, &mut state))?;
            }
        }
        
        // Check for exit condition
        if !state.running {
            break;
        }
    }
    
    // Clean up
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    Ok(())
}

/// Direct key handling that bypasses the complex event system
/// Note: Tab and Enter are specially handled in the main loop for reliability
fn handle_key(state: &mut AppState, key: KeyEvent) {
    // Handle global keys first
    match key.code {
        KeyCode::Esc => {
            // Escape always returns to normal mode regardless of focus
            state.editor_mode = EditorMode::Normal;
            return;
        }
        KeyCode::Char('q') if state.editor_mode == EditorMode::Normal => {
            state.running = false;
            return;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.running = false;
            return;
        }
        _ => {}
    }

    // Handle focus-specific keys
    match state.focus {
        FocusArea::Messages => {
            // Message viewer controls
            match key.code {
                KeyCode::Char('j') => {
                    // Scroll down (show newer messages)
                    if state.messages_scroll > 0 {
                        state.messages_scroll -= 1;
                    }
                }
                KeyCode::Char('k') => {
                    // Scroll up (show older messages)
                    if state.messages_scroll < state.messages.len() {
                        state.messages_scroll += 1;
                    }
                }
                KeyCode::Char('g') => {
                    // Go to top (oldest messages)
                    state.messages_scroll = state.messages.len().saturating_sub(1);
                }
                KeyCode::Char('G') => {
                    // Go to bottom (newest messages)
                    state.messages_scroll = 0;
                }
                KeyCode::Char('a') => {
                    // Toggle auto-scroll
                    state.toggle_auto_scroll();
                    state.add_message(
                        format!(
                            "Auto-scroll {}",
                            if state.auto_scroll {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        ),
                        MessageType::System,
                    );
                }
                _ => {}
            }
        }

        FocusArea::Input => {
            // Handle mode switching
            if key.code == KeyCode::Char('i') && state.editor_mode == EditorMode::Normal {
                state.editor_mode = EditorMode::Insert;
                return;
            }

            // Handle mode-specific keys
            match state.editor_mode {
                EditorMode::Normal => {
                    // Normal mode commands
                    if key.code == KeyCode::Enter {
                        // Submit input using the built-in method
                        if !state.input_content.is_empty() {
                            // Use the built-in submit_input method which properly adds to history
                            // and sets the processing status
                            if let Some(_input) = state.submit_input() {
                                // In direct mode without event system, we don't have async integration
                                // Just simulate a response for now
                                state.add_message(
                                    "Direct mode doesn't support actual LLM integration yet. To use with the LLM, run without --direct-mode.".to_string(),
                                    MessageType::System
                                );

                                // Reset processing status
                                state.processing = ProcessingStatus::Idle;
                            }
                        }
                    }
                }
                EditorMode::Insert => {
                    // Insert mode for text editing
                    match key.code {
                        KeyCode::Char(c) => {
                            // Insert character at cursor
                            state.input_content.insert(state.input_cursor, c);
                            state.input_cursor += 1;
                        }
                        KeyCode::Backspace => {
                            // Delete character before cursor
                            if state.input_cursor > 0 {
                                state.input_cursor -= 1;
                                state.input_content.remove(state.input_cursor);
                            }
                        }
                        KeyCode::Delete => {
                            // Delete character at cursor
                            if state.input_cursor < state.input_content.len() {
                                state.input_content.remove(state.input_cursor);
                            }
                        }
                        KeyCode::Left => {
                            // Move cursor left
                            if state.input_cursor > 0 {
                                state.input_cursor -= 1;
                            }
                        }
                        KeyCode::Right => {
                            // Move cursor right
                            if state.input_cursor < state.input_content.len() {
                                state.input_cursor += 1;
                            }
                        }
                        KeyCode::Home => {
                            // Move cursor to start
                            state.input_cursor = 0;
                        }
                        KeyCode::End => {
                            // Move cursor to end
                            state.input_cursor = state.input_content.len();
                        }
                        KeyCode::Enter => {
                            // Submit input
                            if !state.input_content.is_empty() {
                                // Use the built-in submit_input method which properly adds to history
                                // and sets the processing status
                                if let Some(_input) = state.submit_input() {
                                    // In direct mode without event system, we don't have async integration
                                    // Just simulate a response for now
                                    state.add_message(
                                        "Direct mode doesn't support actual LLM integration yet. To use with the LLM, run without --direct-mode.".to_string(),
                                        MessageType::System
                                    );

                                    // Reset processing status
                                    state.processing = ProcessingStatus::Idle;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {} // Other modes not implemented in this simplified version
            }
        }
    }
}

/// Render the UI - simplified for reliability
fn render_ui(f: &mut ratatui::Frame, state: &mut AppState) {
    // Create a simple vertical layout with just messages and input
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(70), // Messages
            Constraint::Percentage(30), // Input
        ])
        .split(f.area());

    // Render messages - minimal implementation
    render_messages(f, state, chunks[0]);
    
    // Render input editor
    render_input(f, state, chunks[1]);
}

/// Render messages - simplified for reliability
fn render_messages(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    // Create a block with borders
    let block = Block::default()
        .title("Messages")
        .borders(Borders::ALL)
        .border_style(if state.focus == FocusArea::Messages {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });

    // Calculate which messages to show based on scroll
    let messages_offset = state.messages_scroll;
    let messages_to_show = if messages_offset >= state.messages.len() {
        &[]
    } else {
        &state.messages[0..state.messages.len() - messages_offset]
    };

    // Convert messages to simple strings to avoid complex rendering
    let message_lines: Vec<String> = messages_to_show
        .iter()
        .map(|m| {
            // Basic formatting
            let prefix = match m.message_type {
                MessageType::System => "System: ",
                MessageType::User => "You: ",
                MessageType::Assistant => "Assistant: ",
                MessageType::Error => "Error: ",
                MessageType::Tool => "Tool: ",
            };
            
            format!("{}{}", prefix, m.content)
        })
        .collect();
    
    // Join as a simple text block
    let messages_text = message_lines.join("\n");

    // Create messages paragraph with very simple styling
    let messages_widget = Paragraph::new(messages_text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });

    // Render messages widget
    f.render_widget(messages_widget, area);
}

/// Render input - simplified for reliability
fn render_input(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    // Create a block with borders for input
    let mode_str = match state.editor_mode {
        EditorMode::Normal => "Normal Mode",
        EditorMode::Insert => "Insert Mode",
        EditorMode::Visual => "Visual Mode",
    };

    let block = Block::default()
        .title(format!("Input ({})", mode_str))
        .borders(Borders::ALL)
        .border_style(if state.focus == FocusArea::Input {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });

    // Create a paragraph widget for the input
    let input_widget = Paragraph::new(state.input_content.as_str())
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });

    // Render input widget
    f.render_widget(input_widget, area);

    // Position cursor in input field if input is focused
    if state.focus == FocusArea::Input {
        // Calculate cursor position
        let cursor_x = area.x + 1 + state.input_cursor as u16; // +1 for the border
        let cursor_y = area.y + 1; // +1 for the border

        // Set cursor position
        f.set_cursor_position((cursor_x, cursor_y));
    }
}
