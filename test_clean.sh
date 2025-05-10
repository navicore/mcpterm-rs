#!/bin/bash
# Script to test the clean implementation

echo "Building mcpterm-tui..."
cargo build --package mcpterm-tui

echo ""
echo "Running clean implementation with proper scrolling..."
echo "INSTRUCTIONS:"
echo "- Try typing text in the input area"
echo "- Press Enter to submit the text"
echo "- Press Tab to switch focus to the message area"
echo "- Use j/k to scroll up and down in the message area"
echo "- Use the mouse wheel to scroll up and down as well"
echo "- Press Tab again to return to the input area"
echo "- Press i to enter insert mode and type more text"
echo "- Press Esc to go back to normal mode"
echo "- Press q to quit (in normal mode)"
echo ""
echo "Press Enter to start..."
read

cargo run --package mcpterm-tui -- --clean-mode

echo ""
echo "Test complete!"