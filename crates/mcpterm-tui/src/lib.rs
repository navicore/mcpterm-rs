pub mod ui;
pub mod state;
pub mod events;

use anyhow::Result;
use events::EventHandler;
use state::AppState;

pub struct App {
    pub state: AppState,
    event_handler: EventHandler,
}

impl App {
    pub fn new() -> Result<Self> {
        let state = AppState::new();
        let event_handler = EventHandler::new()?;
        
        Ok(Self {
            state,
            event_handler,
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        // Placeholder implementation
        // This would set up the terminal, render loop, etc.
        Ok(())
    }
}