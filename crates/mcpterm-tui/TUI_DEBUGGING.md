# TUI Keyboard Input Debugging

This document outlines the diagnosis and proposed solutions for the keyboard input issues in the mcpterm-tui application.

## Symptoms

1. Tab key requires multiple presses to switch focus
2. j/k keys only work in message viewer when input has focus
3. Typed characters may not appear immediately
4. Keyboard input feels sluggish or unresponsive
5. Potentially high CPU usage

## Diagnosis

Through extensive testing, we've identified several potential causes for these issues:

1. **Inefficient Event Loop**: The main application's event loop might be handling events inefficiently, causing high CPU usage and sluggish response.

2. **Complex Event System**: The application's event system involves multiple layers that may be introducing latency between key press and UI update.

3. **Rendering Approach**: There may be inefficient rendering happening on every event, not just after keyboard input.

4. **Terminal Mode Conflicts**: Multiple components might be changing terminal settings (raw mode, cursor visibility, etc.).

5. **Focus Management**: The focus system might be too complex, causing Tab key issues.

## Testing Tools

We've created three progressively simplified implementations to help diagnose the issues:

1. **Standard Mode**: Original implementation with full event system
   ```
   cargo run --package mcpterm-tui
   ```

2. **Direct Mode**: Simplified implementation with direct key handling
   ```
   cargo run --package mcpterm-tui -- --direct-mode
   ```

3. **Ultra-Simple Mode**: Minimal standalone implementation
   ```
   cargo run --package mcpterm-tui -- --simple-mode
   ```

We also provide a diagnostic script (`debug_tui.sh`) that runs all three implementations and checks CPU usage.

## Key Differences

The ultra-simple implementation, which should work reliably, has these key characteristics:

1. Direct event handling with minimal abstraction
2. Rendering only occurs after key events, not continuously
3. Terminal setup/cleanup happens only once
4. Event polling with a longer timeout (250ms vs 100ms)
5. No complex component interaction or event system
6. Simplified UI rendering

## Proposed Solutions

Based on our testing, we recommend the following solutions:

### Short-term Fix

Use the direct mode implementation, which maintains the AppState structure but simplifies key handling:

```
cargo run --package mcpterm-tui -- --direct-mode
```

This mode lacks LLM integration but provides a usable interface with correct keyboard handling.

### Medium-term Fix

1. Refactor the main application to use the direct key handling approach
2. Keep event system only for async operations (LLM client, tools)
3. Simplify the UI rendering to be more efficient
4. Increase event polling timeout to reduce CPU usage
5. Add proper deduplication of terminal mode changes

### Long-term Fix

1. Redesign the event system to separate key handling from other events
2. Move to a more straightforward UI update approach where rendering happens only after state changes
3. Consolidate focus management into one centralized module
4. Ensure terminal setup/cleanup happens in a well-controlled manner

## Testing

When implementing fixes, use the diagnostic script to verify CPU usage and keyboard responsiveness:

```
./debug_tui.sh
```

Compare the CPU usage and keyboard behavior between all three modes to ensure improvements are working correctly.

## Conclusion

The keyboard input issues appear to be a combination of overly complex event handling, inefficient rendering, and possibly CPU spinning. By simplifying these aspects, we can create a more responsive and user-friendly TUI experience.