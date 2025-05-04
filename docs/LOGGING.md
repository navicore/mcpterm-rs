# Logging in mcpterm-rs

## Overview

mcpterm-rs uses a tracing-based logging system that supports proper log levels through environment variables. This allows you to see detailed information including raw API requests and responses when debugging.

## Log Files

All logs go to a single file for simplicity:

- `/tmp/mcpterm.log`: Unified log file with proper log levels

Using a consistent location in `/tmp` regardless of platform makes logs easy to find.

## Log Levels

The application respects the standard `LOG_LEVEL` environment variable to control logging verbosity. Log levels are hierarchical, meaning that each level includes all higher priority levels:

| Level | Includes | Usage |
|-------|----------|-------|
| `trace` | trace, debug, info, warn, error | Most verbose - shows all logs including raw API requests/responses |
| `debug` | debug, info, warn, error | Detailed information useful for troubleshooting |
| `info` | info, warn, error | Important application events and state changes (default) |
| `warn` | warn, error | Issues that don't break functionality but are concerning |
| `error` | error | Critical errors that prevent functionality from working |

## How to Configure Log Levels

### To see LLM API requests and responses

To see the raw JSON sent to and received from the LLM API, use the `trace` log level:

```bash
LOG_LEVEL=trace mcpterm-cli "Your prompt"
```

### To debug specific components

You can configure different log levels for different parts of the application:

```bash
LOG_LEVEL=info,mcp_llm=trace,mcp_runtime=debug mcpterm-cli "Your prompt"
```

This sets:
- Default log level to `info`
- `mcp_llm` crate to `trace` level (shows API payloads)
- `mcp_runtime` crate to `debug` level

### Using RUST_LOG instead

The system also supports the standard `RUST_LOG` environment variable if you prefer:

```bash
RUST_LOG=trace mcpterm-cli "Your prompt"
```

## Backward Compatibility

For backward compatibility, we maintain the `--verbose` command-line flag which enables verbose logging in the legacy logging system. This is independent of the tracing-based log levels.

## Viewing Logs

To view logs as they're generated, you can use `tail` or a similar utility:

```bash
tail -f /tmp/mcpterm.log
```

For watching only relevant sections of high-volume logs, you might use `grep`:

```bash
tail -f /tmp/mcpterm.log | grep "Raw JSON"
```

## Example

The project includes a comprehensive example that demonstrates the logging system:

```bash
# Run the example with different log levels
cargo run --example logging_levels
LOG_LEVEL=debug cargo run --example logging_levels
LOG_LEVEL=trace cargo run --example logging_levels
```

The example is located at `crates/mcp-core/examples/logging_levels.rs` and demonstrates:
- Basic logging at different levels
- Module-specific logging
- Structured logging with metadata