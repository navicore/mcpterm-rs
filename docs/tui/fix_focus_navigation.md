# Fixing Focus and Navigation Issues in mcpterm-tui

## Key Issues Identified

After analyzing the codebase and terminal tests, the following key issues were identified:

1. **Tab Key Handling**: The Tab key requires multiple presses to change focus because it's being handled at multiple levels with potential conflicts:
   - In `AppState::handle_key_event` 
   - In `App::run_event_loop` with special case handling
   - Possibly being consumed by the edtui component before reaching our handlers

2. **Navigation in Message Viewer**: The j/k keys only move the message viewer when the input has focus because:
   - Each component only handles its own key events
   - The event routing isn't properly redirecting keys based on focus
   - The state isn't consistently synchronized

3. **Terminal Capability Issues**: The "Device not configured (os error 6)" error occurs because:
   - The app is trying to access terminal capabilities in a non-TTY environment (likely tmux)
   - The cursor position query might be failing
   - Raw mode might not be properly supported in the current terminal

## Solution Approach

There are two possible approaches to fix these issues:

### Approach 1: Fix the Current Architecture

1. **Establish a Clear Key Handling Hierarchy**:
   - Handle Tab, Esc, and global navigation keys at the highest level (App)
   - Prevent these keys from being passed to child components
   - Ensure consistent state synchronization after key handling

2. **Explicit Focus Management**:
   - Remove any key handling for focus changes from components
   - Manage focus exclusively in the App struct
   - Update UI rendering to reflect focus state

3. **Direct Key Handling for Navigation**:
   - Implement direct j/k navigation in the App struct
   - Skip passing these keys to components when in message viewer focus

4. **Graceful Fallbacks for Terminal Issues**:
   - Add checks for TTY capability before using raw features
   - Implement fallbacks for cursor positioning and other terminal operations
   - Add clear error messages when terminal features aren't available

### Approach 2: Simplify the Architecture (Recommended)

This approach simplifies the design by removing the complex event system and handling keys directly:

1. **Direct Key Processing Model**:
   - Replace the async event system with direct key handling
   - Process keys in a deterministic, synchronous manner
   - Handle key events at the highest level first

2. **Single Source of Truth for State**:
   - Use a single state struct that components read from
   - Update state directly in response to key events
   - Synchronize the UI with the state on each render

3. **Explicit Mode and Focus Handling**:
   - Handle Tab/Esc/i keys explicitly with direct state changes
   - Clearly separate global keys from focus-specific keys
   - Implement a clean hierarchy of key handling

4. **Reduce Dependency on Advanced Terminal Features**:
   - Avoid features that might not work in all terminal environments
   - Implement graceful degradation for non-TTY environments
   - Add diagnostics to help users identify terminal compatibility issues

## Recommended Implementation

We've created several test implementations to demonstrate these approaches:

1. `examples/key_event_test.rs` - Tests basic key event handling and focus switching
2. `examples/raw_terminal_test.rs` - Tests terminal capabilities for diagnostics
3. `examples/direct_key_handler.rs` - A simplified implementation with direct key handling

The `direct_key_handler.rs` example demonstrates the recommended approach with:
- Clear separation of key handling based on focus state
- Tab key handling at the highest level
- Explicit mode switching with i/Esc keys
- Direct navigation with j/k keys in the message viewer
- Simple state management without complex event systems

## Implementation Steps

1. **Replace the event system with direct key handling**:
   - Modify `App::run_event_loop` to directly read and process key events
   - Remove the separate event thread and channel-based messaging
   - Simplify the state management to avoid race conditions

2. **Implement a clear key handling hierarchy**:
   - First handle global keys (Tab, q, etc.)
   - Then handle focus-specific keys
   - Finally handle mode-specific keys

3. **Synchronize state directly**:
   - Update the focus and mode state directly in response to key events
   - Avoid complex bidirectional state synchronization
   - Ensure UI components read state but don't modify it

4. **Add terminal capability detection**:
   - Check if stdin/stdout are TTYs at startup
   - Provide appropriate error messages if terminal features aren't available
   - Add fallback modes for non-TTY environments

## Testing

To verify the fixes, we recommend:
1. Running the examples to understand the different approaches
2. Testing in different terminal environments (direct terminal, tmux, screen)
3. Testing with varying terminal types (xterm, tmux-256color, etc.)
4. Watching for the "Device not configured" error and adding diagnostics

The raw terminal test should help identify specific terminal capability issues, while the direct key handler demonstrates a more robust approach to key event handling that should work across different terminal environments.