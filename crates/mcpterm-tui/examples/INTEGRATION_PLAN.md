# Integration Plan: TUI Implementation

This document outlines how to integrate our working simple_solution into the main project, while adding back the more powerful edtui editor capabilities for the input area.

## Core Principles

1. **Preserve Simple Scrolling Logic**
   - Keep the direct, index-based scrolling mechanism
   - Maintain auto-scrolling toggle functionality
   - Ensure j/k navigation works consistently

2. **Combine with EdTUI for Input**
   - Use edtui only for the input editor component
   - Keep the message area using simple Paragraph widgets
   - Separate the concerns clearly

3. **Incremental Integration**
   - Implement in phases to ensure each step works
   - Test thoroughly between steps

## Integration Steps

### Phase 1: Edtui Input Editor

1. **Create a new example combining simple_solution with edtui input**
   - Start with a copy of simple_solution.rs
   - Add the edtui InputEditor widget from rebuild_step5_with_edtui.rs
   - Implement direct key handling for the InputEditor
   - Ensure scrolling still works correctly

2. **Test Input Editor Features**
   - Verify that input is displayed in real-time in the input widget
   - Test VI-style modes (normal, insert, visual)
   - Make sure input content properly appears in messages when submitted

### Phase 2: Enhanced Message Display

1. **Improve Message Styling**
   - Add timestamp display for messages
   - Enhance color styling for different message types
   - Support multi-line message formatting

2. **Add Message History Navigation**
   - Implement command history navigation with up/down arrows
   - Add the ability to edit historical inputs

### Phase 3: Full Integration

1. **Create a New UI Structure**
   - Organize the final implementation into proper modules
   - Separate concerns: input handling, rendering, state management

2. **Implement Final Features**
   - Add clipboard support for copy/paste
   - Support multi-line input with proper editing
   - Add status line with more information

3. **Final Cleanup**
   - Remove debug output and temporary code
   - Document the implementation
   - Create examples of common usage patterns

## Technical Approach

### State Management

```rust
pub struct AppState {
    // Messages
    messages: Vec<Message>,
    scroll: usize,
    auto_scroll: bool,
    visible_message_count: usize,
    
    // Input
    input: String,
    history: Vec<String>,
    history_index: usize,
    
    // UI state
    focus: Focus,
    mode: EditorMode,
    running: bool,
}
```

### UI Components

1. **Message Viewer**
   - Simple paragraph widget with colored text
   - Manual scrolling implementation
   - Auto-scroll capability

2. **Input Editor**
   - EdTUI-based editor for powerful editing
   - VI-style mode support
   - History navigation

3. **Status Line**
   - Mode indication
   - Focus indication
   - Scroll status

### Key Handling

```rust
fn handle_key(&mut self, key: KeyEvent) {
    // Global keys (focus, mode, quit)
    if handle_global_key(key) {
        return;
    }
    
    // Focus-specific keys
    match self.focus {
        Focus::Messages => self.handle_message_key(key),
        Focus::Input => self.handle_input_key(key),
    }
}
```

## Expected Outcome

The final implementation will:

1. Have reliable scrolling that shows new messages
2. Support VI-style editing in the input area
3. Have clear, direct keyboard handling
4. Display messages with proper styling
5. Provide a smooth, responsive user experience

Each component will be well-organized and modular, making it easy to understand and maintain.