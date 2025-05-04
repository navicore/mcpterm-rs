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

        Self {
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

        Self {
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

    /// Register a handler for UI events
    pub fn register_ui_handler(&self, handler: EventHandler<UiEvent>) -> Result<()> {
        match self.ui_handlers.lock() {
            Ok(mut handlers) => {
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
                handlers.push(handler);
                debug!(
                    "Registered Model event handler, total handlers: {}",
                    handlers.len()
                );
                Ok(())
            }
            Err(_) => Err(anyhow!("Failed to acquire lock on Model handlers")),
        }
    }

    /// Register a handler for API events
    pub fn register_api_handler(&self, handler: EventHandler<ApiEvent>) -> Result<()> {
        match self.api_handlers.lock() {
            Ok(mut handlers) => {
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

    /// Initialize the event distribution loops
    pub fn start_event_distribution(&self) -> Result<()> {
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

        // Use a tokio task to receive events from crossbeam channel
        let (tx, mut task_rx) = tokio::sync::mpsc::channel::<T>(100);

        // Spawn a task to bridge crossbeam and tokio channels
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if tx.blocking_send(event).is_err() {
                    break;
                }
            }
        });

        while let Some(event) = task_rx.recv().await {
            debug!("Received {} event: {:?}", name, event);

            // Clone all handlers to avoid holding the lock during async work
            let cloned_handlers = {
                match handlers.lock() {
                    Ok(guard) => guard.clone(),
                    Err(e) => {
                        warn!("Failed to acquire lock on {} handlers: {}", name, e);
                        continue;
                    }
                }
            };

            // Process each handler with its own event clone
            let mut join_handles = Vec::new();
            for handler in cloned_handlers {
                let event_clone = event.clone();
                let name_clone = name.clone();

                let handle = tokio::spawn(async move {
                    if let Err(e) = handler.handle(event_clone).await {
                        warn!("Error in {} event handler: {}", name_clone, e);
                    }
                });
                join_handles.push(handle);
            }

            // Wait for all handlers to complete
            for handle in join_handles {
                if let Err(e) = handle.await {
                    warn!("Handler task failed: {}", e);
                }
            }
        }

        info!("{} event loop terminated", name);
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::events::create_handler;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_event_bus_creation() {
        let _bus = EventBus::new();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_ui_event_handling() {
        let bus = EventBus::new();
        let event_processed = Arc::new(AtomicBool::new(false));
        let event_processed_clone = event_processed.clone();

        let handler = create_handler(move |event: UiEvent| {
            let event_processed = event_processed_clone.clone();
            Box::pin(async move {
                if let UiEvent::UserInput(input) = event {
                    assert_eq!(input, "test input");
                    event_processed.store(true, Ordering::SeqCst);
                }
                Ok(())
            })
        });

        bus.register_ui_handler(handler).unwrap();
        bus.start_event_distribution().unwrap();

        // Send a test event
        bus.ui_sender()
            .send(UiEvent::UserInput("test input".to_string()))
            .unwrap();

        // Set a timeout for the test
        let timeout = Duration::from_secs(1);
        let start = std::time::Instant::now();

        // Wait for event processing or timeout
        while !event_processed.load(Ordering::SeqCst) && start.elapsed() < timeout {
            sleep(Duration::from_millis(10)).await;
        }

        assert!(
            event_processed.load(Ordering::SeqCst),
            "Event was not processed within timeout"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_model_event_handling() {
        let bus = EventBus::new();
        let event_processed = Arc::new(AtomicBool::new(false));
        let event_processed_clone = event_processed.clone();

        let handler = create_handler(move |event: ModelEvent| {
            let event_processed = event_processed_clone.clone();
            Box::pin(async move {
                if let ModelEvent::ProcessUserMessage(msg) = event {
                    assert_eq!(msg, "test message");
                    event_processed.store(true, Ordering::SeqCst);
                }
                Ok(())
            })
        });

        bus.register_model_handler(handler).unwrap();
        bus.start_event_distribution().unwrap();

        // Send a test event
        bus.model_sender()
            .send(ModelEvent::ProcessUserMessage("test message".to_string()))
            .unwrap();

        // Set a timeout for the test
        let timeout = Duration::from_secs(1);
        let start = std::time::Instant::now();

        // Wait for event processing or timeout
        while !event_processed.load(Ordering::SeqCst) && start.elapsed() < timeout {
            sleep(Duration::from_millis(10)).await;
        }

        assert!(
            event_processed.load(Ordering::SeqCst),
            "Event was not processed within timeout"
        );
    }
}
