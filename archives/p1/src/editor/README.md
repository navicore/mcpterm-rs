# Editor Modules

This directory contains editor implementations for the mcpterm application:

1. **LineEditor** - Our original custom editor widget
2. **TextAreaEditor** - New wrapper for the tui-textarea crate

Both implementations support Vi and Emacs keybinding modes and share a common interface.

## TextAreaEditor (New Implementation)

A wrapper around the `tui-textarea` crate to provide a rich text editing experience:

### Features

- Multiline text editing
- Vi and Emacs keybinding modes
- Searching within content (via the `search` feature of tui-textarea)
- Cursor navigation
- Text selection
- Standard editing operations (cut, copy, paste)
- History navigation

### Implementation Notes

The editor is designed to integrate seamlessly with the existing application structure:

1. The `TextAreaEditor` wraps `tui-textarea::TextArea` with an API compatible with our previous editor
2. Key events are converted from crossterm to the format expected by ratatui/tui-textarea
3. Handle key events preserve the same behavior (Submit/Abort/Continue) as the original editor

### Usage

```rust
use crate::editor::{TextAreaEditor, KeybindingMode, HandleResult};

// Create the editor
let mut editor = TextAreaEditor::new(KeybindingMode::Vi)
    .title("Input".to_string())
    .placeholder("Type here...".to_string())
    .block(Block::default().borders(Borders::ALL));

// In the event handling loop:
match editor.handle_key_event(key) {
    HandleResult::Continue => {},
    HandleResult::Submit(text) => {
        // Process the text that was submitted
        println!("Submitted: {}", text);
        editor.clear();
    },
    HandleResult::Abort => {
        // Exit or handle abort
    }
}

// No need to manually set cursor position with tui-textarea
// it handles cursor positioning internally

// Render the editor widget
f.render_widget(editor.clone(), area);
```

### Built-in Keybindings

The tui-textarea crate provides a rich set of keybindings out of the box:

#### Vi Mode
- Normal mode navigation (hjkl, w, b, etc.)
- Insert mode for text input
- Delete operations (x, dd, dw, etc.)
- Yanking and pasting
- Many other vi keybindings

#### Emacs Mode
- Ctrl+F/B/N/P for navigation
- Alt+F/B for word navigation
- Ctrl+A/E for line start/end
- Ctrl+K for killing to end of line
- Many other emacs keybindings

## LineEditor (Original Implementation)

The original custom implementation with the following features:

- **Multiple Keybinding Modes**: Support for both Vi and Emacs key bindings
- **Vi Mode Features**:
  - Normal and insert modes
  - Cursor movement (h,j,k,l,w,b,0,$)
  - Text editing operations (x,d,c,u)
  - Compound commands (dw,dd,d$,d0,db)
  - Undo functionality
- **Emacs Mode Features**:
  - Basic cursor movement (Ctrl+A, Ctrl+E, Ctrl+F, Ctrl+B)
  - Basic text editing (Ctrl+D, Ctrl+K)
  - Word navigation (Alt+F, Alt+B)
- **Widget Integration**: Implemented as a proper ratatui widget
- **Multi-line Support**: Handles text wrapping and multi-line editing
- **Customization**: Configurable title, placeholder text, and styling

## Future Improvements

- Add syntax highlighting
- Add autocompletion
- Add history navigation
- Extend search functionality
- Customize keybindings

## License

MIT
