# MCP Runtime

This crate provides the runtime execution environment for the Model Context Protocol (MCP) terminal application. It serves as the orchestration layer between the user interface, the model interaction, and the tools execution.

## Components

### Event Bus

The Event Bus is the central communication hub of the application. It manages the flow of events between different components using an event-driven architecture. This allows components to operate independently and asynchronously.

Key features:
- Different event types for UI, Model, and API interactions
- Async event handlers with proper error handling
- Back-pressure handling via bounded channels
- Non-blocking event dispatch

Example usage:

```rust
use mcp_runtime::EventBus;
use mcp_runtime::UiEvent;
use anyhow::Result;

// Create the event bus
let bus = EventBus::new();

// Register a handler for UI events
bus.register_ui_handler(Box::new(|event| {
    Box::pin(async move {
        match event {
            UiEvent::UserInput(input) => {
                println!("User input: {}", input);
                // Process the input...
                Ok(())
            }
            UiEvent::Quit => {
                println!("User wants to quit");
                // Handle quit...
                Ok(())
            }
            _ => Ok(()),
        }
    })
})).unwrap();

// Start event distribution
bus.start_event_distribution().unwrap();

// Send an event
bus.ui_sender().send(UiEvent::UserInput("Hello, world!".to_string())).unwrap();
```

### Session Management

The Session module manages the conversation state and context, providing:
- Thread-safe access to conversation context
- Methods for adding messages and resetting context
- Proper synchronization through RwLock

### Tool Executor

The Tool Executor coordinates the execution of tools within the MCP protocol:
- Manages tool safety constraints
- Handles execution of tools with proper error handling
- Provides a clean interface for tool invocation

## Architecture

This crate implements a Staged Event-Driven Architecture (SEDA) pattern:

```
┌──────────────┐  Events   ┌────────────────┐  Events   ┌─────────────────┐
│              │ ────────> │                │ ────────> │                 │
│   UI Layer   │           │  Model Layer   │           │   API Clients   │
│              │ <──────── │                │ <──────── │                 │
└──────────────┘  Updates  └────────────────┘  Updates  └─────────────────┘
```

Events flow through the system via channels with each component processing them independently, preventing UI blocking during long-running operations.

## Testing

The crate includes comprehensive tests for all components:
- Unit tests for individual components
- Integration tests for component interaction
- Async tests for event handling

## Example

A simple example of using the runtime:

```rust
use mcp_runtime::{EventBus, Session, ToolExecutor};
use mcp_runtime::{UiEvent, ModelEvent, ApiEvent};
use mcp_tools::ToolManager;
use anyhow::Result;

// Create components
let session = Session::new();
let tool_manager = ToolManager::new();
let tool_executor = ToolExecutor::new(tool_manager);
let event_bus = EventBus::new();

// Register handlers
event_bus.register_ui_handler(Box::new(move |event| {
    let session_clone = session.clone();
    Box::pin(async move {
        match event {
            UiEvent::UserInput(input) => {
                session_clone.add_user_message(&input);
                // Additional processing...
                Ok(())
            }
            _ => Ok(()),
        }
    })
})).unwrap();

// Start event distribution
event_bus.start_event_distribution().unwrap();

// Run the application...
```