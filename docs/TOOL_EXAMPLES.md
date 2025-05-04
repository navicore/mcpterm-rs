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

## Diff and Patch Tools

### Comparing Files or Text

The `diff` tool allows you to compare files or text content and generate differences in various formats.

#### Example 1: Compare two files

```json
{
  "tool": "diff",
  "params": {
    "old_file": "/path/to/original.txt",
    "new_file": "/path/to/modified.txt",
    "output_format": "unified",
    "context_lines": 3
  }
}
```

#### Example 2: Compare text content

```json
{
  "tool": "diff",
  "params": {
    "old_content": "This is the original text.\nIt has multiple lines.\nSome content here.",
    "new_content": "This is the original text.\nIt has been modified.\nSome content here.",
    "output_format": "inline",
    "ignore_whitespace": true
  }
}
```

Example response:

```json
{
  "tool_id": "diff",
  "status": "success",
  "result": {
    "diff": "@@ -1,3 +1,3 @@\n This is the original text.\n-It has multiple lines.\n+It has been modified.\n Some content here.",
    "stats": {
      "inserted": 1,
      "deleted": 1,
      "unchanged": 2
    },
    "files_compared": [
      "/path/to/original.txt",
      "/path/to/modified.txt"
    ]
  }
}
```

### Applying Patches

The `patch` tool allows you to apply patches in unified diff format to files.

#### Example 1: Apply a patch to a file

```json
{
  "tool": "patch",
  "params": {
    "target_file": "/path/to/file.txt",
    "patch_content": "@@ -1,3 +1,3 @@\n Line 1\n-Line 2\n+Modified Line 2\n Line 3",
    "create_backup": true
  }
}
```

#### Example 2: Dry run to test a patch

```json
{
  "tool": "patch",
  "params": {
    "target_file": "/path/to/file.txt",
    "patch_content": "@@ -1,3 +1,3 @@\n Line 1\n-Line 2\n+Modified Line 2\n Line 3",
    "dry_run": true
  }
}
```

Example response:

```json
{
  "tool_id": "patch",
  "status": "success",
  "result": {
    "success": true,
    "target_file": "/path/to/file.txt",
    "backup_created": "/path/to/file.txt.bak",
    "hunks_applied": 1,
    "hunks_failed": 0,
    "conflicts": []
  }
}
```

## Project Navigation Tool

The `project` tool analyzes project structure, type, dependencies, and entry points to help you understand codebases more easily.

### Example 1: Basic project analysis

```json
{
  "tool": "project",
  "params": {
    "project_dir": "/path/to/project",
    "include_hidden": false,
    "analyze_dependencies": true
  }
}
```

### Example 2: Deep project analysis with hidden files

```json
{
  "tool": "project",
  "params": {
    "project_dir": "/path/to/project",
    "include_hidden": true,
    "analyze_dependencies": true,
    "max_depth": 10
  }
}
```

Example response:

```json
{
  "tool_id": "project",
  "status": "success",
  "result": {
    "project_type": "Rust",
    "structure": {
      "path": "project",
      "is_dir": true,
      "size": 0,
      "children": [
        {
          "path": "Cargo.toml",
          "is_dir": false,
          "size": 245,
          "file_type": "config"
        },
        {
          "path": "src",
          "is_dir": true,
          "size": 0,
          "file_type": "source",
          "children": [
            {
              "path": "src/main.rs",
              "is_dir": false,
              "size": 45,
              "file_type": "source"
            },
            {
              "path": "src/lib.rs",
              "is_dir": false,
              "size": 120,
              "file_type": "source"
            }
          ]
        }
      ],
      "file_type": "project-root"
    },
    "directories": {
      "src": "Source code directory",
      "tests": "Tests directory",
      "examples": "Example code"
    },
    "entry_points": [
      {
        "path": "src/main.rs",
        "entry_type": "main",
        "description": "Main executable entry point"
      },
      {
        "path": "src/lib.rs",
        "entry_type": "library",
        "description": "Library crate entry point"
      }
    ],
    "dependencies": [
      {
        "name": "serde",
        "version": "1.0",
        "is_dev": false
      },
      {
        "name": "tokio",
        "version": "1",
        "is_dev": false
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
6. When using diff and patch tools:
   - Always create backups when patching important files
   - Use dry run mode to test patches before applying them
   - Be aware that patches may fail if the context doesn't match