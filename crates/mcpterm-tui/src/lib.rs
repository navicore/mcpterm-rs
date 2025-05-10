pub mod events;
pub mod state;
pub mod ui;
pub mod direct_impl;
pub mod clean_impl;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use events::{Event, EventHandler};
use mcp_metrics::{LogDestination, MetricsDestination, MetricsRegistry};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use state::{AppState, FocusArea, MessageType};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use ui::input_editor::HandleResult as EditorHandleResult;
use ui::message_viewer::HandleResult as ViewerHandleResult;

pub struct App {
    pub state: AppState,
    event_handler: EventHandler,
}

impl App {
    pub fn new() -> Result<Self> {
        let mut state = AppState::new();
        let event_handler = EventHandler::new()?;

        // Add welcome message
        state.add_welcome_message();

        Ok(Self {
            state,
            event_handler,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        let mut terminal = setup_terminal()?;

        // Setup metrics reporting every 2 minutes
        let log_destination = LogDestination;
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(120)).await; // Report every 2 minutes
                let report = MetricsRegistry::global().generate_report();
                info!("Generating metrics report (2-minute interval)");

                if let Err(e) = log_destination.send_report(&report) {
                    debug!("Error sending metrics report: {}", e);
                }

                // Reset counters after reporting
                MetricsRegistry::global().reset_counters();
                debug!("Counters reset after metrics report");
            }
        });

        // Run the main event loop
        self.run_event_loop(&mut terminal)?;

        // Restore terminal
        restore_terminal(&mut terminal)?;

        Ok(())
    }

    fn run_event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        info!("Starting TUI event loop");
        
        // Get a reference to the event handler
        let event_handler = Arc::new(std::mem::replace(&mut self.event_handler, events::EventHandler::new()?));
        
        // Store the viewers in the App struct directly instead of static variables
        let mut message_viewer = ui::message_viewer::MessageViewer::new();
        let mut input_editor = ui::input_editor::InputEditor::new();
        
        // Start in normal mode 
        message_viewer.set_mode(edtui::EditorMode::Normal);
        input_editor.set_mode(edtui::EditorMode::Normal);

        // Main event loop
        while self.state.running {
            // Render the UI
            terminal.draw(|f| {
                // Use the local message_viewer and input_editor instances
                ui::render_with_editors(f, &mut self.state, &mut message_viewer, &mut input_editor);
            })?;

            // Handle events
            match event_handler.next()? {
                Event::Input(key) => {
                    info!("------------ KEY EVENT -------------");
                    info!("Received key: {:?}", key);
                    info!("Current focus: {:?}", self.state.focus);
                    info!("Current editor mode: {:?}", self.state.editor_mode);
                    info!("Input content: {:?}", self.state.input_content);
                    
                    // Handle focus switching with Tab first - ALWAYS handle this separately
                    if key.code == crossterm::event::KeyCode::Tab {
                        info!("TAB KEY DETECTED - EXPLICIT FOCUS CHANGE");
                        // Always toggle focus regardless of other state
                        self.state.focus = match self.state.focus {
                            FocusArea::Input => {
                                info!("  Changing focus: Input -> Messages");
                                FocusArea::Messages
                            },
                            FocusArea::Messages => {
                                info!("  Changing focus: Messages -> Input");
                                FocusArea::Input
                            },
                        };
                        info!("Focus is now: {:?}", self.state.focus);
                        // Skip all other processing for Tab key
                        continue;
                    }
                    
                    // Handle keys based on focus area
                    match self.state.focus {
                        FocusArea::Messages => {
                            info!("HANDLING KEY IN MESSAGE VIEWER");
                            // Process keys specifically for navigation in message viewer
                            let result = match key.code {
                                crossterm::event::KeyCode::Char('j') => {
                                    info!("  Message viewer: 'j' key - move down");
                                    if self.state.messages_scroll > 0 {
                                        self.state.messages_scroll -= 1;
                                        info!("  Scrolled messages down, offset: {}", self.state.messages_scroll);
                                    }
                                    ViewerHandleResult::Continue
                                },
                                crossterm::event::KeyCode::Char('k') => {
                                    info!("  Message viewer: 'k' key - move up");
                                    if self.state.messages_scroll < self.state.messages.len() {
                                        self.state.messages_scroll += 1;
                                        info!("  Scrolled messages up, offset: {}", self.state.messages_scroll);
                                    }
                                    ViewerHandleResult::Continue
                                },
                                crossterm::event::KeyCode::Char('a') => {
                                    info!("  Message viewer: 'a' key - toggle auto-scroll");
                                    self.state.toggle_auto_scroll();
                                    self.state.add_message(
                                        format!("Auto-scroll {}", if self.state.auto_scroll { "enabled" } else { "disabled" }),
                                        MessageType::System,
                                    );
                                    ViewerHandleResult::Continue
                                },
                                // Other message viewer keys
                                _ => {
                                    info!("  Message viewer: passing key to editor component");
                                    // Pass to the editor component
                                    message_viewer.handle_key_event(key)
                                }
                            };
                            
                            // Process the result
                            match result {
                                ViewerHandleResult::Continue => {
                                    info!("  Message viewer: continue");
                                },
                                ViewerHandleResult::Copy(text) => {
                                    info!("  Message viewer: copied text");
                                    // Show a system message that text was copied
                                    self.state.add_message(
                                        format!("Copied to clipboard: {}", 
                                            if text.len() > 50 { 
                                                format!("{}...", &text[..50]) 
                                            } else { 
                                                text 
                                            }
                                        ),
                                        MessageType::System,
                                    );
                                },
                                ViewerHandleResult::Abort => {
                                    info!("  Message viewer: abort");
                                    self.state.running = false;
                                },
                            }
                        },
                        FocusArea::Input => {
                            info!("HANDLING KEY IN INPUT EDITOR");
                            
                            // Special case for ESC - always change to normal mode
                            if key.code == crossterm::event::KeyCode::Esc {
                                info!("  INPUT: ESC key - FORCE change to normal mode");
                                self.state.editor_mode = state::EditorMode::Normal;
                                input_editor.set_mode(edtui::EditorMode::Normal);
                                info!("  Editor mode set to normal");
                                continue;
                            }

                            // Handle 'i' in normal mode to enter insert mode
                            if key.code == crossterm::event::KeyCode::Char('i') && 
                               self.state.editor_mode == state::EditorMode::Normal {
                                info!("  INPUT: 'i' key in normal mode - FORCE change to insert mode");
                                self.state.editor_mode = state::EditorMode::Insert;
                                input_editor.set_mode(edtui::EditorMode::Insert);
                                info!("  Editor mode set to insert");
                                continue;
                            }
                            
                            // Handle 'q' in normal mode to quit
                            if key.code == crossterm::event::KeyCode::Char('q') && 
                               self.state.editor_mode == state::EditorMode::Normal {
                                info!("  INPUT: 'q' key in normal mode - quitting application");
                                self.state.running = false;
                                continue;
                            }
                            
                            // Handle direct character input in insert mode
                            if self.state.editor_mode == state::EditorMode::Insert && 
                               matches!(key.code, crossterm::event::KeyCode::Char(_)) {
                                if let crossterm::event::KeyCode::Char(c) = key.code {
                                    info!("  INPUT: Direct character input in insert mode: '{}'", c);
                                }
                            }
                            
                            // Process in input editor using our local instance
                            info!("  INPUT: Sending key to input_editor component");
                            match input_editor.handle_key_event(key) {
                                EditorHandleResult::Continue => {
                                    info!("  INPUT: Editor returned Continue");
                                    // Update state from editor state after every key
                                    let old_content = self.state.input_content.clone();
                                    self.state.input_content = input_editor.get_text();
                                    if old_content != self.state.input_content {
                                        info!("  INPUT: Content changed to: {:?}", self.state.input_content);
                                    }
                                },
                                EditorHandleResult::Submit(content) => {
                                    info!("  INPUT: Editor returned Submit with content: {:?}", content);
                                    // Get the editor content and update the state
                                    self.state.input_content = content;
                                    
                                    // Submit the input
                                    if let Some(input) = self.state.submit_input() {
                                        info!("  INPUT: Submitting message: {:?}", input);
                                        // Process in background
                                        if let Err(e) = events::EventHandler::process_message(
                                            event_handler.tx.clone(),
                                            event_handler.llm_client.is_some(),
                                            event_handler.pending_requests.clone(),
                                            input, 
                                            self.state.context.clone()
                                        ) {
                                            error!("Failed to process message: {}", e);
                                            self.state.add_message(
                                                format!("Error processing message: {}", e),
                                                MessageType::Error,
                                            );
                                        }
                                    }
                                    
                                    // Clear the editor
                                    info!("  INPUT: Clearing editor");
                                    input_editor.clear();
                                },
                                EditorHandleResult::PreviousHistory => {
                                    info!("  INPUT: Editor returned PreviousHistory");
                                    // Get previous history item
                                    if let Some(prev) = self.state.input_history.previous(&self.state.input_content) {
                                        info!("  INPUT: Previous history item: {:?}", prev);
                                        self.state.input_content = prev;
                                        // Update editor content
                                        input_editor.set_content(&self.state.input_content);
                                    } else {
                                        info!("  INPUT: No previous history available");
                                    }
                                },
                                EditorHandleResult::NextHistory => {
                                    info!("  INPUT: Editor returned NextHistory");
                                    // Get next history item
                                    if let Some(next) = self.state.input_history.next() {
                                        info!("  INPUT: Next history item: {:?}", next);
                                        self.state.input_content = next;
                                        // Update editor content
                                        input_editor.set_content(&self.state.input_content);
                                    } else {
                                        info!("  INPUT: No next history available");
                                    }
                                },
                                EditorHandleResult::Abort => {
                                    info!("  INPUT: Editor returned Abort - exiting application");
                                    self.state.running = false;
                                },
                            }
                        }
                    }
                },
                Event::Tick => {
                    // Update processing status if needed
                    if let Some((msg, msg_type)) = self.state.processing.as_display_message() {
                        // Update status message if changed
                        if self.state.messages.last().map(|m| &m.content) != Some(&msg) {
                            // Only add if it's a new status
                            self.state.add_message(msg, msg_type);
                        }
                    }
                },
                Event::LlmResponse(request, result) => {
                    // Process LLM response
                    self.state.process_llm_response(result);
                    debug!("Processed LLM response for request: {}", request);
                },
                Event::ToolResult(tool_id, result) => {
                    // Process tool result
                    match result {
                        Ok(output) => {
                            self.state.add_message(
                                format!("Tool result: {}", output),
                                MessageType::Tool,
                            );
                        },
                        Err(e) => {
                            self.state.add_message(
                                format!("Tool error: {}", e),
                                MessageType::Error,
                            );
                            self.state.error_count += 1;
                        },
                    }
                    
                    // Reset processing status
                    self.state.processing = state::ProcessingStatus::Idle;
                    debug!("Processed tool result for: {}", tool_id);
                },
                Event::StatusUpdate(id, status) => {
                    // Update status
                    let status_copy = status.clone();
                    self.state.update_processing_status(status);
                    debug!("Status update for {}: {}", id, status_copy);
                },
                Event::Quit => {
                    // Exit the application
                    info!("Received quit event");
                    self.state.running = false;
                },
            }
            
            // Synchronize state with input editor after event handling
            info!("------------ SYNC STATE -------------");
            
            // Sync content
            let content = input_editor.get_text();
            info!("Editor content: {:?}", content);
            info!("State content: {:?}", self.state.input_content);
            if content != self.state.input_content {
                info!("Content MISMATCH - Updating state from editor");
                self.state.input_content = content;
            }
            
            // Get current mode indirectly since we can't access the private field
            // We can infer the current mode from our state or the type of key response
            let current_editor_mode = match self.state.editor_mode {
                state::EditorMode::Normal => edtui::EditorMode::Normal,
                state::EditorMode::Insert => edtui::EditorMode::Insert,
                state::EditorMode::Visual => edtui::EditorMode::Visual,
            };
            info!("Inferred editor mode: {:?}", current_editor_mode);
            info!("State mode: {:?}", self.state.editor_mode);
            
            // Always set the mode to ensure consistency
            let expected_mode = match self.state.editor_mode {
                state::EditorMode::Normal => edtui::EditorMode::Normal,
                state::EditorMode::Insert => edtui::EditorMode::Insert,
                state::EditorMode::Visual => edtui::EditorMode::Visual,
            };
            
            // Always force the mode to match state
            info!("Setting editor mode to match state: {:?}", expected_mode);
            input_editor.set_mode(expected_mode);
            
            info!("------------------------------------");
        }
        
        info!("TUI event loop ended");
        Ok(())
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    
    // Create terminal with crossterm backend
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
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