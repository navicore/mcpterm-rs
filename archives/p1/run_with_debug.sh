#!/bin/bash

# Build the application first
cargo build

# Run mcpterm with RUST_BACKTRACE and RUST_LOG set
echo "Starting mcpterm with verbose logging..."
echo "Log files will be in your temp directory."
RUST_BACKTRACE=1 RUST_LOG=debug ./target/debug/mcpterm "$@"