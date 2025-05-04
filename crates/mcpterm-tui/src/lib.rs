pub mod events;
pub mod state;
pub mod ui;

use anyhow::Result;
use events::EventHandler;
use mcp_metrics::{LogDestination, MetricsDestination, MetricsRegistry};
use state::AppState;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

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

        // Placeholder implementation
        // This would set up the terminal, render loop, etc.
        Ok(())
    }
}
