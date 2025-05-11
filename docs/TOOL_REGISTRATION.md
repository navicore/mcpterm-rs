# Tool Registration Architecture

This document describes the architecture and usage of the tool registration system in the MCP project.

## Overview

The MCP project uses a centralized approach for tool registration to ensure consistency and avoid code duplication between different application interfaces (CLI, TUI, etc.). This approach consists of:

1. A shared tool registry in the `mcp-tools` crate that contains all standard tools and their configurations
2. A factory pattern in the `mcp-runtime` crate for creating tool executors
3. Applications simply using these shared components rather than implementing their own tool registration

## Components

### Tool Registry in mcp-tools

The `/crates/mcp-tools/src/registry/mod.rs` module provides:

- Default configurations for shell and filesystem tools with security restrictions
- Functions to create a `ToolManager` with all standard tools registered
- Functions to register standard tools to an existing `ToolManager`

Key functions:

```rust
// Create a tool manager with default configuration
pub fn create_tool_manager() -> ToolManager

// Create a tool manager with custom configuration
pub fn create_tool_manager_with_config(
    shell_config: ShellConfig,
    filesystem_config: FilesystemConfig,
) -> ToolManager

// Register standard tools to an existing tool manager
pub fn register_standard_tools(
    tool_manager: &mut ToolManager,
    shell_config: ShellConfig,
    filesystem_config: FilesystemConfig,
)
```

### Tool Factory in mcp-runtime

The `/crates/mcp-runtime/src/executor/tool_factory.rs` module provides:

- A factory pattern for creating `ToolExecutor` instances
- Methods to create executors with default or custom configurations
- Methods to wrap an existing `ToolManager` in a `ToolExecutor`

Key methods:

```rust
// Create a ToolExecutor with default configuration
pub fn create_executor() -> ToolExecutor

// Create a ToolExecutor with custom configuration
pub fn create_executor_with_config(
    shell_config: ShellConfig,
    filesystem_config: FilesystemConfig,
) -> ToolExecutor

// Create a ToolExecutor with an existing ToolManager
pub fn create_executor_with_manager(tool_manager: ToolManager) -> ToolExecutor
```

## Usage

### In CLI Applications

```rust
// Create a tool manager with all standard tools
let tool_manager = mcp_tools::create_tool_manager();

// Create a tool executor using the factory
let tool_executor = mcp_runtime::executor::ToolFactory::create_executor_with_manager(tool_manager);

// Or more simply:
let tool_executor = mcp_runtime::executor::ToolFactory::create_executor();
```

### In TUI Applications

The TUI application will follow the same pattern as the CLI:

```rust
// Create a tool executor with all standard tools
let tool_executor = mcp_runtime::executor::ToolFactory::create_executor();

// Create a session manager with the tool executor
let session_manager = SessionManager::new(llm_client, tool_executor, event_bus);
```

## Benefits

1. **Consistency**: All applications use the same tools with the same configurations
2. **Centralization**: Tool registration is defined in one place
3. **Maintainability**: Adding or modifying tools only requires changes in one place
4. **Flexibility**: Applications can still customize tool configurations if needed
5. **Security**: Default configurations include security restrictions

## Future Improvements

- Add configurable tool access control with user permission prompts
- Support for custom tools defined by plugins or extensions
- More granular tool configuration options
- Dependency injection for testing