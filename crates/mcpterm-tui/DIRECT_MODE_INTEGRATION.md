# Direct Mode Integration Plan

This document outlines a plan for fully integrating the direct key handling approach with the rest of the application.

## Current Status

We've created a direct implementation of the terminal UI that handles keyboard input more reliably by bypassing the complex event system. This implementation:

1. Correctly handles Tab key for focus switching
2. Properly handles j/k keys for message scrolling
3. Shows the cursor in the right position
4. Provides auto-scrolling for messages

The current implementation is available with the `--direct-mode` flag but lacks integration with the LLM client.

## Integration Goals

1. Keep the direct key handling approach for improved keyboard behavior
2. Integrate with the LLM client for message processing
3. Preserve tool execution capabilities
4. Maintain history and context management

## Integration Approach

### Short-term Solution (Current Implementation)

The current implementation (`--direct-mode`) provides an immediate solution for users experiencing keyboard input issues. It provides a fully functional UI with all UI features except LLM integration.

### Medium-term Solution

1. Modify `direct_impl.rs` to create and use an EventHandler instance
2. Add a minimal event loop to handle LLM responses
3. Integrate tool execution

```rust
pub fn run_direct_ui() -> Result<()> {
    // ... existing setup code ...
    
    // Create and initialize event handler for LLM integration
    let event_handler = Arc::new(events::EventHandler::new()?);
    
    // Main loop
    while state.running {
        // Render UI
        terminal.draw(|f| render_ui(f, &mut state))?;
        
        // Check for events from the event handler
        if let Ok(event) = event_handler.rx.try_recv() {
            match event {
                events::Event::LlmResponse(request, result) => {
                    state.process_llm_response(result);
                },
                events::Event::ToolResult(tool_id, result) => {
                    // Process tool result
                },
                events::Event::StatusUpdate(id, status) => {
                    state.update_processing_status(status);
                },
                _ => {}
            }
        }
        
        // Handle direct keyboard input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if handle_submit_with_llm(&mut state, key, event_handler.clone()) {
                        continue;
                    }
                    
                    // Normal direct key handling
                    handle_key(&mut state, key);
                }
            }
        }
    }
    
    // ... existing cleanup code ...
}

// New function to handle input submission with LLM integration
fn handle_submit_with_llm(state: &mut AppState, key: KeyEvent, event_handler: Arc<EventHandler>) -> bool {
    // Check if this is a submission key event
    let is_submit = match (state.focus, state.editor_mode, key.code) {
        (FocusArea::Input, EditorMode::Normal, KeyCode::Enter) => true,
        (FocusArea::Input, EditorMode::Insert, KeyCode::Enter) => true,
        _ => false,
    };
    
    if is_submit && !state.input_content.is_empty() {
        // Submit input
        if let Some(input) = state.submit_input() {
            // Process with LLM
            if let Err(e) = events::EventHandler::process_message(
                event_handler.tx.clone(),
                event_handler.llm_client.is_some(),
                event_handler.pending_requests.clone(),
                input,
                state.context.clone()
            ) {
                state.add_message(
                    format!("Error processing message: {}", e),
                    MessageType::Error,
                );
            }
        }
        return true;
    }
    
    false
}
```

### Long-term Solution

1. Rewrite the main application to use the direct key handling approach
2. Eliminate the complex event system for key handling
3. Keep the event system only for async operations (LLM, tools)
4. Ensure that all UI components directly update the application state

## Recommendations

1. Implement the medium-term solution first to provide a fully functional direct mode
2. Gather feedback from users on the direct mode implementation
3. Consider moving to the long-term solution in a future release

## Benefits

1. More reliable keyboard input handling
2. Simpler code structure
3. Better user experience
4. Reduced complexity in UI component interactions
5. More predictable behavior

## Risks

1. Some advanced features may need to be reimplemented
2. Changes to application architecture required
3. Integration with existing async code requires careful handling

## Conclusion

The direct mode approach offers significant improvements to keyboard handling and UI responsiveness. By following this integration plan, we can maintain these benefits while reintegrating with the LLM client and tool execution capabilities.