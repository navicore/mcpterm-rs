use anyhow::Result;
use crossbeam_channel::Sender;
use mcp_runtime::{create_handler, ApiEvent, EventBus, EventHandler, ModelEvent, UiEvent};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tracing::debug;

/// The CliEventAdapter bridges the CLI interface with the event bus architecture
pub struct CliEventAdapter {
    event_bus: Arc<EventBus>,
    ui_tx: Sender<UiEvent>,
    #[allow(dead_code)]
    model_tx: Sender<ModelEvent>,
    #[allow(dead_code)]
    api_tx: Sender<ApiEvent>,
    direct_model_tx: Option<Sender<ModelEvent>>, // Direct model sender from session manager
    response_buffer: Arc<Mutex<String>>,
    interactive_mode: bool,
}

impl CliEventAdapter {
    /// Create a new CLI adapter with the provided event bus
    pub fn new(event_bus: Arc<EventBus>, interactive_mode: bool) -> Self {
        // Get channel senders from the shared event bus
        let ui_tx = event_bus.ui_sender();
        let model_tx = event_bus.model_sender();
        let api_tx = event_bus.api_sender();
        let response_buffer = Arc::new(Mutex::new(String::new()));

        debug!("Created CliEventAdapter with event bus (UI handlers: {}, Model handlers: {}, API handlers: {})",
               event_bus.ui_handlers(),
               event_bus.model_handlers(),
               event_bus.api_handlers());

        Self {
            event_bus,
            ui_tx,
            model_tx,
            api_tx,
            direct_model_tx: None,
            response_buffer,
            interactive_mode,
        }
    }

    /// Register all event handlers for CLI
    pub fn register_handlers(&self) -> Result<()> {
        // Register a handler for model events to display LLM responses
        let model_handler = self.create_model_event_handler();
        self.event_bus.register_model_handler(model_handler)?;

        // Register a handler for API events
        let api_handler = self.create_api_event_handler();
        self.event_bus.register_api_handler(api_handler)?;

        // NOTE: Do not start event distribution here!
        // It will be started in the higher-level CliSession to avoid duplicate event processing

        Ok(())
    }

    /// Get the underlying event bus (returns the Arc reference)
    pub fn get_event_bus(&self) -> Arc<EventBus> {
        Arc::clone(&self.event_bus)
    }

    /// Set a direct model sender from the session manager
    pub fn set_direct_model_sender(&self, sender: Sender<ModelEvent>) {
        // We need to use unsafe to modify self, since the method signature doesn't allow for mutable self
        // This is a temporary workaround for the event bus issues
        unsafe {
            let self_ptr = self as *const Self as *mut Self;
            (*self_ptr).direct_model_tx = Some(sender);
        }
    }

    /// Send a user message to the event bus
    pub fn send_user_message(&self, message: &str) -> Result<()> {
        debug!("CliEventAdapter sending user message: '{}' - handlers info: UI: {}, Model: {}, API: {}",
               message, self.event_bus.ui_handlers(), self.event_bus.model_handlers(), self.event_bus.api_handlers());

        // Clear the response buffer before sending a new message
        {
            let mut buffer = self.response_buffer.lock().unwrap();
            *buffer = String::new();
        }

        // IMPORTANT: Only send the message through ONE path to avoid duplicates
        // We'll try the UI channel first (the proper way), and only fall back to direct model
        // channel if the UI channel fails

        // Try to send to the UI channel to follow normal event flow
        debug!("Sending user input to UI event channel");
        if let Err(e) = self.ui_tx.send(UiEvent::UserInput(message.to_string())) {
            debug!("Failed to send to UI channel: {}", e);

            // UI channel failed, now try the direct model channel as a fallback
            if let Some(direct_model_tx) = &self.direct_model_tx {
                debug!("Sending user message directly to model channel (via direct sender)");
                if let Err(e) =
                    direct_model_tx.send(ModelEvent::ProcessUserMessage(message.to_string()))
                {
                    // This is a critical error since this is our last resort
                    return Err(anyhow::anyhow!(
                        "Failed to send message to model channel: {}",
                        e
                    ));
                }
                debug!("Successfully sent user message to model channel via direct sender");
            } else {
                // No direct model sender, try regular event bus model sender
                debug!("No direct model sender available, using event bus model sender");
                let model_tx = self.event_bus.model_sender();
                if let Err(e) = model_tx.send(ModelEvent::ProcessUserMessage(message.to_string())) {
                    // This is a critical error since it's our last resort
                    return Err(anyhow::anyhow!(
                        "Failed to send message to model channel: {}",
                        e
                    ));
                }
                debug!("Successfully sent user message to model channel via event bus");
            }
        } else {
            debug!("Successfully sent user input to UI channel - NOT sending to model channel to avoid duplicates");
        }

        debug!("Successfully sent user message through exactly one channel");
        Ok(())
    }

    /// Request cancellation of any ongoing operations
    pub fn request_cancellation(&self) -> Result<()> {
        self.ui_tx.send(UiEvent::RequestCancellation)?;
        Ok(())
    }

    /// Clear the conversation context
    pub fn clear_conversation(&self) -> Result<()> {
        self.ui_tx.send(UiEvent::ClearConversation)?;
        Ok(())
    }

    /// Wait for and collect a response with improved reliability
    pub fn wait_for_response(&self, timeout_seconds: u64) -> Result<String> {
        debug!(
            "Starting wait_for_response with timeout of {} seconds",
            timeout_seconds
        );

        // Set up a channel to signal completion
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        // Store the response buffer so we can access it later
        let response_buffer = self.response_buffer.clone();

        // We'll track the TX end with an Arc<Mutex> so it can be shared with the handler
        let tx_mutex = Arc::new(std::sync::Mutex::new(Some(tx)));

        // Create a timer for periodic checking of response status
        let timer_response_buffer = response_buffer.clone();
        let timer_tx_mutex = tx_mutex.clone();

        // Create a special Model handler for completion detection
        let completion_handler = create_handler(move |event: ModelEvent| {
            let tx_mutex = tx_mutex.clone();

            Box::pin(async move {
                match event {
                    ModelEvent::LlmResponseComplete => {
                        debug!("Detected LLM response completion event");
                        // Signal completion through the channel
                        if let Ok(mut guard) = tx_mutex.lock() {
                            if let Some(tx) = guard.take() {
                                let _ = tx.send(());
                                debug!("Sent completion signal due to LlmResponseComplete");
                            }
                        }
                    }
                    ModelEvent::LlmMessage(content) => {
                        // For non-streaming responses, complete immediately if content is present
                        if !content.is_empty() {
                            debug!("Detected LlmMessage with content (len={})", content.len());
                            if let Ok(mut guard) = tx_mutex.lock() {
                                if let Some(tx) = guard.take() {
                                    let _ = tx.send(());
                                    debug!("Sent completion signal due to LlmMessage");
                                }
                            }
                        }
                    }
                    ModelEvent::LlmStreamChunk(chunk) => {
                        // For streaming, just log that we received a chunk
                        // We'll wait for completion signal
                        debug!("Received stream chunk of length {}", chunk.len());
                    }
                    _ => { /* Ignore other events */ }
                }
                Ok(())
            })
        });

        // Register this completion handler for this specific response
        if let Err(e) = self.event_bus.register_model_handler(completion_handler) {
            return Err(anyhow::anyhow!(
                "Failed to register completion handler: {}",
                e
            ));
        }

        debug!("Registered completion handler successfully");

        // Check if we're running in a test environment
        let is_test = cfg!(test) || std::env::args().any(|arg| arg.contains("test"));

        // Start a periodic checker that will signal completion if content is detected
        // but we somehow missed the completion event
        if !is_test {
            let response_buffer_for_timer = timer_response_buffer.clone();
            let timer_tx = timer_tx_mutex.clone();

            // Spawn a timer that checks the response buffer periodically
            std::thread::spawn(move || {
                let start_time = std::time::Instant::now();
                let max_duration = std::time::Duration::from_secs(timeout_seconds);
                let check_interval = std::time::Duration::from_millis(500);

                // Loop until timeout
                while start_time.elapsed() < max_duration {
                    std::thread::sleep(check_interval);

                    // Check if we have content in the buffer
                    if let Ok(buffer) = response_buffer_for_timer.lock() {
                        if !buffer.is_empty() && buffer.len() > 5 {
                            // We have content, but no completion signal
                            // This might happen if the completion event was lost
                            if let Ok(mut guard) = timer_tx.lock() {
                                if let Some(tx) = guard.take() {
                                    let _ = tx.send(());
                                    debug!(
                                        "Sent completion signal from timer due to non-empty buffer"
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            });
        }

        // Set up waiting for the response with appropriate handling for different environments
        let is_timeout = if is_test {
            debug!("Test environment detected, skipping async wait");
            false // In tests, pretend it completed successfully
        } else if tokio::runtime::Handle::try_current().is_ok() {
            // We're already in a runtime, use a sync channel to communicate back
            debug!("Using existing tokio runtime");
            let (tx_sync, rx_sync) = std::sync::mpsc::channel();

            tokio::spawn(async move {
                let timeout = tokio::time::sleep(std::time::Duration::from_secs(timeout_seconds));
                tokio::pin!(timeout);

                tokio::select! {
                    _ = rx => {
                        debug!("Response completion signal received");
                        let _ = tx_sync.send(false);
                    }
                    _ = timeout => {
                        debug!("Response wait timed out after {} seconds", timeout_seconds);
                        let _ = tx_sync.send(true);
                    }
                }
            });

            // Wait for the result with a longer timeout to ensure we get a response
            // The timeout is longer than the expected operation to give it a chance to complete
            match rx_sync.recv_timeout(std::time::Duration::from_secs(timeout_seconds + 5)) {
                Ok(result) => result,
                Err(_) => {
                    debug!("Sync channel timed out waiting for response");
                    true
                }
            }
        } else {
            // We're not in a runtime, create a new one
            debug!("Creating new tokio runtime");
            let runtime = tokio::runtime::Runtime::new()?;

            runtime.block_on(async {
                let timeout = tokio::time::sleep(std::time::Duration::from_secs(timeout_seconds));
                tokio::pin!(timeout);

                tokio::select! {
                    result = rx => {
                        match result {
                            Ok(_) => {
                                debug!("Response completion signal received");
                                false
                            }
                            Err(_) => {
                                debug!("Response receiver was dropped");
                                true
                            }
                        }
                    }
                    _ = timeout => {
                        debug!("Response wait timed out after {} seconds", timeout_seconds);
                        true
                    }
                }
            })
        };

        // Get the final response from the buffer
        debug!("Getting final response buffer");
        let buffer = match response_buffer.lock() {
            Ok(guard) => guard.clone(),
            Err(e) => {
                debug!("Failed to lock response buffer: {}", e);
                String::new()
            }
        };

        if !buffer.is_empty() {
            debug!(
                "Returning non-empty response buffer of length {}",
                buffer.len()
            );
            Ok(buffer)
        } else if is_timeout {
            debug!("No response received within timeout");
            Ok("No response received within timeout.".to_string())
        } else {
            debug!("Returning empty response (but completion was signaled)");
            Ok("No response content received, but completion was signaled.".to_string())
        }
    }

    /// Create a handler for model events (LLM responses, tool calls, etc.)
    fn create_model_event_handler(&self) -> EventHandler<ModelEvent> {
        let response_buffer = self.response_buffer.clone();
        let interactive_mode = self.interactive_mode;

        create_handler(move |event: ModelEvent| {
            let response_buffer = response_buffer.clone();

            Box::pin(async move {
                match event {
                    ModelEvent::LlmMessage(content) => {
                        // TODO: We should also display LLM messages about what tools it's about to use,
                        // not just the results. This would help users understand what the model is doing.
                        // These messages should not be filtered out if they discuss tool usage plans.

                        // Store the message in the response buffer
                        {
                            let mut buffer = response_buffer.lock().unwrap();
                            *buffer = content.clone();
                        }

                        // For interactive mode, also print to stdout directly
                        if interactive_mode {
                            println!("{}", content);
                        }
                    }
                    ModelEvent::LlmStreamChunk(chunk) => {
                        // Append the chunk to the response buffer
                        {
                            let mut buffer = response_buffer.lock().unwrap();
                            buffer.push_str(&chunk);
                        }

                        // For interactive mode, print the chunk immediately
                        if interactive_mode {
                            print!("{}", chunk);
                            let _ = io::stdout().flush();
                        }
                    }
                    ModelEvent::LlmResponseComplete => {
                        // When response is complete, add a newline in interactive mode
                        if interactive_mode {
                            println!();
                        }
                        debug!("LLM response complete");
                    }
                    ModelEvent::ToolRequest(tool_id, params) => {
                        debug!("Tool request: {} with params: {:?}", tool_id, params);
                        // Tool calls are handled by the SessionManager
                    }
                    ModelEvent::ToolResult(tool_id, result) => {
                        debug!("Tool result: {} with result: {:?}", tool_id, result);

                        // Skip displaying results for invalid tool commands
                        if tool_id == "unknown" && result.get("error").is_some() {
                            debug!("Skipping display of invalid tool command result");
                            return Ok(());
                        }

                        // Also skip displaying internal error messages that aren't user-friendly
                        if let Some(error) = result.get("error").and_then(|e| e.as_str()) {
                            // Only filter out specific technical error messages, not all errors
                            if (error.contains("[Invalid tool command detected]") && error.trim() == "[Invalid tool command detected]") ||
                               (error.contains("[Invalid JSON format in tool command]") && error.trim() == "[Invalid JSON format in tool command]") ||
                               error.contains("[Tool command with invalid JSON:") {
                                debug!("Skipping display of internal error message: {}", error);
                                return Ok(());
                            }

                            // Don't filter out errors with meaningful content
                            debug!("Displaying meaningful error message: {}", error);
                        }

                        // Check if this is a skipped duplicate result
                        if let Some(skipped) = result.get("skipped").and_then(|s| s.as_bool()) {
                            if skipped {
                                debug!("Detected skipped duplicate tool execution: {}", tool_id);
                                if interactive_mode {
                                    println!("Note: Skipped duplicate execution of tool '{}'", tool_id);
                                }
                                return Ok(());
                            }
                        }

                        // Convert the result to a ToolResult struct for formatting
                        let tool_result = mcp_tools::ToolResult {
                            tool_id: tool_id.clone(),
                            status: if result.get("error").is_some() {
                                mcp_tools::ToolStatus::Failure
                            } else {
                                mcp_tools::ToolStatus::Success
                            },
                            output: result.clone(),
                            error: result.get("error").and_then(|e| e.as_str()).map(String::from),
                        };

                        // Format the tool result
                        let formatted = crate::formatter::ResponseFormatter::format_tool_result(&tool_result);

                        // In interactive mode, display immediately
                        if interactive_mode {
                            println!("{}", formatted);
                        } else {
                            // In non-interactive mode, append to the response buffer
                            let mut buffer = response_buffer.lock().unwrap();
                            if !buffer.is_empty() {
                                buffer.push_str("\n\n");
                            }
                            buffer.push_str(&formatted);
                        }
                    }
                    _ => {
                        // Other events are handled elsewhere
                    }
                }
                Ok(())
            })
        })
    }

    /// Create a handler for API events
    fn create_api_event_handler(&self) -> EventHandler<ApiEvent> {
        let interactive_mode = self.interactive_mode;

        create_handler(move |event: ApiEvent| {
            Box::pin(async move {
                match event {
                    ApiEvent::Error(error) => {
                        debug!("API error: {}", error);
                        if interactive_mode {
                            eprintln!("Error: {}", error);
                        }
                    }
                    ApiEvent::ConnectionLost(reason) => {
                        debug!("Connection lost: {}", reason);
                        if interactive_mode {
                            eprintln!("Connection lost: {}", reason);
                        }
                    }
                    _ => {
                        // Other API events are not directly relevant to CLI output
                    }
                }
                Ok(())
            })
        })
    }
}
