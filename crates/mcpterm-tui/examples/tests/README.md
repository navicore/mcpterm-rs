# Test Utilities for mcpterm-tui

This directory contains various utility scripts and test programs for debugging and testing the terminal UI.

## Terminal Tests

- `terminal_test.rs` - Tests basic terminal capabilities like raw mode and cursor positioning
- `raw_terminal_test.rs` - A more detailed terminal capability test that also checks TTY status

## Input Handling Tests

- `test_input.rs` - Tests direct keyboard input handling

## Formatting Tests

- `test-formatter.rs` - Tests the output formatting utilities
- `test-prompt.txt` - Sample prompt for testing

## Command Tests

- `test_slash_commands.rs` - Tests slash command parsing and handling
- `test_tool_execution.rs` - Tests tool execution functionality

## Running Tests

To run any of these tests, use:

```bash
cargo run --example tests/<test_name>
```

For example:
```bash
cargo run --example tests/terminal_test
```

These test utilities are primarily for debugging and development purposes.