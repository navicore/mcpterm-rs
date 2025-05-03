# MCPTerm Architecture

This document outlines the architecture for the Model Context Protocol Terminal (MCPTerm) project. The architecture is designed to be modular, testable, and event-driven to ensure a responsive user interface and proper separation of concerns.

## References

- MCP Protocol Specification: https://modelcontextprotocol.io/llms-full.txt
- MCP Schema JSON: https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/refs/heads/main/schema/2025-03-26/schema.json

## Project Structure

The project is organized as a Cargo workspace with multiple crates, each responsible for a specific aspect of the system:

```
mcpterm/
├── Cargo.toml (workspace definition)
├── crates/
│   ├── mcp-core/              (protocol definitions, common types)
│   │   ├── protocol/          (JSON-RPC handling, MCP message types)
│   │   └── context/           (conversation context management)
│   │
│   ├── mcp-resources/         (resource management abstraction)
│   │
│   ├── mcp-tools/             (tool implementations)
│   │   ├── registry/          (tool registration and discovery)
│   │   ├── shell/             (shell execution tools)
│   │   ├── filesystem/        (file operations tools)
│   │   └── search/            (search tools)
│   │
│   ├── mcp-runtime/           (execution environment)
│   │   ├── event-bus/         (the SEDA/CSP message passing system)
│   │   ├── session/           (session state management)
│   │   └── executor/          (tool execution coordination)
│   │
│   ├── mcp-llm/               (LLM providers and adapters)
│   │   ├── client-trait/      (common interface)
│   │   ├── anthropic/         (Claude implementation)
│   │   ├── bedrock/           (AWS Bedrock implementation)
│   │   └── streaming/         (streaming response handling)
│   │
│   ├── mcpterm-tui/           (terminal UI application)
│   │   ├── ui/                (TUI components)
│   │   ├── state/             (UI state management)
│   │   └── events/            (UI event handling)
│   │
│   └── mcpterm-cli/           (command line interface)
│
└── tests/
    ├── integration/           (cross-crate tests)
    └── e2e/                   (end-to-end tests)
```

## Core Architecture

### Event-Driven Architecture

The system uses a Staged Event-Driven Architecture (SEDA) / Communicating Sequential Processes (CSP) approach to ensure non-blocking UI and proper handling of potentially lengthy LLM operations:

```
┌──────────────┐  Events   ┌────────────────┐  Events   ┌─────────────────┐
│              │ ────────> │                │ ────────> │                 │
│   UI Layer   │           │  Model Layer   │           │   API Clients   │
│              │ <──────── │                │ <──────── │                 │
└──────────────┘  Updates  └────────────────┘  Updates  └─────────────────┘
```

This architecture creates a clear separation between the UI, business logic, and external API interactions, while allowing each component to operate independently without blocking.

### Conversation Context Management

The `context` module in `mcp-core` is responsible for managing the conversation state:

```rust
struct ConversationContext {
    system_prompt: String,
    messages: Vec<Message>,
    current_request_id: Option<String>,
}
```

This context keeps track of:
- System prompts that define capabilities
- User messages
- AI responses
- Tool invocations and results

This enables clean context resets when switching tasks while maintaining coherent contexts during multi-step interactions.

### Event Bus

The `event-bus` module in `mcp-runtime` provides message passing between components:

```rust
// Example event types
enum UiEvent {
    KeyPress(KeyEvent),
    UserInput(String),
    RequestCancellation,
    // ...
}

enum ModelEvent {
    ProcessUserMessage(String),
    ToolResult(String, Value),
    ResetContext,
    // ...
}

enum ApiEvent {
    SendRequest(ApiRequest),
    ProcessStream(StreamResponse),
    CancelRequest,
    // ...
}
```

Events flow through the system using channels (e.g., Tokio or crossbeam channels) with proper backpressure handling.

## UI Architecture

The terminal UI is implemented using Ratatui with a non-blocking design:

```rust
struct McpTermApp {
    ui_tx: mpsc::Sender<UiEvent>,
    model_tx: mpsc::Sender<ModelEvent>,
    
    ui_state: Arc<RwLock<UiState>>,
}

// UI rendering loop
fn run_ui(app: Arc<McpTermApp>) {
    // Setup terminal
    let mut terminal = setup_terminal().unwrap();
    
    // Main loop
    while !app.should_exit() {
        // Get latest UI state
        let state = app.ui_state.read().unwrap();
        
        // Render UI
        terminal.draw(|f| render_ui(f, &state)).unwrap();
        
        // Handle input (non-blocking)
        if crossterm::event::poll(Duration::from_millis(100)).unwrap() {
            if let Event::Key(key) = crossterm::event::read().unwrap() {
                // Send UI event through channel
                app.ui_tx.send(UiEvent::KeyPress(key)).unwrap();
            }
        }
    }
}
```

This design ensures the UI remains responsive even when the LLM is processing or tool execution is occurring.

## MCP Implementation

The Model Context Protocol implementation follows the JSON-RPC 2.0 based protocol specified in the MCP documentation:

### Request/Response Flow

```
1. User input → Client
2. Client → LLM (with context)
3. LLM → Client (direct response or tool request)
4. If tool request, Client executes tool and sends result to LLM
5. LLM continues processing, possibly requesting more tools
6. Final LLM response → Client → User
```

### Message Types

```rust
// Based on MCP schema
struct McpRequest {
    jsonrpc: String,
    method: String,
    params: Value,
    id: Option<Value>,
}

struct McpToolInvocation {
    tool_id: String,
    parameters: Value,
}

struct McpResponse {
    jsonrpc: String,
    result: Option<Value>,
    error: Option<McpError>,
    id: Value,
}
```

### Tool Execution

The `mcp-tools` crate provides implementations for various tools:

- Shell execution
- File system operations
- Search capabilities
- etc.

Tools are registered in a central registry with metadata about their capabilities, input/output schemas, and execution handlers.

## LLM Client Architecture

The `mcp-llm` crate provides a common interface for different LLM providers:

```rust
trait LlmClient {
    async fn send_message(&self, context: &ConversationContext) -> Result<LlmResponse>;
    async fn stream_message(&self, context: &ConversationContext) -> Result<impl Stream<Item = Result<StreamChunk>>>;
    fn cancel_request(&self, request_id: &str) -> Result<()>;
}
```

Implementations are provided for:
- Anthropic Claude
- AWS Bedrock
- Other providers as needed

## Testing Strategy

The architecture supports comprehensive testing:

1. **Unit Tests**: Each module has its own tests with mocked dependencies
2. **Integration Tests**: Cross-crate tests to verify proper component interaction
3. **End-to-End Tests**: Full system tests for critical flows
4. **Property Tests**: For complex state handling and event processing

## Performance Considerations

1. **Non-blocking UI**: The event-driven architecture ensures UI responsiveness
2. **Parallel Tool Execution**: Multiple tools can be executed in parallel when applicable
3. **Streaming Responses**: LLM responses are streamed to provide immediate feedback
4. **Backpressure Handling**: Prevents memory issues with high event volumes

## Security Considerations

1. **Validation**: All user and LLM inputs are validated
2. **Sandboxing**: Tool execution is properly constrained
3. **Permission Models**: Resource access follows principle of least privilege

## Scalability Path

This architecture allows for future extensions:

1. Additional LLM providers
2. New tool implementations
3. Alternative UI frontends (GUI, web, etc.)
4. Distributed operation if needed