# MCPTerm TUI

This crate provides the Terminal User Interface (TUI) for the Model Context Protocol (MCP) terminal application.

## Modules

- `ui`: UI components (editors, widgets)
- `state`: UI state management
- `events`: UI event handling

## Features

- Vim-style editor with modal editing
- Read-only message viewer
- Search functionality
- Command history
- Status indicators
- Message formatting

## Usage

```bash
# Run the TUI application
cargo run --package mcpterm-tui

# Run with specific model
cargo run --package mcpterm-tui -- --model anthropic.claude-3-sonnet-20240229-v1:0

# Run with direct key handling mode (for improved keyboard input)
cargo run --package mcpterm-tui -- --direct-mode
```

## Direct Mode

The application supports a direct key handling mode that can be enabled with the `--direct-mode` or `-d` flag. This mode:

- Uses a simpler input handling approach that bypasses the complex event system
- Provides more reliable keyboard input handling
- Fixes issues with Tab key requiring multiple presses
- Ensures j/k keys work properly for message scrolling
- Enables proper auto-scrolling of messages
- Shows cursor position in the input field correctly

Direct mode is recommended if you experience keyboard input issues with the standard mode.

## Keyboard Shortcuts

- `Tab`: Switch focus between editors
- `i`: Enter insert mode (input editor)
- `Esc`: Return to normal mode
- `Enter`: Submit input (in normal mode)
- `/`: Search in message history
- `Ctrl+P/Ctrl+N`: Navigate command history
- `q`: Quit application

### In Direct Mode

- `Tab`: Switch focus between input and messages
- `j/k`: Scroll messages (when message area has focus)
- `a`: Toggle auto-scroll (when message area has focus)
- `g/G`: Jump to top/bottom of messages (when message area has focus)