#!/bin/bash

# Find the temp directory 
TEMP_DIR=$(mktemp -d -t tmp.XXXXXXXXXX)
rm -rf "$TEMP_DIR"
TEMP_DIR=$(dirname "$TEMP_DIR")

echo "Your temporary directory is: $TEMP_DIR"
echo "Looking for log files..."

# Look for the log files
MAIN_LOG="$TEMP_DIR/mcpterm-debug.log"
FALLBACK_LOG="$TEMP_DIR/mcpterm-fallback.log"

if [ -f "$MAIN_LOG" ]; then
    echo "Found main log file: $MAIN_LOG"
    echo "Last 10 lines of main log:"
    tail -n 10 "$MAIN_LOG"
else
    echo "Main log file not found at: $MAIN_LOG"
fi

if [ -f "$FALLBACK_LOG" ]; then
    echo "Found fallback log file: $FALLBACK_LOG"
    echo "Last 10 lines of fallback log:"
    tail -n 10 "$FALLBACK_LOG"
else
    echo "Fallback log file not found at: $FALLBACK_LOG"
fi