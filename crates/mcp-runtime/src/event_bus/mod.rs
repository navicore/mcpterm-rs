mod bus;
mod events;

pub use bus::EventBus;
pub use events::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_event_creation() {
        let ui_event = UiEvent::UserInput("test input".to_string());
        let model_event = ModelEvent::ProcessUserMessage("test message".to_string());

        assert!(matches!(ui_event, UiEvent::UserInput(_)));
        assert!(matches!(model_event, ModelEvent::ProcessUserMessage(_)));
    }

    #[tokio::test]
    async fn test_basic_event_channel() {
        let (tx, rx) = unbounded::<UiEvent>();

        // Send a test event
        tx.send(UiEvent::Quit).unwrap();

        // Test receiving the event
        let event = rx.recv().unwrap();
        assert!(matches!(event, UiEvent::Quit));
    }
}
