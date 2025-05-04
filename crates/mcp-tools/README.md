# MCP Tools

This crate provides tool implementations for the Model Context Protocol (MCP). These tools allow LLMs to interact with the filesystem, execute commands, and more.

## Modules

- `registry`: Tool registration and management
- `shell`: Shell command execution tools
- `filesystem`: File operations tools (read, write, list)
- `search`: File and content search tools

## Usage

```rust
use mcp_tools::{Tool, ToolManager, ToolCategory};
use mcp_tools::shell::ShellTool;

// Example: Create a tool manager
let mut manager = ToolManager::new();

// Register a shell tool
manager.register_tool(Box::new(ShellTool::new()));

// Execute a tool
let result = manager.execute_tool("shell", json!({
    "command": "ls -la",
    "timeout": 5000
}))?;
```
