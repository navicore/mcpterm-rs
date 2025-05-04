# MCP Tool Examples

This document provides examples of how to use the tools in the mcpterm CLI application.

## Shell Tool

The shell tool allows execution of commands on the system.

### Example 1: List files in a directory

```json
{
  "tool": "shell",
  "params": {
    "command": "ls -la",
    "timeout_ms": 5000
  }
}
```

### Example 2: Get system information

```json
{
  "tool": "shell",
  "params": {
    "command": "uname -a"
  }
}
```

## File System Tools

### Reading a File

The `file_read` tool allows you to read the contents of a file.

```json
{
  "tool": "file_read",
  "params": {
    "path": "/path/to/file.txt"
  }
}
```

Example response:

```json
{
  "tool_id": "file_read",
  "status": "success",
  "result": {
    "content": "This is the content of the file.\nIt can span multiple lines.\n",
    "size": 54
  }
}
```

### Writing a File

The `file_write` tool allows you to write content to a file, either overwriting or appending.

```json
{
  "tool": "file_write",
  "params": {
    "path": "/path/to/file.txt",
    "content": "This is new content for the file.\n",
    "append": false
  }
}
```

To append to a file instead of overwriting:

```json
{
  "tool": "file_write",
  "params": {
    "path": "/path/to/file.txt",
    "content": "This is additional content.\n",
    "append": true
  }
}
```

Example response:

```json
{
  "tool_id": "file_write",
  "status": "success",
  "result": {
    "success": true,
    "bytes_written": 35
  }
}
```

### Listing Directory Contents

The `directory_list` tool allows you to list the contents of a directory.

```json
{
  "tool": "directory_list",
  "params": {
    "path": "/path/to/directory"
  }
}
```

Example response:

```json
{
  "tool_id": "directory_list",
  "status": "success",
  "result": {
    "entries": [
      {
        "name": "file1.txt",
        "path": "/path/to/directory/file1.txt",
        "type": "file",
        "size": 1024
      },
      {
        "name": "subdirectory",
        "path": "/path/to/directory/subdirectory",
        "type": "directory",
        "size": -1
      }
    ]
  }
}
```

## Safety Considerations

### Path Safety

Tools that access the file system have safety measures in place:

1. Denied paths: Certain system directories are restricted by default
2. Allowed paths: Can be configured to limit access to specific directories
3. Path validation: Prevents path traversal attacks
4. Size limits: Prevents reading or writing excessively large files

### Command Safety

The shell tool also includes safety features:

1. Timeout: Commands have configurable timeouts
2. Allowed commands: Can whitelist specific commands
3. Denied commands: Can blacklist dangerous commands
4. User confirmation: Prompts the user before execution

## Error Handling

All tools return descriptive error messages in a consistent format:

```json
{
  "tool_id": "file_read",
  "status": "error",
  "result": {
    "error": "File not found: /path/to/nonexistent/file.txt"
  }
}
```

or

```json
{
  "tool_id": "file_write",
  "status": "error",
  "result": {
    "error": "Access to this path is not allowed for security reasons"
  }
}
```

## Usage Tips

1. Always check for errors in the tool response
2. Use path validation to ensure paths exist before operations
3. Handle large files appropriately (check size before reading)
4. Use appropriate timeouts for long-running commands
5. Consider user experience when asking for confirmation