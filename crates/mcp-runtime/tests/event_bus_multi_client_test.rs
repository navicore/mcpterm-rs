use anyhow::Result;
use mcp_runtime::event_bus::events::create_handler;
use mcp_runtime::event_bus::{ApiEvent, EventBus, ModelEvent, UiEvent};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::timeout;

/// This test demonstrates a clean pattern for a three-client event bus architecture
/// where events flow correctly between different components.

#[tokio::test(flavor = "multi_thread")]
async fn test_three_client_event_bus() -> Result<()> {
    // Create a single shared event bus
    let event_bus = Arc::new(EventBus::new());

    // Create counters to track received events
    let ui_events = Arc::new(Mutex::new(Vec::new()));
    let model_events = Arc::new(Mutex::new(Vec::new()));
    let api_events = Arc::new(Mutex::new(Vec::new()));

    // Client 1: UI Component
    let ui_component = {
        let event_bus = event_bus.clone();
        let ui_events = ui_events.clone();
        // Model events reference not used in this closure, but helpful for structure/symmetry
        let _model_events = model_events.clone();

        // Set up UI handler for model events (receives model responses)
        let model_handler = {
            let ui_events = ui_events.clone();
            create_handler(move |event: ModelEvent| {
                let ui_events = ui_events.clone();
                Box::pin(async move {
                    if let ModelEvent::LlmMessage(msg) = &event {
                        let mut events = ui_events.lock().unwrap();
                        events.push(format!("UI received model message: {}", msg));
                    }
                    Ok(())
                })
            })
        };

        // Register handler
        event_bus.register_model_handler(model_handler)?;

        // Return function to send UI events
        move |input: &str| -> Result<()> {
            let mut events = ui_events.lock().unwrap();
            events.push(format!("UI sent: {}", input));

            event_bus
                .ui_sender()
                .send(UiEvent::UserInput(input.to_string()))?;
            Ok(())
        }
    };

    // Client 2: Model Component (this is a closure that can be called later)
    let _model_component = {
        let event_bus = event_bus.clone();
        let model_events = model_events.clone();

        // Set up Model handler for UI events (receives user input)
        let ui_handler = {
            let model_events = model_events.clone();
            let event_bus = event_bus.clone();

            create_handler(move |event: UiEvent| {
                let model_events = model_events.clone();
                let event_bus = event_bus.clone();

                Box::pin(async move {
                    if let UiEvent::UserInput(input) = &event {
                        let mut events = model_events.lock().unwrap();
                        events.push(format!("Model received: {}", input));

                        // Simulate model processing and sending a response
                        let response = format!("Response to: {}", input);
                        event_bus
                            .model_sender()
                            .send(ModelEvent::LlmMessage(response))?;
                    }
                    Ok(())
                })
            })
        };

        // Register handler
        event_bus.register_ui_handler(ui_handler)?;

        // Return function that can be used to send model events directly
        move |response: &str| -> Result<()> {
            let mut events = model_events.lock().unwrap();
            events.push(format!("Model sent: {}", response));

            event_bus
                .model_sender()
                .send(ModelEvent::LlmMessage(response.to_string()))?;
            Ok(())
        }
    };

    // Client 3: API Component (this is a closure that can be called later)
    let _api_component = {
        let event_bus = event_bus.clone();
        let api_events = api_events.clone();

        // We'll make the API component listen to both UI and Model events
        let ui_handler = {
            let api_events = api_events.clone();
            create_handler(move |event: UiEvent| {
                let api_events = api_events.clone();
                Box::pin(async move {
                    if let UiEvent::UserInput(input) = &event {
                        let mut events = api_events.lock().unwrap();
                        events.push(format!("API log - UI input: {}", input));
                    }
                    Ok(())
                })
            })
        };

        let model_handler = {
            let api_events = api_events.clone();
            create_handler(move |event: ModelEvent| {
                let api_events = api_events.clone();
                Box::pin(async move {
                    if let ModelEvent::LlmMessage(msg) = &event {
                        let mut events = api_events.lock().unwrap();
                        events.push(format!("API log - Model response: {}", msg));
                    }
                    Ok(())
                })
            })
        };

        // Register handlers
        event_bus.register_ui_handler(ui_handler)?;
        event_bus.register_model_handler(model_handler)?;

        // Return function to send API events
        move |message: &str| -> Result<()> {
            let mut events = api_events.lock().unwrap();
            events.push(format!("API sent: {}", message));

            event_bus
                .api_sender()
                .send(ApiEvent::SendRequest(message.to_string()))?;
            Ok(())
        }
    };

    // Start the event distribution
    event_bus.start_event_distribution()?;

    // Run a flow through the system
    // Send a user message
    ui_component("Hello, world!")?;

    // Wait a short time to allow event processing to complete (this is for the test only)
    // In a real application, we'd use proper async patterns
    let max_duration = Duration::from_secs(2);

    // Use tokio's timeout for an async-friendly wait
    let result = timeout(max_duration, async {
        loop {
            // Check if we have the expected number of events
            let ui_count = ui_events.lock().unwrap().len();
            let model_count = model_events.lock().unwrap().len();
            let api_count = api_events.lock().unwrap().len();

            // We expect:
            // 1. UI event sent
            // 2. Model received user input
            // 3. UI received model response
            // 4-5. API logged both UI input and model response
            if ui_count >= 2 && model_count >= 1 && api_count >= 2 {
                break;
            }

            // Short yield to allow other tasks to run
            tokio::task::yield_now().await;
        }
    })
    .await;

    match result {
        Ok(_) => {
            // Print out the events for verification
            println!("UI Events:");
            for event in ui_events.lock().unwrap().iter() {
                println!("  {}", event);
            }

            println!("Model Events:");
            for event in model_events.lock().unwrap().iter() {
                println!("  {}", event);
            }

            println!("API Events:");
            for event in api_events.lock().unwrap().iter() {
                println!("  {}", event);
            }

            // Add assertions to verify correct event flow
            assert!(
                ui_events.lock().unwrap().len() >= 2,
                "UI should have registered at least 2 events"
            );
            assert!(
                model_events.lock().unwrap().len() >= 1,
                "Model should have registered at least 1 event"
            );
            assert!(
                api_events.lock().unwrap().len() >= 2,
                "API should have registered at least 2 events"
            );

            Ok(())
        }
        Err(_) => {
            // Print out what events were received before timeout
            println!("TEST TIMED OUT. Events received so far:");

            println!("UI Events: {}", ui_events.lock().unwrap().len());
            for event in ui_events.lock().unwrap().iter() {
                println!("  {}", event);
            }

            println!("Model Events: {}", model_events.lock().unwrap().len());
            for event in model_events.lock().unwrap().iter() {
                println!("  {}", event);
            }

            println!("API Events: {}", api_events.lock().unwrap().len());
            for event in api_events.lock().unwrap().iter() {
                println!("  {}", event);
            }

            anyhow::bail!("Test timed out waiting for events to propagate")
        }
    }
}
