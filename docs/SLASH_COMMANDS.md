# Slash Commands in mcpterm

mcpterm supports special slash commands for various debugging and utility functions. These commands are available in both CLI interactive mode (`mcpterm-cli -I`) and TUI mode.

**Important:** Slash commands are processed locally by the mcpterm application itself, not sent to the LLM. They provide direct access to information about your mcpterm environment.

## Architecture

Slash commands are implemented as a first-class service in the `mcp-core` crate, making them available to both CLI and TUI interfaces. The design follows these principles:

1. **Separation of concerns**: Command handling logic is separate from UI code
2. **Extensibility**: New command types can be added easily by implementing the `SlashCommand` trait
3. **Consistency**: Commands work the same way across all interfaces
4. **Source of truth**: Commands access the actual implementation details directly

## MCP Tool Debugging Commands

The following commands help you debug and understand the MCP tools available to the LLM:

### `/mcp help`

Display help information about available MCP debugging commands.

Example:
```
> /mcp help

=== MCP Debug Commands ===
/mcp help            - Show this help message
/mcp list            - List all available tools
/mcp show <tool_id>  - Show detailed information for a specific tool
/mcp version         - Show MCP client version
```

### `/mcp list`

List all available MCP tools with a brief description.

Example:
```
> /mcp list

=== Available MCP Tools ===
1. shell - Execute shell commands
2. read_file - Read file contents
3. write_file - Write content to a file
4. grep - Search file contents with regex patterns
5. find - Find files matching patterns
...

Use '/mcp show <tool_id>' for detailed information about a specific tool.
```

### `/mcp show <tool_id>`

Show detailed information about a specific tool, including its input and output JSON schemas.

Example:
```
> /mcp show shell

=== Tool: Shell ===
ID: shell
Description: Execute shell commands
Category: Shell

Input Schema:
{
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "description": "The shell command to execute"
    },
    "timeout_ms": {
      "type": "integer",
      "description": "Optional timeout in milliseconds"
    }
  },
  "required": [
    "command"
  ]
}

Output Schema:
{
  "type": "object",
  "properties": {
    "stdout": {
      "type": "string"
    },
    "stderr": {
      "type": "string"
    },
    "exit_code": {
      "type": "integer"
    }
  }
}
```

### `/mcp version`

Display the current MCP client version.

Example:
```
> /mcp version
MCP Client Version: 0.1.0
```

## Implementing New Slash Commands

If you want to implement a new slash command, follow these steps:

1. Create a new implementation of the `SlashCommand` trait in `mcp-core/src/commands/`
2. Add the command to the appropriate UI code (CLI or TUI)

Here's a simplified example:

```rust
use mcp_core::{CommandResult, SlashCommand};

struct MyCommand;

impl SlashCommand for MyCommand {
    fn name(&self) -> &str {
        "mycommand"
    }
    
    fn description(&self) -> &str {
        "My custom command"
    }
    
    fn help(&self) -> &str {
        "/mycommand - Does something useful"
    }
    
    fn execute(&self, args: &[&str]) -> CommandResult {
        // Implement your command logic here
        CommandResult::success("Command executed successfully!")
    }
}
```

## Using Slash Commands

Slash commands are useful for troubleshooting LLM tool interactions. When you're trying to understand why a tool might not be working properly, or you want to see what parameters a tool requires, these commands provide an easy way to inspect the available tools without leaving the mcpterm session.

### Key Features

- **Locally processed:** These commands are handled directly by the mcpterm application, not sent to the LLM
- **Source of truth:** They reflect the exact tool definitions and schemas as implemented in your local mcpterm build
- **Zero context usage:** Using slash commands doesn't consume any of your conversation context with the LLM
- **First-class service:** Commands are available across different interfaces (CLI and TUI)

Remember that slash commands are available in both interactive CLI mode (`mcpterm-cli -I`) and TUI mode.