use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{Event as CrosstermEvent, KeyEvent};
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};

#[derive(Debug, Clone)]
pub enum Event {
    Input(KeyEvent),
    Tick,
}

pub struct EventHandler {
    rx: Receiver<Event>,
    _tx: Sender<Event>,
}

impl EventHandler {
    pub fn new() -> Result<Self> {
        debug!("Initializing TUI event handler");
        let tick_rate = Duration::from_millis(100);
        let (tx, rx) = crossbeam_channel::unbounded();

        // Clone channel for event thread
        let event_tx = tx.clone();

        // Spawn input handling thread
        info!(
            "Spawning event handling thread with tick rate of {}ms",
            tick_rate.as_millis()
        );
        thread::spawn(move || {
            debug!("Event handling thread started");
            loop {
                // Poll for events with a small timeout
                if let Ok(true) = crossterm::event::poll(tick_rate) {
                    if let Ok(event) = crossterm::event::read() {
                        trace!("Received terminal event: {:?}", event);
                        match event {
                            CrosstermEvent::Key(key) => {
                                trace!("Processing key event: {:?}", key);
                                if let Err(e) = event_tx.send(Event::Input(key)) {
                                    // Channel closed, exit thread
                                    error!("Failed to send key event: {}", e);
                                    break;
                                }
                            }
                            // Other event types can be handled here
                            _ => {
                                trace!("Ignoring non-key event");
                            }
                        }
                    }
                }

                // Send tick event
                trace!("Sending tick event");
                if let Err(e) = event_tx.send(Event::Tick) {
                    // Channel closed, exit thread
                    error!("Failed to send tick event: {}", e);
                    break;
                }
            }
            warn!("Event handling thread exiting");
        });

        debug!("Event handler initialization complete");
        Ok(Self { rx, _tx: tx })
    }

    pub fn next(&self) -> Result<Event> {
        match self.rx.recv() {
            Ok(event) => {
                trace!("Next event received: {:?}", event);
                Ok(event)
            }
            Err(e) => {
                error!("Failed to receive event: {}", e);
                Err(e.into())
            }
        }
    }
}
