# mcpterm-tui Examples

This directory contains example programs and tests for the mcpterm-tui application.

## Rebuild Examples

The following examples demonstrate a step-by-step approach to rebuilding the keyboard input handling in mcpterm-tui:

### Basic Examples

1. **step1_minimal.rs** - Minimal terminal application that handles keyboard input reliably
   ```bash
   cargo run --example step1_minimal
   ```

2. **step2_two_panels.rs** - Basic two-panel layout with message area and input area
   ```bash
   cargo run --example step2_two_panels
   ```

3. **step3_with_modes.rs** - Adds VI-style normal/insert modes with proper focus management
   ```bash
   cargo run --example step3_with_modes
   ```

### Advanced Examples

4. **rebuild_step4_with_ratatui.rs** - Uses ratatui for rendering with direct keyboard handling
   ```bash
   cargo run --example rebuild_step4_with_ratatui
   ```

5. **rebuild_step5_with_edtui.rs** - Adds edtui integration while maintaining direct keyboard control
   ```bash
   cargo run --example rebuild_step5_with_edtui
   ```

6. **rebuild_final_implementation.rs** - Complete example with all components integrated
   ```bash
   cargo run --example rebuild_final_implementation
   ```

## Terminal Tests

These examples test terminal capabilities and input handling:

- **raw_terminal_test.rs** - Tests basic terminal capabilities and TTY status
  ```bash
  cargo run --example raw_terminal_test
  ```

- **test_terminal.rs** - Tests terminal functionality directly
  ```bash
  cargo run --example test_terminal
  ```

- **tty_test.rs** - Tests TTY capabilities
  ```bash
  cargo run --example tty_test
  ```

## Other Examples

- **direct_key_handler.rs** - Direct keyboard input handling with a clean architecture
  ```bash
  cargo run --example direct_key_handler
  ```

- **direct_key_handling.rs** - Similar to direct_key_handler but with more features
  ```bash
  cargo run --example direct_key_handling
  ```

- **direct_tui.rs** - A minimal direct implementation of a terminal UI
  ```bash
  cargo run --example direct_tui
  ```

- **minimal_editor.rs** - A minimal example of an editor using edtui
  ```bash
  cargo run --example minimal_editor
  ```

- **minimal_tui.rs** - A minimal example of a terminal UI
  ```bash
  cargo run --example minimal_tui
  ```

- **simple_tui.rs** - A simple terminal UI example
  ```bash
  cargo run --example simple_tui
  ```

## Documentation

- **rebuild_NEXT_STEPS.md** - Next steps for rebuilding the main application
- **README.md** - Overview of the rebuild approach

## Running Examples

To run any example, use:

```bash
cargo run --example <example_name>
```

Replace `<example_name>` with the name of the example file without the `.rs` extension.