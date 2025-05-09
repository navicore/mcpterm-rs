# Fixing Keyboard Input Handling in mcpterm-tui

## Core Issues Identified

After analyzing the codebase, I've identified several key structural issues with the keyboard input handling:

1. **Complex Multi-layered Event Processing**
   - Input events are first read in a background thread (`events/mod.rs`)
   - Events are converted and sent through a channel to the main loop
   - The main loop processes events and conditionally passes them to components
   - Components may handle or ignore events based on internal state
   - This creates multiple points of failure and state inconsistency

2. **Focus Management Problems**
   - Tab key is handled in multiple places (`App::run_event_loop` and `AppState::handle_key_event`)
   - Focus state isn't consistently synchronized between components
   - No clear "source of truth" for which component has focus

3. **Input Routing Inconsistency**
   - j/k keys don't work reliably because they're routed to the wrong component
   - Message viewer doesn't receive navigation keys when it has focus
   - Editor mode switching doesn't sync properly between app state and component state

4. **Excessive State Synchronization**
   - Bidirectional state sync between app state and components
   - Creates race conditions and inconsistent state
   - Makes debugging difficult due to state being updated in multiple places

## Core Architectural Solution

The solution is to simplify the keyboard input handling architecture:

### 1. Direct Key Handling Model

Replace the complex event system with a direct key handling approach:

```
                  +----------------+
                  |                |
Keyboard ---->    | Main App Loop  |  ---> handle_key() in AppState
                  |   (poll)       |
                  |                |
                  +----------------+
                          |
                          v
                  +----------------+
                  |                |
                  |   Components   |
                  |    (render)    |
                  |                |
                  +----------------+
```

This model:
- Processes key events directly in the main loop
- Handles all key events in a single function
- Maintains a clear hierarchy of key handling
- Eliminates the need for complex event routing

### 2. Clear Input Handling Hierarchy

Establish a strict hierarchy for key handling:

1. **Global Keys** - Always handled first (Tab, Ctrl+C, etc.)
2. **Focus-specific Keys** - Handled based on current focus
3. **Mode-specific Keys** - Handled based on current mode within focus

This ensures predictable behavior and eliminates conflicts.

### 3. Unidirectional Data Flow

Implement a unidirectional data flow pattern:

- State is modified ONLY by the main app through handle_key()
- Components read state but never modify it
- UI rendering reflects the current state
- No bidirectional synchronization

### 4. Focus Isolation

Implement clear focus management:
- One central place for focus state
- Tab key handling only in the main app
- Components respond based on focus state
- No component-level focus management

## Implementation Strategy

I've created a `direct_key_handling.rs` example that demonstrates this approach:

### Key Features:

1. **Single Event Loop**:
   - Direct polling for events in the main loop
   - No separate event thread or channels
   - Clear control flow and easier debugging

2. **Central State Management**:
   - Single AppState with all UI state
   - No static variables or shared state
   - All state updates in one place

3. **Explicit Key Handling Hierarchy**:
   - Global keys first (Tab, quit)
   - Focus-specific keys next
   - Mode-specific keys last

4. **Passive Components**:
   - Components are simple widgets that render state
   - No internal state management
   - Focus and mode controlled by app state

### Migration Steps:

1. **Remove the event channel system**:
   - Move event polling directly into the main loop
   - Remove EventHandler thread and channels
   - Poll for events directly with crossterm

2. **Centralize state management**:
   - Move all state into AppState
   - Remove bidirectional synchronization
   - Make components read-only views of the state

3. **Implement hierarchical key handling**:
   - Create a single handle_key method in AppState
   - Implement the key handling hierarchy
   - Handle focus changes explicitly

4. **Simplify component interfaces**:
   - Make components simpler rendering widgets
   - Remove internal event handling
   - Pass focus and mode state from app state

## Benefits

This simplified architecture will:

1. **Fix the Tab key issues**:
   - Tab will work reliably with a single press
   - Focus changes will be immediately visible

2. **Fix the j/k navigation**:
   - Navigation will work in the appropriate focus state
   - Clear routing of keys based on focus

3. **Improve maintainability**:
   - Easier to understand and debug
   - Fewer points of failure
   - Clear data flow

4. **Reduce potential for race conditions**:
   - No thread synchronization issues
   - No state inconsistency
   - Deterministic behavior

## Testing Strategy

To validate this approach:

1. Run the `direct_key_handling.rs` example to verify it works correctly
2. Apply the architectural changes to the main app incrementally
3. Test each key (Tab, j/k, i, Esc, etc.) to ensure correct behavior
4. Add detailed logging during key processing for debugging

I've created this example outside of tmux to ensure we isolate terminal capability issues from the architectural issues.