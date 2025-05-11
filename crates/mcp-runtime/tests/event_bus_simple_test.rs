use anyhow::Result;
use mcp_runtime::event_bus::events::create_handler;
use mcp_runtime::event_bus::{ApiEvent, EventBus, ModelEvent, UiEvent};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::time::sleep;

// This test checks basic event flow with a simpler approach than the multi-client test
#[tokio::test(flavor = "multi_thread")]
async fn test_basic_event_bus_functionality() -> Result<()> {
    // Create a single shared event bus
    let event_bus = Arc::new(EventBus::new());

    // Create a flag to track UI event processing
    let ui_event_processed = Arc::new(AtomicBool::new(false));
    let ui_event_clone = ui_event_processed.clone();

    // Create a flag to track Model event processing
    let model_event_processed = Arc::new(AtomicBool::new(false));
    let model_event_clone = model_event_processed.clone();

    // Register a UI event handler that simply sets the flag
    let ui_handler = create_handler(move |event: UiEvent| {
        let flag = ui_event_clone.clone();
        Box::pin(async move {
            if let UiEvent::UserInput(input) = event {
                assert_eq!(input, "ui test");
                flag.store(true, Ordering::SeqCst);
            }
            Ok(())
        })
    });

    // Register a Model event handler that simply sets the flag
    let model_handler = create_handler(move |event: ModelEvent| {
        let flag = model_event_clone.clone();
        Box::pin(async move {
            if let ModelEvent::ProcessUserMessage(msg) = event {
                assert_eq!(msg, "model test");
                flag.store(true, Ordering::SeqCst);
            }
            Ok(())
        })
    });

    // Register the handlers
    event_bus.register_ui_handler(ui_handler)?;
    event_bus.register_model_handler(model_handler)?;

    // Start event distribution
    event_bus.start_event_distribution()?;

    // Short delay to ensure event processing is ready
    sleep(Duration::from_millis(100)).await;

    // Send both UI and Model events
    event_bus
        .ui_sender()
        .send(UiEvent::UserInput("ui test".to_string()))?;
    event_bus
        .model_sender()
        .send(ModelEvent::ProcessUserMessage("model test".to_string()))?;

    // Wait with timeout for events to be processed
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);

    // Poll for up to 5 seconds
    while (!ui_event_processed.load(Ordering::SeqCst)
        || !model_event_processed.load(Ordering::SeqCst))
        && start.elapsed() < timeout
    {
        // Periodically send events again to increase chances of success
        if start.elapsed() > Duration::from_secs(1) && start.elapsed() < Duration::from_secs(2) {
            println!("Sending events again");
            event_bus
                .ui_sender()
                .send(UiEvent::UserInput("ui test".to_string()))?;
            event_bus
                .model_sender()
                .send(ModelEvent::ProcessUserMessage("model test".to_string()))?;
        }

        // Sleep briefly to avoid tight polling
        sleep(Duration::from_millis(10)).await;
    }

    // Check if both events were processed
    let ui_processed = ui_event_processed.load(Ordering::SeqCst);
    let model_processed = model_event_processed.load(Ordering::SeqCst);

    assert!(ui_processed, "UI event was not processed within timeout");
    assert!(
        model_processed,
        "Model event was not processed within timeout"
    );

    Ok(())
}
