use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{Event as CrosstermEvent, KeyEvent};
use std::thread;
use std::time::Duration;

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
        let tick_rate = Duration::from_millis(100);
        let (tx, rx) = crossbeam_channel::unbounded();
        
        // Clone channel for event thread
        let event_tx = tx.clone();
        
        // Spawn input handling thread
        thread::spawn(move || {
            loop {
                // Poll for events with a small timeout
                if let Ok(true) = crossterm::event::poll(tick_rate) {
                    if let Ok(event) = crossterm::event::read() {
                        match event {
                            CrosstermEvent::Key(key) => {
                                if let Err(_) = event_tx.send(Event::Input(key)) {
                                    // Channel closed, exit thread
                                    break;
                                }
                            }
                            // Other event types can be handled here
                            _ => {}
                        }
                    }
                }
                
                // Send tick event
                if let Err(_) = event_tx.send(Event::Tick) {
                    // Channel closed, exit thread
                    break;
                }
            }
        });
        
        Ok(Self {
            rx,
            _tx: tx,
        })
    }
    
    pub fn next(&self) -> Result<Event> {
        Ok(self.rx.recv()?)
    }
}