# Tools Implementation

This document describes the tool system in the MCP project, including architecture, registration, execution flow, and component details.

## Overview

The MCPTerm project implements a flexible tool system that allows the LLM to execute commands, access the file system, search code, and perform other operations to assist with coding tasks. The architecture is designed to be modular, extensible, and to work with different user interfaces (CLI, TUI).

Tools allow the LLM to:
- Execute shell commands
- Read and write files
- Search for files and patterns
- Analyze code
- Create and apply patches
- Run tests
- And more...

## Architecture

### Component Structure

```
+---------------+                 +---------------+
|     UI        |                 |     LLM       |
| (CLI or TUI)  |                 |               |
+-------+-------+                 +-------+-------+
        |                                 |
        |       +-------------------+     |
        +------>|    Event Bus      |<----+
                +-------------------+
                         |
                         v
                +-------------------+
                |  Session Manager  |
                +-------------------+
                         |
                         v
                +-------------------+
                |   Tool Executor   |
                +-------------------+
                         |
                         v
                +-------------------+
                |   Tool Manager    |
                +-------------------+
                         |
                         v
                +-------------------+
                | Individual Tools  |
                +-------------------+
```

### Tool Registration

The project uses a centralized approach for tool registration to ensure consistency and avoid code duplication between different application interfaces (CLI, TUI):

1. A shared tool registry in the `mcp-tools` crate that contains all standard tools and their configurations
2. A factory pattern in the `mcp-runtime` crate for creating tool executors
3. Applications use these shared components rather than implementing their own tool registration

#### Tool Registry in mcp-tools

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

#### Tool Factory in mcp-runtime

The `/crates/mcp-runtime/src/executor/tool_factory.rs` module provides:

- A factory pattern for creating `ToolExecutor` instances
- Methods to create executors with default or custom configurations
- Methods to wrap a shared `ToolManager` in a `ToolExecutor`

Key methods:
```rust
// Create a ToolExecutor with default configuration
pub fn create_executor() -> ToolExecutor

// Create a ToolExecutor with custom configuration
pub fn create_executor_with_config(
    shell_config: ShellConfig,
    filesystem_config: FilesystemConfig,
) -> ToolExecutor

// Create a ToolExecutor with a shared ToolManager
pub fn create_executor_with_shared_manager(tool_manager: Arc<ToolManager>) -> ToolExecutor

// Create a shared ToolManager with default configuration
pub fn create_shared_tool_manager() -> Arc<ToolManager>
```

## Execution Flow

The tool execution flow involves several components:

1. **User Interface (CLI/TUI)**
   - Captures user input
   - Displays assistant responses
   - Shows tool execution results

2. **Event Bus**
   - Facilitates communication between components
   - Manages event queues and handlers
   - Routes events to appropriate handlers

3. **Session Manager**
   - Maintains conversation context
   - Processes LLM responses
   - Detects and extracts tool calls
   - Forwards tool calls to the Tool Executor

4. **Tool Executor**
   - Applies safety constraints
   - Tracks tool execution metrics
   - Delegates actual execution to Tool Manager

5. **Tool Manager**
   - Maintains a registry of available tools
   - Looks up tools by ID
   - Invokes the appropriate tool with parameters

6. **Individual Tools**
   - Implement the actual functionality
   - Apply tool-specific constraints
   - Return results to the caller

### Flow Sequence

1. User sends a message via UI
2. Event bus delivers message to Session Manager
3. Session Manager sends message to LLM
4. LLM responds with content (potentially including tool calls)
5. Session Manager extracts tool calls from response
6. Tool calls are sent to Tool Executor
7. Tool Executor applies safety constraints
8. Tool Manager looks up and executes the tool
9. Results are sent back to Session Manager
10. Session Manager sends results back to LLM for continuation
11. Final response is displayed to user

### JSON-RPC Format

Tool calls can be embedded in LLM responses in JSON-RPC format:

```json
{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "shell",
    "parameters": {
      "command": "ls -la",
      "description": "List files in current directory"
    }
  },
  "id": "abc123"
}
```

## Implemented Tools

The MCP project implements various tools, each serving a specific purpose:

### 1. Shell Tool

The `ShellTool` allows execution of shell commands with safety constraints:
- Configurable timeout
- Allowlist/denylist for commands
- Result capturing and formatting

### 2. Filesystem Tools

- `ReadFileTool`: Read files with size limits and path restrictions
- `WriteFileTool`: Write/modify files with safety constraints
- `ListDirectoryTool`: List directory contents with filtering

### 3. Search Tools

- `GrepTool`: Search file contents with regex support
- `FindTool`: Find files matching name patterns (glob)

### 4. Diff/Patch Tools

- `DiffTool`: Compare files or text
- `PatchTool`: Apply changes to files

### 5. Analysis Tools

- `LanguageAnalyzerTool`: Parse and analyze code
- `ProjectNavigator`: Help navigate project structure

### 6. Testing Tools

- `TestRunnerTool`: Run tests with various configurations

## Security Considerations

The tool system includes several safety measures:

1. **Path Restrictions**
   - Denied paths to protect sensitive directories
   - Optional allowed paths for sandboxing

2. **Command Restrictions**
   - Denied commands to prevent dangerous operations
   - Optional allowed commands for a stricter sandbox

3. **Resource Limits**
   - Timeouts for long-running operations
   - Size limits for file operations

4. **User Confirmation**
   - Optional confirmation for tool execution
   - Auto-approval options for trusted environments

## Implementation Notes

- Tools are registered on startup based on configuration
- Tools are wrapped in Arc for efficient sharing between components
- Tool identification is case-sensitive
- Tool parameters must match the expected schema
- Tool execution is asynchronous

## Future Improvements

- Enhanced permission model for tools
- Plugin system for third-party tools
- More granular configuration options
- Tool execution profiling and optimization