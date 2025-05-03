# Logging Guidelines for mcpterm-rs

## Overview

This document outlines the logging approach for the mcpterm-rs project. We use the `tracing` crate for structured logging throughout the application.

## Logging Setup

### Log File Locations

By default, logs are written to standard temporary directories:

- Main log file: `/tmp/mcpterm-debug.log` (or platform-specific temp directory)
- Fallback log file: `/tmp/mcpterm-fallback.log` (in case of main log failure)

These paths will be standardized across platforms with appropriate OS-specific temporary directory detection.

### Initialization

Logging is initialized in the main entry points using `tracing_subscriber`:

```rust
// Initialize logging
tracing_subscriber::fmt::init();
```

In the future, we plan to extend this with:
- Custom file-based logging configuration
- Platform-specific log file locations
- Log rotation and size limits

## Log Levels

Use appropriate log levels throughout the codebase:

| Level | Usage |
|-------|-------|
| `error!` | Critical errors that prevent functionality from working |
| `warn!` | Issues that don't break functionality but are concerning |
| `info!` | Important application events and state changes |
| `debug!` | Detailed information useful for troubleshooting |
| `trace!` | Very verbose information for deep debugging |

## Logging Guidelines

1. **Be Contextual**: Include relevant context in your log messages. Use structured logging where appropriate.

   ```rust
   debug!("Processing user message: {}", message);
   ```

2. **Be Concise**: Keep log messages clear and to the point.

3. **Include Identifiers**: For operations spanning multiple components, include request IDs or other identifiers.

   ```rust
   debug!("Request {} was cancelled", request_id);
   ```

4. **Error Details**: When logging errors, include enough information to diagnose the issue.

   ```rust
   error!("Failed to read conversation context: {:?}", err);
   ```

5. **Performance**: Avoid expensive computations in log statements, particularly at trace/debug levels which might be compiled out in release builds.

## Testing Logging

When implementing tests, you can capture and verify log output using the `tracing_test` crate:

```rust
#[test]
fn test_with_logs() {
    // Will capture logs during test execution
    let _guard = init_test_logging();
    
    // Your test code that produces logs
    
    // Assert logs contain expected messages
    assert_logs_contain("Expected log message");
}
```

## Future Improvements

- Configuration file for adjusting log levels and destinations
- Integration with platform-specific logging systems
- Structured logging with additional metadata
- Log aggregation support