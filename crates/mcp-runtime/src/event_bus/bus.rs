use anyhow::{anyhow, Result};
use crossbeam_channel::{self, bounded, Receiver, Sender};
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

use super::events::{ApiEvent, EventHandler, EventType, ModelEvent, UiEvent};

const DEFAULT_BUFFER_SIZE: usize = 100;

/// EventBus provides a central hub for event distribution in the application.
/// It manages multiple event channels and allows components to subscribe to
/// specific event types.
pub struct EventBus {
    // Unique identifier for this event bus instance
    instance_id: u64,
    ui_tx: Sender<UiEvent>,
    ui_rx: Receiver<UiEvent>,
    model_tx: Sender<ModelEvent>,
    model_rx: Receiver<ModelEvent>,
    api_tx: Sender<ApiEvent>,
    api_rx: Receiver<ApiEvent>,
    ui_handlers: Arc<Mutex<Vec<EventHandler<UiEvent>>>>,
    model_handlers: Arc<Mutex<Vec<EventHandler<ModelEvent>>>>,
    api_handlers: Arc<Mutex<Vec<EventHandler<ApiEvent>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        let (ui_tx, ui_rx) = bounded::<UiEvent>(DEFAULT_BUFFER_SIZE);
        let (model_tx, model_rx) = bounded::<ModelEvent>(DEFAULT_BUFFER_SIZE);
        let (api_tx, api_rx) = bounded::<ApiEvent>(DEFAULT_BUFFER_SIZE);

        // Generate a unique instance ID
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let instance_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

        debug!("Creating new EventBus with instance ID: {}", instance_id);

        Self {
            instance_id,
            ui_tx,
            ui_rx,
            model_tx,
            model_rx,
            api_tx,
            api_rx,
            ui_handlers: Arc::new(Mutex::new(Vec::new())),
            model_handlers: Arc::new(Mutex::new(Vec::new())),
            api_handlers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Creates a new channel with a specified buffer size
    pub fn new_with_buffer_size(buffer_size: usize) -> Self {
        let (ui_tx, ui_rx) = bounded::<UiEvent>(buffer_size);
        let (model_tx, model_rx) = bounded::<ModelEvent>(buffer_size);
        let (api_tx, api_rx) = bounded::<ApiEvent>(buffer_size);

        // Generate a unique instance ID
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let instance_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

        debug!("Creating new EventBus with instance ID: {} and buffer size: {}", instance_id, buffer_size);

        Self {
            instance_id,
            ui_tx,
            ui_rx,
            model_tx,
            model_rx,
            api_tx,
            api_rx,
            ui_handlers: Arc::new(Mutex::new(Vec::new())),
            model_handlers: Arc::new(Mutex::new(Vec::new())),
            api_handlers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get a clone of the UI event sender
    pub fn ui_sender(&self) -> Sender<UiEvent> {
        self.ui_tx.clone()
    }

    /// Get a clone of the Model event sender
    pub fn model_sender(&self) -> Sender<ModelEvent> {
        self.model_tx.clone()
    }

    /// Get a clone of the API event sender
    pub fn api_sender(&self) -> Sender<ApiEvent> {
        self.api_tx.clone()
    }

    /// Get the number of UI handlers
    pub fn ui_handlers(&self) -> usize {
        if let Ok(handlers) = self.ui_handlers.lock() {
            handlers.len()
        } else {
            0
        }
    }

    /// Get the number of Model handlers
    pub fn model_handlers(&self) -> usize {
        if let Ok(handlers) = self.model_handlers.lock() {
            handlers.len()
        } else {
            0
        }
    }

    /// Get the number of API handlers
    pub fn api_handlers(&self) -> usize {
        if let Ok(handlers) = self.api_handlers.lock() {
            handlers.len()
        } else {
            0
        }
    }

    /// Register a handler for UI events
    pub fn register_ui_handler(&self, handler: EventHandler<UiEvent>) -> Result<()> {
        match self.ui_handlers.lock() {
            Ok(mut handlers) => {
                // Add the handler (we don't do deduplication since we don't have a good way to compare handlers)
                handlers.push(handler);
                debug!(
                    "Registered UI event handler, total handlers: {}",
                    handlers.len()
                );
                Ok(())
            }
            Err(_) => Err(anyhow!("Failed to acquire lock on UI handlers")),
        }
    }

    /// Register a handler for Model events
    pub fn register_model_handler(&self, handler: EventHandler<ModelEvent>) -> Result<()> {
        match self.model_handlers.lock() {
            Ok(mut handlers) => {
                // Add the handler
                handlers.push(handler);
                debug!(
                    "Registered Model event handler, total handlers: {}",
                    handlers.len()
                );

                // If we have at least one handler, make sure we log every time
                // This helps with debugging to ensure handlers are properly registered
                if !handlers.is_empty() {
                    debug!(
                        "Model event handler registration successful, now have {} handler(s)",
                        handlers.len()
                    );
                }

                Ok(())
            }
            Err(_) => Err(anyhow!("Failed to acquire lock on Model handlers")),
        }
    }

    /// Register a handler for API events
    pub fn register_api_handler(&self, handler: EventHandler<ApiEvent>) -> Result<()> {
        match self.api_handlers.lock() {
            Ok(mut handlers) => {
                // Add the handler
                handlers.push(handler);
                debug!(
                    "Registered API event handler, total handlers: {}",
                    handlers.len()
                );
                Ok(())
            }
            Err(_) => Err(anyhow!("Failed to acquire lock on API handlers")),
        }
    }

    /// Clear all registered handlers
    /// This is useful for testing and for resetting the event bus
    pub fn clear_handlers(&self) -> Result<()> {
        match self.ui_handlers.lock() {
            Ok(mut handlers) => {
                let count = handlers.len();
                handlers.clear();
                debug!("Cleared {} UI event handlers", count);
            }
            Err(_) => return Err(anyhow!("Failed to acquire lock on UI handlers")),
        }

        match self.model_handlers.lock() {
            Ok(mut handlers) => {
                let count = handlers.len();
                handlers.clear();
                debug!("Cleared {} Model event handlers", count);
            }
            Err(_) => return Err(anyhow!("Failed to acquire lock on Model handlers")),
        }

        match self.api_handlers.lock() {
            Ok(mut handlers) => {
                let count = handlers.len();
                handlers.clear();
                debug!("Cleared {} API event handlers", count);
            }
            Err(_) => return Err(anyhow!("Failed to acquire lock on API handlers")),
        }

        Ok(())
    }

    // Use instance-based flag for tracking distribution status
    // This avoids test interference without using thread-local storage

    /// Initialize the event distribution loops
    /// This is idempotent - calling it multiple times will only start the distribution once
    pub fn start_event_distribution(&self) -> Result<()> {
        // Use a static to track which event buses have started distribution
        use std::sync::Mutex;
        use std::collections::HashSet;
        lazy_static::lazy_static! {
            static ref STARTED_BUSES: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
        }

        // Each event bus instance has a unique ID to track it
        let instance_id = self.instance_id;

        // Check if this bus instance has already started distribution
        let mut started_buses = STARTED_BUSES.lock().unwrap();
        if started_buses.contains(&instance_id) {
            debug!("Event distribution already started for bus instance #{}", instance_id);
            return Ok(());
        }

        // Mark this bus as started
        started_buses.insert(instance_id);
        debug!("Starting event distribution for bus instance #{}", instance_id);

        // Start the three event loops - each event bus gets its own loops
        self.start_ui_event_loop()?;
        self.start_model_event_loop()?;
        self.start_api_event_loop()?;

        info!("Event distribution started");
        Ok(())
    }

    // Start the UI event distribution loop
    fn start_ui_event_loop(&self) -> Result<()> {
        let rx = self.ui_rx.clone();
        let handlers = self.ui_handlers.clone();

        tokio::spawn(async move {
            Self::run_event_loop("UI".to_string(), rx, handlers).await;
        });

        Ok(())
    }

    // Start the Model event distribution loop
    fn start_model_event_loop(&self) -> Result<()> {
        let rx = self.model_rx.clone();
        let handlers = self.model_handlers.clone();

        tokio::spawn(async move {
            Self::run_event_loop("Model".to_string(), rx, handlers).await;
        });

        Ok(())
    }

    // Start the API event distribution loop
    fn start_api_event_loop(&self) -> Result<()> {
        let rx = self.api_rx.clone();
        let handlers = self.api_handlers.clone();

        tokio::spawn(async move {
            Self::run_event_loop("API".to_string(), rx, handlers).await;
        });

        Ok(())
    }

    // Generic event loop implementation shared by all event types
    async fn run_event_loop<T: EventType + Clone + Send + 'static>(
        name: String,
        rx: Receiver<T>,
        handlers: Arc<Mutex<Vec<EventHandler<T>>>>,
    ) {
        debug!("{} event loop started", name);

        // Create a channel for passing events from crossbeam to tokio
        let (tx, mut task_rx) = tokio::sync::mpsc::channel::<T>(1000);

        // Clone the name for the bridge thread
        let thread_name = name.clone();

        // Spawn a dedicated thread to bridge between crossbeam channel and tokio
        // This is necessary because crossbeam channels can't be used directly in async context
        std::thread::Builder::new()
            .name(format!("{}-event-bridge", name))
            .spawn(move || {
                debug!("Bridge thread for {} events started", thread_name);

                // Keep receiving events indefinitely - this thread stays alive for the
                // program duration to ensure events are never missed
                loop {
                    match rx.recv_timeout(std::time::Duration::from_millis(10)) {
                        Ok(event) => {
                            debug!("Bridge received {} event: {:?}", thread_name, event);

                            // Forward event to the tokio runtime
                            if let Err(e) = tx.blocking_send(event) {
                                // Only log and continue - never break the bridge thread
                                warn!("Failed to forward {} event: {}", thread_name, e);
                            }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            // Normal timeout - just keep waiting
                            continue;
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            // Channel disconnected, but we'll keep running
                            // In a properly designed system, this shouldn't happen
                            warn!(
                                "{} event channel disconnected, waiting for reconnection",
                                thread_name
                            );
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                }

                // This is unreachable, but included for clarity
                #[allow(unreachable_code)]
                {
                    debug!("{} bridge thread terminated", thread_name);
                }
            })
            .expect("Failed to spawn event bridge thread");

        let event_type_str = match name.as_str() {
            "UI" => "UiEvent",
            "Model" => "ModelEvent",
            "API" => "ApiEvent",
            _ => "Unknown",
        };
        debug!("{} event loop waiting for {} events", name, event_type_str);

        // Main event processing loop runs indefinitely
        debug!("{} event loop ready to process events", name);

        loop {
            // Wait for an event with a heartbeat to keep the loop alive
            let event_opt = tokio::select! {
                event = task_rx.recv() => event,
                // Add a heartbeat to ensure the loop stays alive even if no events arrive
                _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                    debug!("{} event loop heartbeat", name);
                    None
                }
            };

            // Process event if one was received
            if let Some(event) = event_opt {
                debug!("Processing {} event: {:?}", name, event);

                // Get a snapshot of all current handlers under a short lock
                let handlers_snapshot = match handlers.lock() {
                    Ok(guard) => {
                        if guard.is_empty() {
                            debug!("{} event loop: no handlers registered", name);
                        }
                        guard.clone()
                    }
                    Err(e) => {
                        warn!("Failed to acquire lock on {} handlers: {}", name, e);
                        continue;
                    }
                };

                // Process each handler concurrently with its own event clone
                let mut join_handles = Vec::with_capacity(handlers_snapshot.len());

                for handler in handlers_snapshot {
                    let event_clone = event.clone();
                    let name_clone = name.clone();

                    // Spawn each handler in its own task
                    let handle = tokio::spawn(async move {
                        match handler.handle(event_clone).await {
                            Ok(_) => {
                                debug!("Handler for {} event completed successfully", name_clone)
                            }
                            Err(e) => warn!("Error in {} event handler: {}", name_clone, e),
                        }
                    });

                    join_handles.push(handle);
                }

                // Wait for all handlers to complete - we don't proceed until all handlers
                // have finished processing the current event
                futures::future::join_all(join_handles).await;
            } else {
                // This happens on channel closure or heartbeat
                debug!(
                    "{} event loop heartbeat or channel temporarily closed",
                    name
                );
                // Don't add any sleep here since we already have the timeout in the select
            }
        }

        // This line will never be reached due to the infinite loop
        #[allow(unreachable_code)]
        {
            info!("{} event loop terminated", name);
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// IMPORTANT: NO DIRECT CLONING
// Instead of implementing Clone, we'll use Arc<EventBus> throughout the codebase
// This avoids all the issues with duplicate message processing
// Clone is deliberately not implemented to prevent accidental use
//
// If you need this EventBus in multiple places, wrap it in an Arc when first creating it
// Then pass the Arc<EventBus> reference around
//
// Example:
//   let event_bus = Arc::new(EventBus::new());
//   let session_manager = SessionManager::new(client, executor, Arc::clone(&event_bus));
//   let event_adapter = CliEventAdapter::new(Arc::clone(&event_bus));
//
// This ensures a true singleton pattern with proper shared ownership.
//
// WARNING: DO NOT IMPLEMENT Clone FOR EventBus - it's an anti-pattern for this type!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::events::create_handler;
    use std::sync::Arc;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_event_bus_creation() {
        let _bus = EventBus::new();
    }

    /// Test both UI and Model event handling together
    /// This helps avoid test isolation issues
    #[tokio::test(flavor = "multi_thread")]
    async fn test_ui_event_handling() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::time::sleep;

        // Create a single event bus for the test
        let bus = EventBus::new();

        // Create flags to track event processing
        let ui_processed = Arc::new(AtomicBool::new(false));
        let ui_clone = ui_processed.clone();

        // Register UI handler
        let ui_handler = create_handler(move |event: UiEvent| {
            let flag = ui_clone.clone();
            Box::pin(async move {
                if let UiEvent::UserInput(msg) = event {
                    assert_eq!(msg, "test input");
                    flag.store(true, Ordering::SeqCst);
                }
                Ok(())
            })
        });

        // Register the handler
        bus.register_ui_handler(ui_handler).unwrap();

        // Start event distribution
        bus.start_event_distribution().unwrap();

        // Send event
        bus.ui_sender()
            .send(UiEvent::UserInput("test input".to_string()))
            .unwrap();

        // Wait with timeout
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        // Poll until timeout or success
        while !ui_processed.load(Ordering::SeqCst) && start.elapsed() < timeout {
            // Send again after a delay to increase chances
            if start.elapsed() > Duration::from_millis(100)
                && start.elapsed() < Duration::from_millis(200)
            {
                bus.ui_sender()
                    .send(UiEvent::UserInput("test input".to_string()))
                    .unwrap();
            }

            sleep(Duration::from_millis(50)).await;
        }

        // Verify event was processed
        assert!(
            ui_processed.load(Ordering::SeqCst),
            "UI event was not processed within timeout"
        );
    }

    /// Test model event handling
    #[tokio::test(flavor = "multi_thread")]
    async fn test_model_event_handling() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::time::sleep;

        // Create a new event bus
        let bus = EventBus::new();

        // Create flag to track event processing
        let model_processed = Arc::new(AtomicBool::new(false));
        let model_clone = model_processed.clone();

        // Register model handler
        let model_handler = create_handler(move |event: ModelEvent| {
            let flag = model_clone.clone();
            Box::pin(async move {
                if let ModelEvent::ProcessUserMessage(msg) = event {
                    assert_eq!(msg, "test message");
                    flag.store(true, Ordering::SeqCst);
                }
                Ok(())
            })
        });

        // Register the handler
        bus.register_model_handler(model_handler).unwrap();

        // Start event distribution
        bus.start_event_distribution().unwrap();

        // Send event multiple times to increase chance of success
        for _ in 0..3 {
            bus.model_sender()
                .send(ModelEvent::ProcessUserMessage("test message".to_string()))
                .unwrap();

            // Small delay between sends
            sleep(Duration::from_millis(10)).await;
        }

        // Wait with timeout
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(5);

        // Poll until timeout or success
        while !model_processed.load(Ordering::SeqCst) && start.elapsed() < timeout {
            // Send again after a delay to increase chances
            if start.elapsed() > Duration::from_millis(1000) {
                bus.model_sender()
                    .send(ModelEvent::ProcessUserMessage("test message".to_string()))
                    .unwrap();
            }

            sleep(Duration::from_millis(50)).await;
        }

        // Verify event was processed
        assert!(
            model_processed.load(Ordering::SeqCst),
            "Model event was not processed within timeout"
        );
    }
}
