#!/bin/bash
# Script to test different TUI implementations and debug the keyboard issues

# Function to display section headers
section() {
  echo
  echo "==============================================="
  echo "$1"
  echo "==============================================="
  echo
}

# Build the project
section "Building mcpterm-tui"
cargo build --package mcpterm-tui

# Function to check for CPU usage
check_cpu() {
  local pid=$1
  local name=$2
  echo "CPU usage for $name (PID: $pid):"
  top -pid $pid -l 2 -n 1 -stats cpu | grep CPU
  echo
}

# Ultra-simple mode test
section "TESTING ULTRA-SIMPLE MODE"
echo "This runs the simplest possible implementation"
echo "It should definitely handle keyboard input properly"
echo "Type some text and press Enter to test"
echo "Press 'q' to quit"
echo
echo "Press Enter to start..."
read

# Run in background and get PID
cargo run --package mcpterm-tui -- --simple-mode &
simple_pid=$!

# Wait a bit for process to start
sleep 2

# Check CPU usage
check_cpu $simple_pid "Ultra-simple mode"

# Wait for process to complete 
echo "Testing simple mode. Press Enter after you're done testing (after pressing 'q')..."
read
kill -9 $simple_pid 2>/dev/null

# Direct mode test
section "TESTING DIRECT MODE"
echo "This runs our direct implementation with AppState"
echo "It should handle keyboard input better than standard mode"
echo "Type some text and press Enter to test"
echo "Press 'q' to quit"
echo
echo "Press Enter to start..."
read

# Run in background and get PID
cargo run --package mcpterm-tui -- --direct-mode &
direct_pid=$!

# Wait a bit for process to start
sleep 2

# Check CPU usage
check_cpu $direct_pid "Direct mode"

# Wait for process to complete
echo "Testing direct mode. Press Enter after you're done testing (after pressing 'q')..."
read
kill -9 $direct_pid 2>/dev/null

# Standard mode test
section "TESTING STANDARD MODE"
echo "This runs the standard implementation with complex event system"
echo "It may have keyboard input issues we're debugging"
echo "Type some text and press Enter to test"
echo "Press 'q' to quit"
echo
echo "Press Enter to start..."
read

# Run in background and get PID
cargo run --package mcpterm-tui &
standard_pid=$!

# Wait a bit for process to start
sleep 2

# Check CPU usage
check_cpu $standard_pid "Standard mode"

# Wait for process to complete
echo "Testing standard mode. Press Enter after you're done testing (after pressing 'q')..."
read
kill -9 $standard_pid 2>/dev/null

section "TESTING COMPLETE"
echo "Compare the CPU usage between the different modes."
echo "If simple mode has much lower CPU usage, the issue might be CPU spinning."
echo "If all have similar CPU usage but different keyboard behavior, the issue is in event handling."