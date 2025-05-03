# MCP Runtime

This crate provides the runtime execution environment for the Model Context Protocol (MCP), including event handling, session management, and tool execution coordination.

## Modules

- `event-bus`: Event-driven communication between components
- `session`: Conversation session management
- `executor`: Tool execution coordination

## Features

- Non-blocking event processing
- Tool execution with safety constraints
- Integration with LLM clients
- Session state management
- Efficient event routing

## Usage

```rust
use mcp_runtime::event_bus::{EventBus, UiEvent, ModelEvent};
use mcp_runtime::session::Session;
use mcp_runtime::executor::ToolExecutor;

// Create event bus
let (ui_tx, ui_rx) = EventBus::new_ui_channel();
let (model_tx, model_rx) = EventBus::new_model_channel();

// Create session
let session = Session::new();

// Process events
ui_tx.send(UiEvent::UserInput("Hello, world!".to_string()));
```