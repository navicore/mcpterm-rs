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
```

## Keyboard Shortcuts

- `Tab`: Switch focus between editors
- `i`: Enter insert mode (input editor)
- `Esc`: Return to normal mode
- `Enter`: Submit input (in normal mode)
- `/`: Search in message history
- `Ctrl+P/Ctrl+N`: Navigate command history
- `q`: Quit application