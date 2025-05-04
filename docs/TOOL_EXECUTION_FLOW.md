# Tool Execution Flow

This document outlines the design for tool execution in the MCPTerm CLI application.

## Overview

The tool execution flow allows the LLM to use tools like shell commands, file system operations, and search capabilities to perform actions and retrieve information from the user's system.

## Flow Diagram

```
┌─────────────┐       ┌─────────────┐        ┌─────────────┐        ┌─────────────┐
│  User Input  │──────▶│   CLI App   │───────▶│  LLM Client │────────▶│    LLM API   │
└─────────────┘       └──────┬──────┘        └─────────────┘        └──────┬──────┘
                             │                                             │
                             │                                             │
                 ┌───────────▼───────────┐                    ┌────────────▼─────────┐
                 │  Process LLM Response  │◀───────────────────│  LLM Returns Result  │
                 └───────────┬───────────┘                    └────────────┬─────────┘
                             │                                             │
                             │                                             │
         ┌─────────────┐     │     ┌────────────────┐          ┌──────────▼─────────┐
         │ Show Result │◀────┴─────│   Tool Call?   │◀─────────│  Parse LLM Response │
         └─────────────┘           └────────┬───────┘          └────────────────────┘
                                            │
                                            │ Yes
                                            ▼
                                  ┌────────────────────┐
                                  │  Validate Request  │
                                  └──────────┬─────────┘
                                             │
                                             │
                                  ┌──────────▼─────────┐
                                  │  Apply Safety Rules │
                                  └──────────┬─────────┘
                                             │
                                             │
                                  ┌──────────▼─────────┐
                                  │ Execute Tool Action │
                                  └──────────┬─────────┘
                                             │
                                             │
                                  ┌──────────▼─────────┐
                                  │  Format Result      │
                                  └──────────┬─────────┘
                                             │
                                             │
                                  ┌──────────▼─────────┐
                                  │ Send Result to LLM  │
                                  └────────────────────┘
```

## Components and Responsibilities

### 1. CLI App (`mcpterm-cli`)
- Manages the conversation context
- Sends user messages to LLM
- Processes LLM responses
- Detects tool calls in responses
- Coordinates tool execution
- Sends tool results back to LLM

### 2. Tool Manager (`mcp-tools`)
- Registers available tools
- Validates tool requests
- Executes tool actions
- Formats tool results
- Applies safety measures
- Returns tool execution results

### 3. Tools Implementation

#### Shell Tool
- Execute shell commands on the system with safety measures
- Configurable timeouts for command execution
- Allow/deny lists for command execution
- Resource usage limitations
- Sanitized command input and output formatting

#### Filesystem Tools
- **ReadFileTool**: Safely read file contents with size limitations
  - Input: File path
  - Output: File content and size
  - Safety: Path validation, file size limits, content truncation
  
- **WriteFileTool**: Write or append content to files
  - Input: File path, content, and append flag
  - Output: Success status and bytes written
  - Safety: Path validation, content size limits, parent directory creation
  
- **ListDirectoryTool**: List directory contents with metadata
  - Input: Directory path
  - Output: List of entries with name, path, type, and size
  - Safety: Path validation, limited depth traversal

#### Search Tools (Planned)
- Find files by patterns and metadata
- Search file contents using regular expressions
- Code-aware search capabilities

#### System Info Tools (Planned)
- Get system information and metrics
- Monitor resource usage
- Access environment information

## Tool Execution Process

1. **Detect Tool Call**:
   - Parse the LLM response to identify tool calls
   - Extract tool name and parameters

2. **Validate Request**:
   - Check if the requested tool exists
   - Validate parameters against the tool's schema
   - Apply safety rules based on tool type

3. **Execute Tool**:
   - Invoke the tool with validated parameters
   - Capture the execution result (success/failure)
   - Format the result in the expected schema

4. **Continue Conversation**:
   - Send the tool result back to the LLM
   - Continue the conversation with the updated context

## Safety Considerations

Tool execution requires careful consideration of security and safety:

1. **Shell Command Execution**:
   - Restrict dangerous commands (rm -rf, etc.)
   - Apply timeouts to prevent hung processes
   - Limit resource usage
   - Sanitize command input
   - Log all executed commands for audit

2. **Filesystem Operations**:
   - Restrict access to sensitive directories
   - Prevent modification of system files
   - Validate paths for traversal attacks
   - Limit file sizes for read/write operations

3. **User Confirmation**:
   - Prompt for confirmation before executing potentially risky operations
   - Allow users to configure approval settings for different tool categories
   - Provide clear information about what the tool will do before execution

## Implementation Strategy

The implementation will follow these steps:

1. ✅ Complete the `ShellTool` implementation with safety measures
2. ✅ Implement basic filesystem tools (read, write, list)
   - ✅ FilesystemBaseTool with path validation
   - ✅ ReadFileTool for safe file reading
   - ✅ WriteFileTool for safe file writing
   - ✅ ListDirectoryTool for directory listing
3. 🔄 Implement search tools (in progress)
4. ✅ Integrate tool execution in the CLI app
5. ✅ Add user confirmation mechanism
6. ✅ Implement tool result formatting for LLM consumption
7. ✅ Add configurable safety settings

## Configuration

Users will be able to configure:

1. Which tools are enabled/disabled
2. Safety levels for different tool categories
3. Confirmation requirements for different actions
4. Timeouts and resource limits
5. Directories and files that can be accessed