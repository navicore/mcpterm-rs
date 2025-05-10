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
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};

// Import editor-related modules from our UI implementation
use crate::ui::input_editor::{HandleResult, InputEditor};
use crate::ui::message_viewer::{MessageViewer, HandleResult as MessageHandleResult};
use edtui::EditorMode as EdtuiMode;
use std::io;
use std::time::Duration;
use mcp_core::context::{ConversationContext, Message as CoreMessage};
use crate::state::{AppState, EditorMode, FocusArea, Message, MessageType, ProcessingStatus};

// Simple working state that tracks messages, input, and uses edtui for proper VI mode
struct SimpleApp {
    state: AppState,
    needs_redraw: bool,
    input_editor: InputEditor,
    message_viewer: MessageViewer,
}

impl SimpleApp {
    // Create a new simple app with basic initialization
    fn new() -> Self {
        let mut state = AppState::new();
        
        // Add welcome message
        state.add_welcome_message();
        
        // Create input editor - use our existing component that's configured properly
        let mut input_editor = InputEditor::new();
        input_editor.set_mode(EdtuiMode::Normal); // Start in normal mode
        
        // Create message viewer - use our existing component
        let mut message_viewer = MessageViewer::new();
        
        // Update message viewer with messages
        message_viewer.set_content(&state.messages);
        
        Self { 
            state,
            needs_redraw: true,
            input_editor,
            message_viewer,
        }
    }
    
    // Process a key event and return true if it was handled
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Set redraw flag by default when handling keys
        self.needs_redraw = true;
        
        // Handle global keys
        match key.code {
            // Quit with Ctrl+C anytime
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.running = false;
                return true;
            }
            
            // Tab always toggles focus
            KeyCode::Tab => {
                self.state.focus = match self.state.focus {
                    FocusArea::Messages => FocusArea::Input,
                    FocusArea::Input => FocusArea::Messages,
                };
                return true;
            }
            
            _ => {}
        }
        
        // Handle focus-specific keys using our UI components
        match self.state.focus {
            FocusArea::Input => {
                // First handle custom direct key handling for insert mode
                if self.state.editor_mode == EditorMode::Insert && matches!(key.code, KeyCode::Char(_)) {
                    if let KeyCode::Char(c) = key.code {
                        // Insert character at current cursor position
                        let cursor_pos = self.state.input_cursor;
                        
                        if cursor_pos >= self.state.input_content.len() {
                            // Append to the end
                            self.state.input_content.push(c);
                        } else {
                            // Insert in the middle
                            self.state.input_content.insert(cursor_pos, c);
                        }
                        
                        // Move cursor forward
                        self.state.input_cursor += 1;
                        
                        // Update the input editor to keep it in sync
                        self.input_editor.set_content(&self.state.input_content);
                        
                        return true;
                    }
                }
                
                // Handle backspace in insert mode
                if self.state.editor_mode == EditorMode::Insert && key.code == KeyCode::Backspace {
                    if self.state.input_cursor > 0 {
                        // Remove character before cursor
                        self.state.input_cursor -= 1;
                        self.state.input_content.remove(self.state.input_cursor);
                        
                        // Update the input editor to keep it in sync
                        self.input_editor.set_content(&self.state.input_content);
                        
                        return true;
                    }
                    return true;
                }
                
                // Handle delete in insert mode
                if self.state.editor_mode == EditorMode::Insert && key.code == KeyCode::Delete {
                    if self.state.input_cursor < self.state.input_content.len() {
                        self.state.input_content.remove(self.state.input_cursor);
                        
                        // Update the input editor to keep it in sync
                        self.input_editor.set_content(&self.state.input_content);
                        
                        return true;
                    }
                    return true;
                }
                
                // Handle cursor movement in insert mode
                if self.state.editor_mode == EditorMode::Insert {
                    match key.code {
                        KeyCode::Left if self.state.input_cursor > 0 => {
                            self.state.input_cursor -= 1;
                            return true;
                        }
                        KeyCode::Right if self.state.input_cursor < self.state.input_content.len() => {
                            self.state.input_cursor += 1;
                            return true;
                        }
                        KeyCode::Home => {
                            self.state.input_cursor = 0;
                            return true;
                        }
                        KeyCode::End => {
                            self.state.input_cursor = self.state.input_content.len();
                            return true;
                        }
                        _ => {}
                    }
                }
                
                // Handle mode switching
                if key.code == KeyCode::Esc {
                    self.state.editor_mode = EditorMode::Normal;
                    return true;
                } else if key.code == KeyCode::Char('i') && self.state.editor_mode == EditorMode::Normal {
                    self.state.editor_mode = EditorMode::Insert;
                    return true;
                }
                
                // Handle Enter in normal mode to submit
                if key.code == KeyCode::Enter && self.state.editor_mode == EditorMode::Normal {
                    // Submit the input
                    if !self.state.input_content.is_empty() {
                        if let Some(input) = self.state.submit_input() {
                            // Add user message
                            self.state.add_message(
                                input.clone(),
                                MessageType::User
                            );
                            
                            // Echo back the message for now
                            self.state.add_message(
                                format!("You entered: {}", input),
                                MessageType::System
                            );
                            
                            // Update the viewer with all messages
                            self.message_viewer.set_content(&self.state.messages);
                            
                            // Clear the input content and reset cursor
                            self.state.input_content.clear();
                            self.state.input_cursor = 0;
                            self.input_editor.clear();
                        }
                    }
                    return true;
                }
                
                // For all other keys, pass to the input editor component
                match self.input_editor.handle_key_event(key) {
                    HandleResult::Continue => {
                        // Sync our state with editor (though we might not use this)
                        let editor_content = self.input_editor.get_text();
                        if editor_content != self.state.input_content {
                            self.state.input_content = editor_content;
                            // We don't have direct access to cursor position from editor
                            self.state.input_cursor = self.state.input_content.len();
                        }
                        true
                    },
                    HandleResult::Submit(content) => {
                        // Update state content
                        self.state.input_content = content.clone();
                        
                        // Process submission
                        if !content.is_empty() {
                            if let Some(input) = self.state.submit_input() {
                                // Add user message
                                self.state.add_message(
                                    input.clone(),
                                    MessageType::User
                                );
                                
                                // Echo back the message for now
                                self.state.add_message(
                                    format!("You entered: {}", input),
                                    MessageType::System
                                );
                                
                                // Update the viewer with all messages
                                self.message_viewer.set_content(&self.state.messages);
                            }
                            
                            // Clear the editor and reset cursor
                            self.input_editor.clear();
                            self.state.input_cursor = 0;
                        }
                        
                        true
                    },
                    HandleResult::Abort => {
                        // Quit
                        self.state.running = false;
                        true
                    },
                    _ => false,
                }
            },
            
            FocusArea::Messages => {
                // Let the message viewer handle the key
                match self.message_viewer.handle_key_event(key) {
                    MessageHandleResult::Continue => {
                        // Just continue
                        true
                    },
                    MessageHandleResult::Copy(text) => {
                        // Show a system message with copy notification
                        let message = Message::new(
                            format!("Copied: {}", 
                                if text.len() > 50 { 
                                    format!("{}...", &text[..50]) 
                                } else { 
                                    text.clone() 
                                }
                            ),
                            MessageType::System
                        );
                        
                        // Add to the state
                        self.state.add_message(
                            format!("Copied: {}", text),
                            MessageType::System
                        );
                        
                        // Update the viewer with all messages
                        self.message_viewer.set_content(&self.state.messages);
                        
                        true
                    },
                    MessageHandleResult::Abort => {
                        // Switch focus to input 
                        self.state.focus = FocusArea::Input;
                        true
                    },
                }
            }
        }
    }
    
    // Draw the UI using basic widgets
    fn render(&mut self, f: &mut ratatui::Frame) {
        // Create layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(70),  // Messages
                Constraint::Percentage(30),  // Input
            ])
            .split(f.area());
        
        // Configure message viewer block
        let messages_title = format!("Messages [{}]", if self.state.auto_scroll { "AUTO" } else { "MANUAL" });
        let messages_block = Block::default()
            .title(messages_title)
            .borders(Borders::ALL)
            .border_style(if self.state.focus == FocusArea::Messages {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            });
        
        // Update the edtui state for the message viewer
        let message_text = self.state.messages.iter()
            .map(|m| {
                let prefix = match m.message_type {
                    MessageType::System => "System: ",
                    MessageType::User => "You: ",
                    MessageType::Assistant => "Assistant: ",
                    MessageType::Error => "Error: ",
                    MessageType::Tool => "Tool: ",
                };
                format!("{}{}", prefix, m.content)
            })
            .collect::<Vec<String>>()
            .join("\n");
        
        // Create and render a simple paragraph for messages
        let messages_widget = Paragraph::new(message_text)
            .block(messages_block)
            .wrap(Wrap { trim: true });
        
        f.render_widget(messages_widget, chunks[0]);
        
        // Configure input editor block and mode
        let mode_str = match self.state.editor_mode {
            EditorMode::Normal => "NORMAL",
            EditorMode::Insert => "INSERT",
            EditorMode::Visual => "VISUAL",
        };
        
        let input_block = Block::default()
            .title(format!("Input [{}]", mode_str))
            .borders(Borders::ALL)
            .border_style(if self.state.focus == FocusArea::Input {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            });
        
        // Create and render a paragraph for input
        let input_widget = Paragraph::new(self.state.input_content.clone())
            .block(input_block);
        
        f.render_widget(input_widget, chunks[1]);
        
        // Set cursor when input has focus (with a border offset of 1)
        if self.state.focus == FocusArea::Input {
            let cursor_x = chunks[1].x + 1 + self.state.input_cursor as u16;
            let cursor_y = chunks[1].y + 1;
            f.set_cursor_position((cursor_x, cursor_y));
        }
        
        // Reset redraw flag
        self.needs_redraw = false;
    }
}

/// Run the clean implementation
pub fn run_clean() -> Result<()> {
    run_clean_with_options(false)
}

/// Run the clean implementation with options
pub fn run_clean_with_options(no_mouse: bool) -> Result<()> {
    // Print startup message first
    println!("Running clean implementation with working scrolling...");
    
    // Setup terminal with more robust error handling
    let enable_result = enable_raw_mode();
    if let Err(e) = &enable_result {
        eprintln!("Failed to enable raw mode: {}", e);
        eprintln!("Error kind: {:?}", e.kind());
        // Continue anyway as some terminals might still work
    }
    
    let mut stdout = io::stdout();
    
    // Try enabling alternate screen without mouse capture first
    let screen_result = execute!(
        stdout, 
        EnterAlternateScreen
    );
    
    if let Err(e) = &screen_result {
        eprintln!("Failed to enter alternate screen: {}", e);
        eprintln!("Error kind: {:?}", e.kind());
        // Try to disable raw mode when we encounter an error
        let _ = disable_raw_mode();
        return Err(anyhow::anyhow!("Failed to enter alternate screen: {}", e));
    }
    
    // Only try to enable mouse capture if alternate screen succeeded and mouse is not disabled
    if screen_result.is_ok() && !no_mouse {
        // Try to enable mouse capture, but continue even if it fails
        let mouse_result = execute!(
            stdout,
            crossterm::event::EnableMouseCapture
        );
        
        // Log but don't fail if mouse capture fails
        if let Err(e) = &mouse_result {
            eprintln!("Warning: Failed to enable mouse capture: {}", e);
            eprintln!("Mouse scrolling may not work, but keyboard navigation will still function.");
        }
    } else if no_mouse {
        println!("Mouse capture disabled by user request.");
    }
    
    // Create terminal with backend
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to create terminal: {}", e);
            let _ = disable_raw_mode();
            return Err(anyhow::anyhow!("Failed to create terminal: {}", e));
        }
    };
    
    // Create app
    let mut app = SimpleApp::new();
    
    // Add introduction message
    app.state.add_message(
        "Welcome to the clean TUI implementation!".to_string(),
        MessageType::System
    );
    app.state.add_message(
        "Press Tab to switch focus, Esc for normal mode, i for insert mode".to_string(),
        MessageType::System
    );
    app.state.add_message(
        "Use j/k to scroll messages when message area has focus".to_string(),
        MessageType::System
    );
    
    // Initial draw
    terminal.draw(|f| app.render(f))?;
    
    // Main loop
    while app.state.running {
        // Only redraw when needed
        if app.needs_redraw {
            terminal.draw(|f| app.render(f))?;
            app.needs_redraw = false; // Reset flag after drawing
        }
        
        // Poll for events with a longer timeout to reduce CPU usage
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                // Handle keyboard events
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Handle the key
                    let handled = app.handle_key(key);
                    
                    // Only set redraw flag if something changed
                    if handled {
                        app.needs_redraw = true;
                    }
                },
                
                // Handle mouse events 
                Event::Mouse(mouse_event) => {
                    // For mouse events, just redraw the UI
                    // Our UI components don't directly handle mouse events yet
                    use crossterm::event::{MouseEventKind};
                    
                    // Simple scroll handling
                    match mouse_event.kind {
                        MouseEventKind::ScrollDown => {
                            if app.state.focus == FocusArea::Messages && app.state.messages_scroll > 0 {
                                // Scroll down - show newer messages
                                app.state.messages_scroll -= 1;
                                app.needs_redraw = true;
                            }
                        },
                        MouseEventKind::ScrollUp => {
                            if app.state.focus == FocusArea::Messages && app.state.messages_scroll < app.state.messages.len() {
                                // Scroll up - show older messages
                                app.state.messages_scroll += 1;
                                app.needs_redraw = true;
                            }
                        },
                        _ => {}
                    }
                },
                
                // Ignore other events
                _ => {}
            }
        }
    }
    
    // Clean up with more robust error handling
    // First try to show cursor
    let _ = terminal.show_cursor();
    
    // Try to disable mouse capture, but don't fail if it doesn't work
    let _ = execute!(
        terminal.backend_mut(), 
        crossterm::event::DisableMouseCapture
    );
    
    // Leave alternate screen
    let screen_result = execute!(
        terminal.backend_mut(), 
        LeaveAlternateScreen
    );
    
    if let Err(e) = &screen_result {
        eprintln!("Warning: Failed to leave alternate screen: {}", e);
    }
    
    // Disable raw mode last
    let mode_result = disable_raw_mode();
    if let Err(e) = &mode_result {
        eprintln!("Warning: Failed to disable raw mode: {}", e);
    }
    
    Ok(())
}