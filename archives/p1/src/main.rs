mod agent;
mod config;
mod editor;
mod journal;
mod mcp;
mod ui;

use crate::mcp::debug_log;
use agent::{Agent, BedrockAgentSync, McpAgent, StubAgent};
use anyhow::Result;
use clap::Parser;
use config::{Cli, Config};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use editor::{EdtuiEditor, HandleResult, ReadOnlyEditor};
use journal::Journal;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders},
    Terminal as RatatuiTerminal,
};
use std::io;
use std::time::Duration;
use ui::MessageType;

// Function to format messages with styled prefixes for use in a widget
// This is a separate function from the styling in ReadOnlyEditor to demonstrate
// direct Ratatui styling capability
// fn format_message_with_style(
//     content: &str,
//     message_type: MessageType,
// ) -> ratatui::text::Text<'static> {
//     use ratatui::style::{Color, Modifier, Style};
//     use ratatui::text::{Line, Span, Text};
//
//     // Create styled prefixes for different message types
//     let (prefix, style) = match message_type {
//         MessageType::System => (
//             "SYSTEM: ",
//             Style::default()
//                 .fg(Color::Yellow)
//                 .add_modifier(Modifier::BOLD),
//         ),
//         MessageType::User => (
//             "YOU: ",
//             Style::default()
//                 .fg(Color::Blue)
//                 .add_modifier(Modifier::BOLD),
//         ),
//         MessageType::Assistant => (
//             "MCP: ",
//             Style::default()
//                 .fg(Color::Green)
//                 .add_modifier(Modifier::BOLD),
//         ),
//     };
//
//     // Create a styled span for the prefix
//     let styled_prefix = Span::styled(prefix.to_string(), style);
//
//     // Create a regular span for the content
//     let content_span = Span::raw(content.to_string());
//
//     // Combine them into a single line
//     let line = Line::from(vec![styled_prefix, content_span]);
//
//     // Create and return a Text object with just this line
//     Text::from(line)
// }

struct MessageHistory {
    messages: Vec<(String, MessageType)>,
    scroll_offset: usize,
    journal: Option<Journal>,
}

enum FocusArea {
    Messages,
    Input,
}

impl MessageHistory {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            journal: None,
        }
    }

    fn init_journal(&mut self) -> Result<()> {
        let journal = Journal::new()?;

        // Load existing messages from today's journal if it exists
        let journal_messages = journal.load_current_journal()?;
        self.messages = journal_messages;

        self.journal = Some(journal);
        Ok(())
    }

    fn add_message(&mut self, content: String, message_type: MessageType) {
        self.messages.push((content.clone(), message_type));

        // Also save to journal if initialized
        if let Some(journal) = &self.journal {
            // We ignore errors here to not disrupt the UI experience
            let _ = journal.append_message(&content, message_type);
        }

        // Set scroll_offset to 0 to show the latest messages (at the bottom)
        self.scroll_offset = 0;
    }

    // Apply all messages to the editor
    fn apply_to_editor(&self, editor: &mut ReadOnlyEditor) {
        editor.clear();
        for (content, msg_type) in &self.messages {
            // Clean up the content if it's an assistant message
            if *msg_type == MessageType::Assistant {
                // Preprocess content to make it display better
                let cleaned_content = self.preprocess_content(content);
                editor.append_styled_content(cleaned_content, *msg_type);
            } else {
                editor.append_styled_content(content.clone(), *msg_type);
            }
        }
        // Update styles
        editor.debug_styles();
    }

    // Process content to remove problematic elements and make display better
    fn preprocess_content(&self, content: &str) -> String {
        // First, handle the case of special system messages and error responses
        if content.starts_with("An unexpected error occurred")
            || content.starts_with("The request was cancelled")
            || content.starts_with("The request was taking too long")
        {
            return content.to_string();
        }

        // IMPORTANT: We need to preserve the original format of command results
        // so they can be properly processed by the LLM in subsequent turns
        if content.contains("**Command Result:**") {
            // Only remove debug messages but keep the command structure intact
            let content = content.replace("Response generated without command execution", "");
            let content = content.replace("Commands executed successfully", "");

            // This is critical: we must preserve the original format with the command results
            // for the LLM to properly process them
            return content.trim().to_string();
        }

        // For responses without command results, apply normal formatting
        // Remove any debug message lines
        let content = content.replace("Response generated without command execution", "");
        let content = content.replace("Commands executed successfully", "");

        // Make sure command results are displayed prominently
        // This ensures users see the command output even if it's mixed with other text
        let formatted_content = if content.contains("mcp shell") {
            if content.contains("**Command Result:**") {
                // Make command results more obvious with highlight markers
                let highlighted = content.replace("**Command Result:**", "⭐ COMMAND RESULT ⭐");
                highlighted.trim().to_string()
            } else {
                // If we have a shell command but no result marker, add a note
                let note = "\n\n[Note: Command execution may be in progress or timeout occurred]";
                format!("{}{}", content.trim(), note)
            }
        } else {
            content.trim().to_string()
        };

        formatted_content
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments and load configuration
    let cli = Cli::parse();
    let config = Config::load(&cli)?;

    // Initialize debug logging (to file only)
    // No output at all to avoid disrupting TUI
    let _ = crate::mcp::init_debug_log();

    // Initialize agent based on configuration
    let agent: Box<dyn Agent> = if config.mcp.enabled {
        // If MCP is enabled, use the MCP agent
        match McpAgent::from_config(&config) {
            Ok(mcp_agent) => {
                println!("Using MCP agent");
                Box::new(mcp_agent)
            }
            Err(e) => {
                debug_log(&format!("Failed to initialize MCP agent: {}", e));
                if config.aws.region.is_empty() {
                    Box::new(StubAgent::new())
                } else {
                    Box::new(BedrockAgentSync::from_config(&config))
                }
            }
        }
    } else if config.aws.region.is_empty() {
        // If no AWS region is configured, use the stub agent
        Box::new(StubAgent::new())
    } else {
        // Use the Bedrock agent with our config
        Box::new(BedrockAgentSync::from_config(&config))
    };

    // Initialize message history
    let mut history = MessageHistory::new();

    // Initialize journal and load existing messages
    if let Err(e) = history.init_journal() {
        // If there's an error initializing the journal, log it as a system message
        let error_message = format!("Failed to initialize journal: {}", e);
        history.add_message(error_message, MessageType::System);
    }

    // Only add welcome message if history is empty (no previous messages were loaded)
    if history.messages.is_empty() {
        let welcome_message = String::from("Welcome to mcpterm! Tab to switch focus between editors.\n\nInput editor (bottom): Press 'i' to edit in insert mode, Enter in normal mode to submit, q to quit. Use Ctrl+V to paste from clipboard in insert mode, or 'v' to select text and 'y' to copy.\n\nMessage editor (top): Read-only with vim navigation - use h/j/k/l to move, v for visual mode to select text, y to copy selected text to system clipboard, G to jump to end, gg to jump to start. Press '/' to search in text (edtui native search).\n\nMCP Features: This terminal supports the Model Context Protocol (MCP) for enhanced AI agent capabilities. You can use the shell, search, and coding tools.");
        history.add_message(welcome_message.clone(), MessageType::System);
    }

    // Add a message showing which model is active
    if let Some(model) = config.get_active_model() {
        let model_info = if let Some(desc) = &model.description {
            format!("Using model: {} ({})", model.model_id, desc)
        } else {
            format!("Using model: {}", model.model_id)
        };

        let config_info = format!(
            "Model settings: max_tokens={}, temperature={:.1}",
            model.max_tokens, model.temperature
        );

        // Add MCP info if enabled
        let mcp_info = if config.mcp.enabled {
            "MCP is enabled. You can use shell, search, and coding tools."
        } else {
            "MCP is disabled."
        };

        // Add the model information as a system message
        let model_message = format!("{}\n{}\n{}", model_info, config_info, mcp_info);
        history.add_message(model_message, MessageType::System);
    } else {
        // If no active model was found, show a message about using stub agent
        let stub_message = "No active model configured. Using stub agent.".to_string();

        // Add MCP info if enabled
        let mcp_info = if config.mcp.enabled {
            format!(
                "{}\nMCP is enabled. You can use shell, search, and coding tools.",
                stub_message
            )
        } else {
            stub_message
        };

        history.add_message(mcp_info, MessageType::System);
    }

    // Setup terminal for TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = std::rc::Rc::new(std::cell::RefCell::new(RatatuiTerminal::new(backend)?));

    // Create our edtui editor for input
    let mut editor = EdtuiEditor::new().title("Input".to_string());
    editor.block = Some(Block::default().borders(Borders::ALL));

    // Create our read-only editor for messages display
    let mut messages_editor = ReadOnlyEditor::new().title(
        "Messages [Tab to focus, v for visual mode, y to copy, G/gg to navigate, / to search]"
            .to_string(),
    );
    messages_editor.block = Some(Block::default().borders(Borders::ALL));

    // Initialize the messages editor with the welcome message
    history.apply_to_editor(&mut messages_editor);

    // Main loop for TUI display
    let mut running = true;
    let mut focus = FocusArea::Input; // Default focus on input area

    while running {
        // Draw the UI
        terminal.borrow_mut().draw(|f| {
            let size = f.area();

            // Create layout with 70% messages, 30% input
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
                .split(size);

            // Set border style based on focus for messages editor
            let message_block = Block::default()
                .borders(Borders::ALL)
                .border_style(match focus {
                    FocusArea::Messages => Style::default().fg(Color::Yellow),
                    _ => Style::default(),
                })
                .title("Messages [READ-ONLY] [Tab to focus, v for visual mode, y to copy, G/gg to navigate, / to search]");

            // Update the block for messages editor based on focus
            if let Some(block) = &mut messages_editor.block {
                *block = message_block;
            } else {
                messages_editor.block = Some(message_block);
            }

            // Set border style based on focus for input editor
            let input_block = Block::default()
                .borders(Borders::ALL)
                .border_style(match focus {
                    FocusArea::Input => Style::default().fg(Color::Yellow),
                    _ => Style::default(),
                })
                .title(format!("Input [i: insert, ⏎: submit, ^P/^N: history{}]",
                    if let Some(idx) = editor.get_history_index() {
                        format!(" ({})", idx + 1)
                    } else {
                        "".to_string()
                    }));

            // Update the block for input editor
            if let Some(block) = &mut editor.block {
                *block = input_block;
            } else {
                editor.block = Some(input_block);
            }

            // Render the messages editor widget using a mutable reference
            f.render_widget(&mut messages_editor, chunks[0]);

            // Render the input editor widget using a mutable reference
            f.render_widget(&mut editor, chunks[1]);
        })?;

        // Handle events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Handle Tab key to switch focus between areas
                if key.code == KeyCode::Tab {
                    focus = match focus {
                        FocusArea::Input => FocusArea::Messages,
                        FocusArea::Messages => FocusArea::Input,
                    };
                    continue;
                }

                // Handle focus-specific keybindings
                match focus {
                    FocusArea::Input => {
                        // Pass key events to our editor
                        match editor.handle_key_event(key) {
                            HandleResult::Continue => {}
                            HandleResult::Submit(text) => {
                                // Process the submitted text
                                history.add_message(text.clone(), MessageType::User);

                                // Add styled user message
                                messages_editor
                                    .append_styled_content(text.clone(), MessageType::User);

                                // Add to command history
                                editor.add_to_history();

                                // Clear the input for next entry
                                editor.clear();

                                // Create a temporary editor to show progress without affecting the main editor
                                let mut temp_editor = messages_editor.clone();

                                // Add a more informative "thinking" message to the temporary editor
                                temp_editor.append_styled_content(
                                    "⏳ Generating response... (Connecting to AWS Bedrock, this may take up to 60 seconds)".to_string(),
                                    MessageType::System,
                                );

                                // Force an immediate redraw with the temporary editor
                                terminal.borrow_mut().draw(|f| {
                                    let size = f.area();
                                    let chunks = Layout::default()
                                        .direction(Direction::Vertical)
                                        .constraints(
                                            [
                                                Constraint::Percentage(70),
                                                Constraint::Percentage(30),
                                            ]
                                            .as_ref(),
                                        )
                                        .split(size);
                                    f.render_widget(&mut temp_editor, chunks[0]);
                                    f.render_widget(&mut editor, chunks[1]);
                                })?;

                                let _mcp_agent = agent.as_any().downcast_ref::<McpAgent>();

                                // Update the temporary editor with processing message
                                temp_editor.append_styled_content(
                                    "⏳ Processing request and executing commands (limited to max 5 iterations)...".to_string(),
                                    MessageType::System,
                                );

                                // Force a redraw with the temporary editor
                                terminal.borrow_mut().draw(|f| {
                                    let size = f.area();
                                    let chunks = Layout::default()
                                        .direction(Direction::Vertical)
                                        .constraints(
                                            [
                                                Constraint::Percentage(70),
                                                Constraint::Percentage(30),
                                            ]
                                            .as_ref(),
                                        )
                                        .split(size);
                                    f.render_widget(&mut temp_editor, chunks[0]);
                                    f.render_widget(&mut editor, chunks[1]);
                                })?;

                                // Create a thread to process the message without blocking the UI
                                // We use a standard thread instead of tokio::spawn for simplicity
                                // since agent.process_message is already blocking
                                let text_clone = text.clone();
                                let agent_clone = agent.clone();

                                // Create a channel for communication between threads (unused for now)
                                // let (_tx, _rx) = std::sync::mpsc::channel::<String>();

                                // IMPORTANT: Create a more robust thread with a proper mechanism
                                // to detect long-running operations

                                // Use a flag to track if we have a result or error
                                let (result_tx, result_rx) = std::sync::mpsc::channel();
                                let (error_tx, error_rx) = std::sync::mpsc::channel();

                                // Create a thread to process the message
                                let _processing_thread = std::thread::spawn(move || {
                                    // Use a timer to ensure we don't get stuck (for future use)
                                    let _thread_start = std::time::Instant::now();

                                    // Track if we're processing a command (for logging purposes)
                                    let mut _last_status = String::from("Starting processing...");
                                    let status_tx = error_tx.clone();

                                    // Simple helper for updating status
                                    let update_status = |msg: &str| {
                                        let _ = status_tx.send(format!("⏳ {}", msg));
                                    };

                                    // If this is an MCP agent, get special handling
                                    if let Some(mcp_agent) =
                                        agent_clone.as_any().downcast_ref::<McpAgent>()
                                    {
                                        // Use the progress reporting capability
                                        update_status("Processing with MCP agent...");

                                        // Set a safety timeout in case the agent gets stuck (increased timeout)
                                        let safety_timeout =
                                            config.ui.command_timeout.unwrap_or(240);
                                        let safety_thread = std::thread::spawn(move || {
                                            std::thread::sleep(Duration::from_secs(safety_timeout));
                                            let _ = error_tx.send(format!(
                                                "⚠️ Command execution timeout after {} seconds",
                                                safety_timeout
                                            ));
                                        });

                                        // Process the message with progress callbacks
                                        let response = mcp_agent.process_message_with_progress(
                                            &text_clone,
                                            move |status| {
                                                // Update the status (for logging purposes)
                                                _last_status = status.to_string();
                                                let _ = status_tx.send(status.to_string());
                                            },
                                        );

                                        // We completed normally, so cancel the safety thread
                                        drop(safety_thread);

                                        // Send the result
                                        let _ = result_tx.send(response);
                                    } else {
                                        // Regular agent without progress reporting
                                        update_status("Processing with standard agent...");

                                        // Process the message
                                        let response = agent_clone.process_message(&text_clone);

                                        // Send the result
                                        let _ = result_tx.send(response);
                                    }
                                });

                                // Set a timeout for processing requests (significantly increased)
                                let timeout_duration =
                                    Duration::from_secs(config.ui.command_timeout.unwrap_or(240));
                                let start_time = std::time::Instant::now();

                                // Poll for events while waiting for the task to complete
                                let mut response = String::new();
                                let mut cancelled = false;
                                let mut status_message = "Processing request...".to_string();

                                'waiting: loop {
                                    // Check if we have a response (non-blocking)
                                    match result_rx.try_recv() {
                                        Ok(result) => {
                                            // Got a response
                                            response = result;
                                            break 'waiting;
                                        }
                                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                                            // No response yet, continue waiting
                                        }
                                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                            // Thread crashed or disconnected
                                            response = "An unexpected error occurred during processing. The connection to the AI service may have been interrupted. Please try again.".to_string();
                                            break 'waiting;
                                        }
                                    }

                                    // Check for status updates
                                    if let Ok(status) = error_rx.try_recv() {
                                        // Clone the status to avoid move issues
                                        let status_clone = status.clone();
                                        status_message = status;

                                        // If the status indicates a timeout, we should stop waiting
                                        if status_clone.contains("timeout") {
                                            response = format!("The request was taking too long and has timed out. Last status: {}", status_message);
                                            break 'waiting;
                                        }
                                    }

                                    // Check for timeout
                                    if start_time.elapsed() > timeout_duration {
                                        // This doesn't actually cancel the work (thread continues in background),
                                        // but we stop waiting for it
                                        response = format!("The request was taking too long and has timed out after {:?}. Last status: {}. You can try again with a simpler query or adjust the timeout in the config file.", timeout_duration, status_message);
                                        break 'waiting;
                                    }

                                    // Poll for user input (with a small timeout)
                                    if event::poll(Duration::from_millis(50))? {
                                        if let Event::Key(key) = event::read()? {
                                            // Check for ESC to cancel the operation
                                            if key.code == KeyCode::Esc {
                                                // We can't really abort the thread since Agent::process_message
                                                // is blocking, but we can stop waiting for it
                                                cancelled = true;
                                                break 'waiting;
                                            }
                                        }
                                    }

                                    // Update the UI to show that we're still processing
                                    let elapsed = start_time.elapsed().as_secs();
                                    let dots = ".".repeat((elapsed % 4) as usize);

                                    // Create a temporary copy of the history to show current status
                                    temp_editor.clear();

                                    // Create a temporary copy of messages with the status appended
                                    let mut temp_messages = history.messages.clone();
                                    temp_messages.push((
                                        format!("⏳ Processing request{} ({:?} elapsed, press ESC to cancel)",
                                            dots,
                                            start_time.elapsed()
                                        ),
                                        MessageType::System
                                    ));

                                    // Apply each message individually
                                    for (content, msg_type) in &temp_messages {
                                        temp_editor
                                            .append_styled_content(content.clone(), *msg_type);
                                    }

                                    // Force a redraw with the temporary editor
                                    terminal.borrow_mut().draw(|f| {
                                        let size = f.area();
                                        let chunks = Layout::default()
                                            .direction(Direction::Vertical)
                                            .constraints(
                                                [
                                                    Constraint::Percentage(70),
                                                    Constraint::Percentage(30),
                                                ]
                                                .as_ref(),
                                            )
                                            .split(size);
                                        f.render_widget(&mut temp_editor, chunks[0]);
                                        f.render_widget(&mut editor, chunks[1]);
                                    })?;

                                    // Small sleep to prevent CPU spinning
                                    std::thread::sleep(Duration::from_millis(100));
                                }

                                // If the request was cancelled, show a message
                                if cancelled {
                                    response = "The request was cancelled. You can try again with a different prompt if needed.".to_string();
                                }

                                // Check if this is a special message (timeout, cancelled, etc.)
                                // Only log status info, not add debug messages to response
                                let is_special_message = cancelled
                                    || response.starts_with("The request was taking too long")
                                    || response.starts_with("An unexpected error occurred");

                                // Check if the response contains command results for logging purposes only
                                let has_command_results = response.contains("**Command Result:**");

                                // Clean up the response to make it more readable in the UI
                                // Remove the command result markers that might cause issues
                                if !is_special_message && has_command_results {
                                    // Log command execution status only in debug mode
                                    if config.logging.api_debug {
                                        debug_log("Commands executed successfully");
                                    }
                                } else if !is_special_message {
                                    // Log only in debug mode
                                    if config.logging.api_debug {
                                        debug_log("Response generated without command execution");
                                    }
                                }

                                // For short inputs or test messages, don't parse as multi-line results
                                // This makes the UI more responsive for simple queries

                                // Preserve the original response exactly as-is for use in next LLM interaction
                                // This is crucial so the LLM can see the command results
                                let original_response = response.clone(); // Save the original response

                                // Note: We previously had display_response processing here, but it's not needed
                                // since the ReadOnlyEditor's preprocess_content method handles the display formatting

                                // CRITICAL: We need to preserve the original format with command results
                                // for the LLM to see them in subsequent turns
                                // Add the ORIGINAL response to history (this is what gets returned to the LLM in next turn)
                                history.add_message(original_response, MessageType::Assistant);

                                // First, clear the messages editor completely
                                messages_editor.clear();

                                // Then rebuild the entire message view with the updated history
                                // The editor will apply our display formatting through preprocess_content
                                history.apply_to_editor(&mut messages_editor);

                                // Force a redraw to ensure everything is displayed correctly
                                terminal.borrow_mut().draw(|f| {
                                    let size = f.area();
                                    let chunks = Layout::default()
                                        .direction(Direction::Vertical)
                                        .constraints(
                                            [
                                                Constraint::Percentage(70),
                                                Constraint::Percentage(30),
                                            ]
                                            .as_ref(),
                                        )
                                        .split(size);
                                    f.render_widget(&mut messages_editor, chunks[0]);
                                    f.render_widget(&mut editor, chunks[1]);
                                })?;
                            }
                            HandleResult::Abort => {
                                running = false;
                            }
                        }
                    }
                    FocusArea::Messages => {
                        // Special case for 'i' and Enter to return to input area
                        // But only if we're not in search mode
                        if !messages_editor.is_search_mode
                            && ((key.code == KeyCode::Char('i')
                                && !key.modifiers.contains(KeyModifiers::CONTROL))
                                || key.code == KeyCode::Enter)
                        {
                            focus = FocusArea::Input;
                            continue;
                        }

                        // Special case for 'q' to quit
                        if key.code == KeyCode::Char('q')
                            && !key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            running = false;
                            continue;
                        }

                        // Pass all other keys to the read-only editor
                        // This enables vim-style navigation and selection
                        if let HandleResult::Abort = messages_editor.handle_key_event(key) {
                            running = false;
                        }
                    }
                }
            }
        }
    }

    // Clean up terminal
    disable_raw_mode()?;
    execute!(terminal.borrow_mut().backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}
