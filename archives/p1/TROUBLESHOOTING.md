# Troubleshooting mcpterm-rs

This document provides troubleshooting tips for common issues with mcpterm-rs.

## Timeout Issues

We've significantly increased all timeout values to prevent timeout issues:
- Command execution timeout: 240 seconds (4 minutes)
- LLM processing timeout: 240 seconds (4 minutes)
- Safety timeout: 240 seconds (4 minutes)

## Log File Locations

The application now writes logs to consistent locations that are easy to find:
- Main log file: `/tmp/mcpterm-debug.log`
- Fallback log file: `/tmp/mcpterm-fallback.log`

These locations are also printed to the console when the application starts.

You can view the logs at any time with:
```bash
tail -f /tmp/mcpterm-debug.log
```

## Running with Debug Mode

Use the included script to run mcpterm with enhanced debugging:

```bash
./run_with_debug.sh
```

This script:
1. Builds the application
2. Runs it with RUST_BACKTRACE=1 and RUST_LOG=debug
3. Shows log file locations

## Common Issues and Solutions

### UI Locks Up

If the UI locks up:
1. Press ESC to attempt to cancel the current operation
2. Check the log files for error messages
3. Restart the application
4. Use simpler commands initially

### Commands Timeout

If commands are timing out:
1. Check that the command isn't too complex
2. Verify shell access/permissions
3. For long-running commands, try breaking them into smaller steps
4. Check log files for detailed error messages

### Debug Messages on Screen

This should be fixed now. All debug messages are sent only to log files, never to stderr or stdout. If you still see debug messages on screen, please check the log files and report the issue.

## Reporting Issues

If you continue to experience issues:
1. Collect the log files
2. Note the exact steps to reproduce the issue
3. Report it via GitHub issues

## Customizing Timeouts

Timeouts can be further adjusted by modifying the application configuration. The default `config.json` file is created in your config directory the first time you run the application.