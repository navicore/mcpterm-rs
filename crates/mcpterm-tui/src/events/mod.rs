use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{Event as CrosstermEvent, KeyEvent, KeyModifiers};
use mcp_llm::client_trait::{LlmClient, LlmResponse};
use mcp_core::context::ConversationContext;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::{debug, error, info, trace, warn};

/// Event types for the TUI application
pub enum Event {
    /// Input event from the terminal
    Input(KeyEvent),
    
    /// Timer tick for UI updates
    Tick,
    
    /// LLM response received
    LlmResponse(String, Result<LlmResponse, anyhow::Error>),
    
    /// Tool execution result
    ToolResult(String, Result<String, anyhow::Error>),
    
    /// Status update for a running task
    StatusUpdate(String, String),
    
    /// User request to quit the application
    Quit,
}

// Custom implementation of Debug for Event to handle anyhow::Error
impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Input(key) => write!(f, "Event::Input({:?})", key),
            Event::Tick => write!(f, "Event::Tick"),
            Event::LlmResponse(req, _) => write!(f, "Event::LlmResponse({}, ...)", req),
            Event::ToolResult(id, _) => write!(f, "Event::ToolResult({}, ...)", id),
            Event::StatusUpdate(id, status) => write!(f, "Event::StatusUpdate({}, {})", id, status),
            Event::Quit => write!(f, "Event::Quit"),
        }
    }
}

/// Handler for TUI events
pub struct EventHandler {
    /// Channel for receiving events
    pub rx: Receiver<Event>,
    
    /// Channel for sending events
    pub tx: Sender<Event>,
    
    /// Tick rate for the event loop
    pub tick_rate: Duration,
    
    /// LLM client for processing requests
    pub llm_client: Option<Box<dyn LlmClient + Send + Sync>>,
    
    /// Tracks in-flight requests
    pub pending_requests: Arc<RwLock<Vec<String>>>,
}

// Implement Send and Sync for EventHandler
unsafe impl Send for EventHandler {}
unsafe impl Sync for EventHandler {}

impl EventHandler {
    /// Create a new event handler
    pub fn new() -> Result<Self> {
        debug!("Initializing TUI event handler");
        let tick_rate = Duration::from_millis(100);
        let (tx, rx) = crossbeam_channel::unbounded();
        let pending_requests = Arc::new(RwLock::new(Vec::new()));

        // Clone data for event thread
        let event_tx = tx.clone();
        let thread_pending = pending_requests.clone();

        // Spawn input handling thread
        info!(
            "Spawning event handling thread with tick rate of {}ms",
            tick_rate.as_millis()
        );
        thread::spawn(move || {
            debug!("Event handling thread started");
            let mut last_tick = Instant::now();
            
            loop {
                // Poll for events with a small timeout
                // Use non-blocking polling with a short timeout
                if let Ok(true) = crossterm::event::poll(Duration::from_millis(10)) {
                    info!("Event poll detected an event");
                    if let Ok(event) = crossterm::event::read() {
                        info!("Event read: {:?}", event);
                        trace!("Received terminal event: {:?}", event);
                        match event {
                            CrosstermEvent::Key(key) => {
                                trace!("Processing key event: {:?}", key);
                                
                                // Handle Ctrl+C specially for immediate exit
                                if key.code == crossterm::event::KeyCode::Char('c') 
                                   && key.modifiers.contains(KeyModifiers::CONTROL) {
                                    // Send quit event and break the loop
                                    let _ = event_tx.send(Event::Quit);
                                    break;
                                }
                                
                                // Send the key event
                                // Add debug output for key events
                                info!("EVENTS: Sending key event to main loop: {:?}", key);
                                match event_tx.send(Event::Input(key)) {
                                    Ok(_) => {
                                        info!("EVENTS: Key event sent successfully");
                                    },
                                    Err(e) => {
                                        // Channel closed, exit thread
                                        error!("EVENTS: Failed to send key event: {}", e);
                                        break;
                                    }
                                }
                            }
                            CrosstermEvent::Resize(_width, _height) => {
                                // If we want to handle resize events specifically, we could create
                                // a new event type for them
                                trace!("Terminal resize event");
                                // Force a tick to refresh the UI
                                if let Err(e) = event_tx.send(Event::Tick) {
                                    error!("Failed to send tick event after resize: {}", e);
                                    break;
                                }
                            }
                            // Handle other event types if needed
                            _ => {
                                trace!("Ignoring non-key event");
                            }
                        }
                    }
                }

                // Send tick event at regular intervals
                let now = Instant::now();
                if now.duration_since(last_tick) >= tick_rate {
                    trace!("Sending tick event");
                    if let Err(e) = event_tx.send(Event::Tick) {
                        // Channel closed, exit thread
                        error!("Failed to send tick event: {}", e);
                        break;
                    }
                    last_tick = now;
                    
                    // Check for any completed requests that need cleanup
                    let pending_count = thread_pending.read().unwrap().len();
                    if pending_count > 0 {
                        trace!("There are {} pending requests", pending_count);
                    }
                }
                
                // Small sleep to prevent CPU spinning
                thread::sleep(Duration::from_millis(10));
            }
            warn!("Event handling thread exiting");
        });

        debug!("Event handler initialization complete");
        Ok(Self { 
            rx, 
            tx, 
            tick_rate,
            llm_client: None,
            pending_requests,
        })
    }

    /// Get the next event from the channel (non-blocking with timeout)
    pub fn next(&self) -> Result<Event> {
        // Try to receive with a timeout to avoid blocking forever
        // Only log on debug level to avoid spamming
        trace!("EVENTS: Waiting for next event...");
        match self.rx.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => {
                match &event {
                    Event::Input(key) => {
                        info!("EVENTS: Received key event: {:?}", key);
                    },
                    Event::Tick => {
                        // Don't log ticks as they're too frequent
                    },
                    _ => {
                        info!("EVENTS: Received event: {:?}", event);
                    }
                }
                Ok(event)
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // Timeout is normal, just return a tick event without logging
                Ok(Event::Tick)
            }
            Err(e) => {
                error!("EVENTS: Failed to receive event: {}", e);
                Err(e.into())
            }
        }
    }
    
    /// Send an event to the channel
    pub fn send(&self, event: Event) -> Result<()> {
        self.tx.send(event)?;
        Ok(())
    }
    
    /// Set the LLM client
    pub fn set_llm_client(&mut self, client: Box<dyn LlmClient + Send + Sync>) {
        self.llm_client = Some(client);
    }
    
    /// Process a user message asynchronously without borrowing self
    pub fn process_message(
        tx: Sender<Event>,
        has_llm_client: bool,
        pending_requests: Arc<RwLock<Vec<String>>>,
        user_message: String, 
        context: Arc<RwLock<ConversationContext>>
    ) -> Result<()> {
        // Skip processing empty messages
        let trimmed = user_message.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        
        // Check if we have a client
        if !has_llm_client {
            let error_msg = "No LLM client available";
            tx.send(Event::LlmResponse(
                user_message.clone(),
                Err(anyhow::anyhow!(error_msg))
            ))?;
            return Ok(());
        }
        
        // Clone what we need for the task
        let tx_clone = tx.clone();
        let request_id = uuid::Uuid::new_v4().to_string();
        let user_message_clone = user_message.clone();
        let pending_requests_clone = pending_requests.clone();
        
        // Add to pending requests
        {
            let mut pending = pending_requests.write().unwrap();
            pending.push(request_id.clone());
        }
        
        // Send status update
        tx.send(Event::StatusUpdate(
            request_id.clone(),
            "Connecting to AI service...".to_string(),
        ))?;
        
        // Create a runtime handle for running async code from sync context
        let rt = tokio::runtime::Handle::current();
        
        // Spawn a new thread to handle the processing
        std::thread::spawn(move || {
            // Block on the async processing
            let _ = rt.block_on(async {
                debug!("Processing message in background task");
                
                // Get the context with a lock
                let context_result = if let Ok(ctx) = context.read() {
                    Ok(ctx.clone())
                } else {
                    Err(anyhow::anyhow!("Failed to acquire read lock on conversation context"))
                };
                
                // For now, simulate a response
                // In a real implementation, we would create an LLM client and send the message
                let process_future = async {
                    match context_result {
                        Ok(_ctx) => {
                            // Simulate an LLM response
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                            Ok(LlmResponse {
                                content: format!("Simulated response to: {}", user_message_clone),
                                tool_calls: Vec::new(),
                                id: uuid::Uuid::new_v4().to_string(),
                            })
                        }
                        Err(e) => Err(e),
                    }
                };
                
                // Wait for the response with a timeout
                let llm_result = match timeout(Duration::from_secs(60), process_future).await {
                    Ok(result) => {
                        // Process the result
                        match result {
                            Ok(response) => {
                                debug!("LLM response received successfully");
                                Ok(response)
                            }
                            Err(e) => {
                                error!("Error from LLM client: {}", e);
                                Err(e)
                            }
                        }
                    }
                    Err(_) => {
                        // Timeout occurred
                        error!("LLM request timed out after 60 seconds");
                        Err(anyhow::anyhow!("Request timed out"))
                    }
                };
                
                // Send the result back to the main thread
                if let Err(e) = tx_clone.send(Event::LlmResponse(user_message_clone, llm_result)) {
                    error!("Failed to send LLM response back to main thread: {}", e);
                }
                
                // Remove from pending requests
                if let Ok(mut pending) = pending_requests_clone.write() {
                    let mut idx = None;
                    for (i, id) in pending.iter().enumerate() {
                        if id == &request_id {
                            idx = Some(i);
                            break;
                        }
                    }
                    if let Some(i) = idx {
                        pending.remove(i);
                    }
                }
            });
        });
        
        Ok(())
    }
    
    /// Simplified method that calls the static version
    pub fn process_message_instance(&self, user_message: String, context: Arc<RwLock<ConversationContext>>) -> Result<()> {
        Self::process_message(
            self.tx.clone(),
            self.llm_client.is_some(),
            self.pending_requests.clone(),
            user_message,
            context
        )
    }
    
    /// Execute a tool asynchronously without borrowing self
    pub fn execute_tool(
        tx: Sender<Event>,
        tool_name: String,
        params: serde_json::Value
    ) -> Result<()> {
        // Create a unique ID for this tool execution
        let tool_id = uuid::Uuid::new_v4().to_string();
        
        // Send initial status update
        tx.send(Event::StatusUpdate(
            tool_id.clone(),
            format!("Executing tool: {}", tool_name),
        ))?;
        
        // Clone what we need for the task
        let tx_clone = tx.clone();
        let tool_name_clone = tool_name.clone();
        let params_clone = params.clone();
        
        // Create a runtime handle for running async code from sync context
        let rt = tokio::runtime::Handle::current();
        
        // Spawn a thread to execute the tool
        std::thread::spawn(move || {
            // Block on the async processing
            let _ = rt.block_on(async {
                // Here we would actually execute the tool
                // For now, just simulate with a delay
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Create a mock response based on the tool name
                let result = match tool_name_clone.as_str() {
                    "shell" => {
                        if let Some(cmd) = params_clone.get("command").and_then(|c| c.as_str()) {
                            Ok(format!("Executed command: {}\nOutput: Simulated output", cmd))
                        } else {
                            Err(anyhow::anyhow!("Missing command parameter"))
                        }
                    }
                    "file_read" => {
                        if let Some(path) = params_clone.get("path").and_then(|p| p.as_str()) {
                            Ok(format!("Read file: {}\nContent: Simulated file content", path))
                        } else {
                            Err(anyhow::anyhow!("Missing path parameter"))
                        }
                    }
                    _ => Err(anyhow::anyhow!("Unsupported tool: {}", tool_name_clone)),
                };
                
                // Send the result back to the main thread
                if let Err(e) = tx_clone.send(Event::ToolResult(tool_id, result)) {
                    error!("Failed to send tool result back to main thread: {}", e);
                }
            });
        });
        
        Ok(())
    }
    
    /// Simplified instance method that calls the static version
    pub fn execute_tool_instance(&self, tool_name: String, params: serde_json::Value) -> Result<()> {
        Self::execute_tool(
            self.tx.clone(),
            tool_name,
            params
        )
    }
}

/// Create a oneshot channel and return the sender and receiver
pub fn create_oneshot<T>() -> (oneshot::Sender<T>, oneshot::Receiver<T>) {
    oneshot::channel()
}
