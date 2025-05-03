#[cfg(test)]
mod tests {
    use mcp_runtime::event_bus::{EventBus, UiEvent, ModelEvent};
    
    #[test]
    fn test_event_passing() {
        let (ui_tx, ui_rx) = EventBus::new_ui_channel();
        let (model_tx, model_rx) = EventBus::new_model_channel();
        
        // Send a UI event
        ui_tx.send(UiEvent::UserInput("test".to_string())).unwrap();
        
        // Receive the UI event
        let event = ui_rx.recv().unwrap();
        if let UiEvent::UserInput(content) = event {
            assert_eq!(content, "test");
            
            // Send a model event in response
            model_tx.send(ModelEvent::ProcessUserMessage(content)).unwrap();
        }
        
        // Receive the model event
        let model_event = model_rx.recv().unwrap();
        if let ModelEvent::ProcessUserMessage(content) = model_event {
            assert_eq!(content, "test");
        } else {
            panic!("Expected ProcessUserMessage");
        }
    }
}