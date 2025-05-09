// A simplified implementation of direct key handling for terminal UI applications
// This is designed to replace the complex event system with a direct, single-level event loop

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
    Terminal,
};
use std::io;
use std::time::Duration;

// Focus areas for the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusArea {
    Messages,
    Input,
}

// Editor modes for input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    Normal,
    Insert,
    Visual,
}

// A simple application state
struct AppState {
    messages: Vec<String>,
    input: String,
    focus: FocusArea,
    editor_mode: EditorMode,
    running: bool,
    messages_scroll: usize,
}

impl AppState {
    fn new() -> Self {
        Self {
            messages: vec![
                "Welcome to Direct Key Handling Example".to_string(),
                "--------------------------------".to_string(),
                "Press TAB to switch focus".to_string(),
                "In message area: j/k to scroll messages".to_string(),
                "In input area: i to enter insert mode, ESC for normal mode".to_string(),
                "Press ENTER in normal mode to send message".to_string(),
                "Press q in normal mode to quit".to_string(),
            ],
            input: String::new(),
            focus: FocusArea::Input,
            editor_mode: EditorMode::Normal,
            running: true,
            messages_scroll: 0,
        }
    }

    // Add a message to the list
    fn add_message(&mut self, msg: String) {
        self.messages.push(msg);
        // Reset scroll position to see new message
        self.messages_scroll = 0;
    }

    // Submit the current input as a message
    fn submit_input(&mut self) {
        let input = std::mem::take(&mut self.input);
        if !input.trim().is_empty() {
            self.add_message(format!("You: {}", input));
            self.add_message(format!("Echo: {}", input));
        }
    }

    // Direct key handling
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        println!("Key: {:?}, Focus: {:?}, Mode: {:?}", key, self.focus, self.editor_mode);
        
        // STEP 1: Global keys that work regardless of focus or mode
        
        // Always handle tab key first for focus switching
        if key.code == KeyCode::Tab {
            println!("TAB detected - switching focus");
            self.focus = match self.focus {
                FocusArea::Input => FocusArea::Messages,
                FocusArea::Messages => FocusArea::Input,
            };
            return true;
        }
        
        // Handle global quit command
        if key.code == KeyCode::Char('q') && self.editor_mode == EditorMode::Normal {
            println!("QUIT detected");
            self.running = false;
            return true;
        }
        
        // STEP 2: Focus-specific key handling
        
        match self.focus {
            FocusArea::Messages => {
                // Message viewer controls
                match key.code {
                    KeyCode::Char('j') => {
                        println!("j key in Messages - scroll down");
                        if self.messages_scroll > 0 {
                            self.messages_scroll -= 1;
                        }
                        return true;
                    }
                    KeyCode::Char('k') => {
                        println!("k key in Messages - scroll up");
                        if self.messages_scroll < self.messages.len() {
                            self.messages_scroll += 1;
                        }
                        return true;
                    }
                    KeyCode::Enter => {
                        // Return focus to input area
                        println!("ENTER in Messages - switch focus to input");
                        self.focus = FocusArea::Input;
                        return true;
                    }
                    _ => {
                        // Any other key is ignored in message viewer
                        return false;
                    }
                }
            }
            
            FocusArea::Input => {
                // STEP 3: Mode switching keys
                
                // Handle mode switches first
                if key.code == KeyCode::Esc && self.editor_mode != EditorMode::Normal {
                    println!("ESC key in Input - switch to normal mode");
                    self.editor_mode = EditorMode::Normal;
                    return true;
                }
                
                if key.code == KeyCode::Char('i') && self.editor_mode == EditorMode::Normal {
                    println!("i key in normal mode - switch to insert mode");
                    self.editor_mode = EditorMode::Insert;
                    return true;
                }
                
                if key.code == KeyCode::Char('v') && self.editor_mode == EditorMode::Normal {
                    println!("v key in normal mode - switch to visual mode");
                    self.editor_mode = EditorMode::Visual;
                    return true;
                }
                
                // STEP 4: Mode-specific key handling
                
                match self.editor_mode {
                    EditorMode::Normal => {
                        // Normal mode commands
                        match key.code {
                            KeyCode::Enter => {
                                println!("ENTER in normal mode - submit input");
                                self.submit_input();
                                return true;
                            }
                            _ => return false,
                        }
                    }
                    
                    EditorMode::Insert => {
                        // Insert mode for text editing
                        match key.code {
                            KeyCode::Char(c) => {
                                println!("Character '{}' in insert mode", c);
                                self.input.push(c);
                                return true;
                            }
                            KeyCode::Backspace => {
                                println!("BACKSPACE in insert mode");
                                self.input.pop();
                                return true;
                            }
                            KeyCode::Enter => {
                                println!("ENTER in insert mode - submit input");
                                self.submit_input();
                                return true;
                            }
                            _ => return false,
                        }
                    }
                    
                    EditorMode::Visual => {
                        // Visual mode for selection (simplified here)
                        match key.code {
                            KeyCode::Char('y') => {
                                println!("y key in visual mode - 'yank' operation");
                                // In a real app, this would copy selected text
                                self.editor_mode = EditorMode::Normal;
                                return true;
                            }
                            _ => return false,
                        }
                    }
                }
            }
        }
    }
}

// A simplified message viewer component
struct MessageViewer<'a> {
    block: Option<Block<'a>>,
    messages: &'a [String],
    scroll: usize,
}

impl<'a> MessageViewer<'a> {
    fn new(messages: &'a [String], scroll: usize) -> Self {
        Self {
            block: None,
            messages,
            scroll,
        }
    }
    
    fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for MessageViewer<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        // Create block if specified
        let inner_area = if let Some(block) = self.block {
            let inner = block.inner(area);
            block.render(area, buf);
            inner
        } else {
            area
        };
        
        // Calculate which messages to show based on scroll
        let messages_offset = self.scroll;
        let messages_to_show = if messages_offset >= self.messages.len() {
            &[]
        } else {
            &self.messages[0..self.messages.len() - messages_offset]
        };
        
        // Render messages
        let mut y = inner_area.y;
        for msg in messages_to_show {
            if y >= inner_area.y + inner_area.height {
                break;
            }
            
            ratatui::widgets::Paragraph::new(msg.as_str())
                .render(Rect::new(inner_area.x, y, inner_area.width, 1), buf);
            
            y += 1;
        }
    }
}

// A simplified input editor component
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
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        // Create block if specified
        let inner_area = if let Some(block) = self.block {
            let inner = block.inner(area);
            block.render(area, buf);
            inner
        } else {
            area
        };
        
        // Render content
        let mode_str = match self.mode {
            EditorMode::Normal => "Normal",
            EditorMode::Insert => "Insert",
            EditorMode::Visual => "Visual",
        };
        
        let display_text = format!("{} | Mode: {}", self.content, mode_str);
        
        ratatui::widgets::Paragraph::new(display_text)
            .render(inner_area, buf);
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
    let mut state = AppState::new();
    
    // Main loop 
    while state.running {
        // STEP 1: Render UI
        terminal.draw(|f| {
            // Create layout
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(f.area());
            
            // Create messages block with focus indicator
            let messages_block = Block::default()
                .title(format!("Messages (scroll: {})", state.messages_scroll))
                .borders(Borders::ALL)
                .border_style(if state.focus == FocusArea::Messages {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                });
            
            // Create input block with focus indicator and mode info
            let input_block = Block::default()
                .title(format!("Input ({})", match state.editor_mode {
                    EditorMode::Normal => "Normal Mode - 'i' to insert, 'q' to quit",
                    EditorMode::Insert => "Insert Mode - ESC for normal mode",
                    EditorMode::Visual => "Visual Mode - ESC for normal mode",
                }))
                .borders(Borders::ALL)
                .border_style(if state.focus == FocusArea::Input {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                });
            
            // Render message viewer
            let message_viewer = MessageViewer::new(&state.messages, state.messages_scroll)
                .block(messages_block);
            f.render_widget(message_viewer, chunks[0]);
            
            // Render input editor
            let input_editor = InputEditor::new(&state.input, state.editor_mode)
                .block(input_block);
            f.render_widget(input_editor, chunks[1]);
        })?;
        
        // STEP 2: Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only process key press events (not key release or repeat)
                if key.kind == KeyEventKind::Press {
                    // Process key directly in state
                    state.handle_key(key);
                }
            }
        }
    }
    
    // Cleanup terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    
    Ok(())
}