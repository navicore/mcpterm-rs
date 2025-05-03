# Migration Plan to Use LineEditor Widget

## Overview

This document outlines the steps to migrate the current mcpterm application to use the new LineEditor widget, which:

1. Supports both vi and emacs keybindings
2. Is implemented as a reusable widget
3. Cleanly separates editor logic from application logic

## Migration Steps

### 1. Update Config to Support Keybinding Mode Selection

```rust
// In config/mod.rs
pub struct Config {
    // Existing fields...
    pub keybinding_mode: KeybindingMode,
}

// Update Config::load() to include keybinding mode
// Default to Vi for now
```

### 2. Replace Direct Editor Implementation in main.rs

1. Remove the `ViState` and `UndoState` structs and their implementations
2. Remove all the direct key handling for editing
3. Replace with the new LineEditor widget

### 3. Update the Main Loop

```rust
// In main.rs
let mut editor = LineEditor::new(
    if config.use_emacs_mode { 
        KeybindingMode::Emacs 
    } else { 
        KeybindingMode::Vi 
    }
).title("Input".to_string())
 .placeholder("Press 'i' to enter input mode, 'q' to quit".to_string())
 .block(Block::default().borders(Borders::ALL));

// In the event handling loop for input focus:
match editor.handle_key_event(key) {
    HandleResult::Continue => {},
    HandleResult::Submit(text) => {
        // Process the submitted text
        history.add_message(text.clone(), MessageType::User);
        
        // Process with agent and add response
        let response = agent.process_message(&text);
        history.add_message(response, MessageType::Assistant);
        
        // Clear the editor
        editor.clear();
    },
    HandleResult::Abort => {
        running = false;
    }
}
```

### 4. Update the UI Rendering

```rust
// In the terminal.draw callback:

// If input area is focused, set cursor position
if matches!(focus, FocusArea::Input) {
    let (cursor_x, cursor_y) = editor.calc_cursor_coords(chunks[1]);
    f.set_cursor(cursor_x, cursor_y);
}

// Render the editor widget
f.render_widget(editor.clone(), chunks[1]);
```

### 5. Add Configuration Option

Add command-line flag and config option for keybinding mode:

```
--emacs-mode     Use Emacs keybindings instead of Vi
```

### 6. Test Thoroughly

Test both Vi and Emacs mode and ensure:
- All keyboard shortcuts work
- Cursor handling is correct
- Input and editing works as expected
- Undo functionality works

## Future Improvements

1. Move editor to a separate crate when stable
2. Add more advanced features:
   - Syntax highlighting
   - Auto-completion
   - History navigation (up/down arrows for past messages)
   - Search functionality

## Sample Command to Enable Emacs Mode

```
mcpterm --emacs-mode
```
