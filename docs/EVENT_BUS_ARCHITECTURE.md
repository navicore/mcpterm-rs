# Event Bus Architecture for McpTerm

This document describes the event-driven architecture based on the `EventBus` in `mcp-runtime` crate, which is designed to facilitate non-blocking, asynchronous communication between different components of the application.

## Overview

The architecture follows a SEDA (Staged Event-Driven Architecture) pattern where different components communicate via events passed through channels. This approach:

1. Decouples components from each other, allowing them to evolve independently
2. Ensures non-blocking I/O throughout the system
3. Simplifies concurrency by using message passing instead of shared mutable state
4. Provides a consistent model for both CLI and TUI interfaces

## Core Components

### EventBus

The `EventBus` in `mcp-runtime` provides the central message-passing infrastructure with three main event types:

- **UiEvent**: Events initiated by user interactions (input, key presses, etc.)
- **ModelEvent**: Events related to the conversation model (processing messages, tool execution)
- **ApiEvent**: Events related to external API calls (LLM requests/responses)

### SessionManager

The `SessionManager` acts as the controller that connects all components together:

- Maintains the conversation state
- Registers handlers for different event types
- Orchestrates the flow of events between UI, model, and API layers
- Manages tool execution through the ToolExecutor

### Adapters

Adapters bridge the gap between the specific UI implementations (CLI, TUI) and the event bus:

- **CLI Adapter**: Translates between CLI input/output and events
- **TUI Adapter**: (Future) Will translate between TUI widgets and events

## Event Flow

1. **User Input → UiEvent**:
   - User provides input through CLI or TUI
   - Adapter converts input to a `UiEvent::UserInput` and sends it to the event bus

2. **UiEvent → ModelEvent**:
   - Event handler receives `UiEvent::UserInput`
   - Adds user message to conversation context
   - Sends `ModelEvent::ProcessUserMessage` to request processing

3. **ModelEvent → ApiEvent**:
   - Event handler for `ModelEvent::ProcessUserMessage` prepares request to LLM
   - LLM client streams the response back through event handlers

4. **Response Handling**:
   - For regular text: `ModelEvent::LlmMessage` or `ModelEvent::LlmStreamChunk`
   - For tool calls: `ModelEvent::ToolRequest`
   
5. **Tool Execution**:
   - ToolExecutor executes tools asynchronously
   - Results are sent as `ModelEvent::ToolResult`
   - Follow-up responses are requested automatically

## Implementation Notes

### CLI Implementation

The CLI implementation uses the event bus architecture as follows:

1. `CliSession` creates and manages the `SessionManager` and `EventBus`
2. `CliEventAdapter` handles UI-specific aspects of the event flow
3. Messages are sent to the event bus and responses are collected asynchronously
4. Tool execution is handled through the event system
5. Bidirectional event bus bridging enables communication between components

#### Event Bus Bridging

A key enhancement to our implementation is the bidirectional event bus bridging, which allows multiple event buses to communicate:

```rust
// Create a bidirectional bridge between two event buses
fn bridge_event_buses(bus1: &Arc<EventBus>, bus2: &Arc<EventBus>) -> Result<()> {
    // Create handlers that forward events from bus1 to bus2
    let bus2_ui_tx = bus2.ui_sender();

    // Forward UI events from bus1 to bus2
    let ui_forward_handler = event_bus::create_handler(move |event: UiEvent| {
        let bus2_ui_tx = bus2_ui_tx.clone();
        Box::pin(async move {
            let _ = bus2_ui_tx.send(event);
            Ok(())
        })
    });
    bus1.register_ui_handler(ui_forward_handler)?;

    // (Similarly for Model and API events, and for bus2 to bus1 direction)

    Ok(())
}
```

This pattern enables:
- Components to have their own isolated event buses
- Seamless communication between components
- Better testability and component isolation
- Flexible event routing topologies

### TUI Implementation (Recommendations)

For the TUI implementation, we recommend:

1. Create a `TuiEventAdapter` that bridges between the TUI widgets and the event bus
2. Use async handlers for updating the UI widgets based on events
3. Convert user interactions (keystrokes, etc.) into appropriate `UiEvent` types
4. Maintain the same event flow model as the CLI but with UI rendering
5. Implement proper event bus bridging between TUI components and the session manager
6. Use the following enhanced response handling pattern for UI updates:

```rust
// Create a handler for model events to update TUI
let model_handler = event_bus::create_handler(move |event: ModelEvent| {
    let ui_state = ui_state.clone();

    Box::pin(async move {
        match event {
            ModelEvent::LlmStreamChunk(chunk) => {
                // Update UI with streaming chunk
                ui_state.write().unwrap().append_to_chat(chunk);
                // Request UI redraw
                let _ = redraw_tx.send(());
            },
            ModelEvent::LlmResponseComplete => {
                // Handle completion (e.g., update status indicators)
                ui_state.write().unwrap().set_status(Status::Ready);
                // Request UI redraw
                let _ = redraw_tx.send(());
            },
            ModelEvent::ToolRequest(tool_id, params) => {
                // Show tool execution in UI
                ui_state.write().unwrap().set_status(Status::ToolExecution(tool_id));
                // Request UI redraw
                let _ = redraw_tx.send(());
            },
            // Handle other events...
            _ => {}
        }
        Ok(())
    })
});

## Advantages for TUI

Implementing the TUI using this event bus architecture provides several advantages:

1. **Non-blocking UI**: The UI remains responsive even during long-running operations
2. **Consistent architecture**: The same event flow model works for both CLI and TUI
3. **Separation of concerns**: UI components focus on rendering, while business logic is handled through the event system
4. **Testability**: Components can be tested in isolation by mocking events
5. **Extensibility**: New UI components or event types can be added without affecting existing code

## TUI-Specific Implementation Guide

1. **Widget Event Mapping**:
   - Map TUI widget actions to appropriate `UiEvent` types
   - Register handlers for `ModelEvent` to update UI widgets

2. **Async Rendering**:
   - Use Tokio tasks for rendering updates
   - Ensure UI updates do not block the event loop

3. **State Management**:
   - Use `Arc<RwLock<State>>` for shared UI state
   - Update state based on events rather than direct manipulation

4. **Widget Structure**:
   - Create widgets that receive events for updates
   - Use channel-based communication between widgets

## Example TUI Event Flow

```
User types → KeyEvent → UiEventAdapter → UiEvent::UserInput → EventBus
  → ModelEvent::ProcessUserMessage → LLM processing
  → ModelEvent::LlmStreamChunk → TuiEventAdapter → Update ChatWidget
  → ModelEvent::ToolRequest → ToolExecutor → Tool execution
  → ModelEvent::ToolResult → Event handlers → LLM follow-up
  → ModelEvent::LlmMessage → TuiEventAdapter → Update ChatWidget
```

By following this event-driven architecture, the TUI implementation will maintain a clean separation of concerns while ensuring a responsive and non-blocking user experience.