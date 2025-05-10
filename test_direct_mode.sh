#!/bin/bash
# Test script for direct mode implementation

# Build the latest version
echo "Building mcpterm-tui..."
cargo build --package mcpterm-tui

# Run with direct mode
echo "Running with direct mode..."
echo "Press keys to test input handling, especially Tab and j/k."
echo "Press 'q' to quit."
cargo run --package mcpterm-tui -- --direct-mode

# Run without direct mode for comparison
echo "Running without direct mode for comparison..."
echo "Notice any differences in keyboard handling."
echo "Press 'q' to quit."
cargo run --package mcpterm-tui