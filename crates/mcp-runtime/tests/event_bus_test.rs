use anyhow::Result;
use mcp_runtime::{
    create_handler, ApiEvent, EventBus, KeyCode, KeyEvent, KeyModifiers, ModelEvent, UiEvent,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_event_bus_multi_handler() -> Result<()> {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // Register two handlers for the same event type
    for _ in 0..2 {
        let counter_clone = counter.clone();
        let handler = create_handler(move |event: UiEvent| {
            let counter = counter_clone.clone();
            Box::pin(async move {
                if let UiEvent::UserInput(_) = event {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                Ok(())
            })
        });
        bus.register_ui_handler(handler)?;
    }

    bus.start_event_distribution()?;

    // Send an event that both handlers should process
    bus.ui_sender()
        .send(UiEvent::UserInput("test".to_string()))?;

    // Give handlers time to process
    sleep(Duration::from_millis(50)).await;

    // Both handlers should have processed the event
    assert_eq!(counter.load(Ordering::SeqCst), 2);

    Ok(())
}

#[tokio::test]
async fn test_event_bus_cross_channel() -> Result<()> {
    let bus = EventBus::new();
    let ui_received = Arc::new(AtomicUsize::new(0));
    let model_received = Arc::new(AtomicUsize::new(0));

    // Register UI handler
    let model_tx = bus.model_sender();
    let ui_received_clone = ui_received.clone();
    let ui_handler = create_handler(move |event: UiEvent| {
        let model_tx = model_tx.clone();
        let ui_received = ui_received_clone.clone();
        Box::pin(async move {
            if let UiEvent::UserInput(msg) = event {
                ui_received.fetch_add(1, Ordering::SeqCst);
                // Forward to model channel
                model_tx.send(ModelEvent::ProcessUserMessage(msg)).unwrap();
            }
            Ok(())
        })
    });
    bus.register_ui_handler(ui_handler)?;

    // Register Model handler
    let model_received_clone = model_received.clone();
    let model_handler = create_handler(move |event: ModelEvent| {
        let model_received = model_received_clone.clone();
        Box::pin(async move {
            if let ModelEvent::ProcessUserMessage(_) = event {
                model_received.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        })
    });
    bus.register_model_handler(model_handler)?;

    bus.start_event_distribution()?;

    // Send a UI event that should be forwarded to the model channel
    bus.ui_sender()
        .send(UiEvent::UserInput("test message".to_string()))?;

    // Give handlers time to process
    sleep(Duration::from_millis(50)).await;

    // Both handlers should have processed their respective events
    assert_eq!(ui_received.load(Ordering::SeqCst), 1);
    assert_eq!(model_received.load(Ordering::SeqCst), 1);

    Ok(())
}

#[tokio::test]
async fn test_event_bus_error_handling() -> Result<()> {
    let bus = EventBus::new();
    let success_counter = Arc::new(AtomicUsize::new(0));

    // Register a handler that will succeed
    let success_counter_clone = success_counter.clone();
    let success_handler = create_handler(move |_: UiEvent| {
        let counter = success_counter_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    });
    bus.register_ui_handler(success_handler)?;

    // Register a handler that will fail
    let error_handler = create_handler(move |_: UiEvent| {
        Box::pin(async move { Err(anyhow::anyhow!("Simulated error")) })
    });
    bus.register_ui_handler(error_handler)?;

    bus.start_event_distribution()?;

    // Send an event
    bus.ui_sender()
        .send(UiEvent::UserInput("test".to_string()))?;

    // Give handlers time to process
    sleep(Duration::from_millis(50)).await;

    // The successful handler should have processed the event
    assert_eq!(success_counter.load(Ordering::SeqCst), 1);

    // The error in the second handler should not have affected the first
    Ok(())
}

#[tokio::test]
async fn test_key_event_handling() -> Result<()> {
    let bus = EventBus::new();
    let key_counter = Arc::new(AtomicUsize::new(0));

    // Register handler for key events
    let key_counter_clone = key_counter.clone();
    let key_handler = create_handler(move |event: UiEvent| {
        let counter = key_counter_clone.clone();
        Box::pin(async move {
            if let UiEvent::KeyPress(key) = event {
                if key.code.is_enter() && key.modifiers.ctrl {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            }
            Ok(())
        })
    });
    bus.register_ui_handler(key_handler)?;

    bus.start_event_distribution()?;

    // Send a key event with Ctrl+Enter
    let key_event = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers {
            ctrl: true,
            alt: false,
            shift: false,
        },
    };

    bus.ui_sender().send(UiEvent::KeyPress(key_event))?;

    // Give handlers time to process
    sleep(Duration::from_millis(50)).await;

    // The handler should have processed the key event
    assert_eq!(key_counter.load(Ordering::SeqCst), 1);

    Ok(())
}
