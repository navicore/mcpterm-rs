# MCP Resources

This crate provides resource management for the Model Context Protocol (MCP). Resources are data sources and sinks that tools can interact with.

## Features

- Resource identification with URIs
- Resource access control
- File system resources
- Memory resources
- Metadata tracking

## Usage

```rust
use mcp_resources::{ResourceManager, ResourceType, AccessMode};

// Example: Create a resource manager
let mut manager = ResourceManager::new("/path/to/base/dir");

// Register a file resource
let resource_uri = manager.register_file("example.txt", AccessMode::ReadOnly);

// Read from the resource
let content = manager.read_resource(&resource_uri)?;
```