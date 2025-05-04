# Tool Implementation Plan

This document outlines the implementation plan for enhancing the MCPTerm tool suite to better support complex coding workflows. The enhancements are prioritized based on their impact on the LLM's ability to perform multi-step coding tasks.

## 1. Search Tools Implementation (High Priority)

Search tools are critical for helping the LLM navigate and understand code, making them the highest priority.

### 1.1. GrepTool

Purpose: Search file contents for patterns (regex or literal).

Implementation:
1. Create basic tool structure in `mcp-tools/src/search/grep.rs`
2. Use the `regex` crate for pattern matching
3. Implement path filtering with glob patterns
4. Add security checks similar to other filesystem tools
5. Include context lines around matches (similar to `grep -A/-B/-C`)
6. Add result formatting options

Interface:
```rust
// Tool parameters
{
    "pattern": "function\\s+\\w+",   // Regex pattern to find
    "path": "./src",                 // Directory to search in
    "include": "*.{rs,toml}",        // File pattern to include
    "exclude": "target/**",          // File pattern to exclude
    "context_lines": 2,              // Lines of context before/after matches
    "max_results": 100,              // Maximum number of results to return
    "case_sensitive": false          // Whether to perform case-sensitive matching
}

// Tool result
{
    "matches": [
        {
            "file": "/path/to/file.rs",
            "line": 42,
            "column": 5, 
            "match": "function main() {",
            "context_before": ["// Entry point", "// Called when program starts"],
            "context_after": ["    println!(\"Hello, world!\");", "    process_args();"]
        },
        // More matches...
    ],
    "total_matches": 15,
    "searched_files": 25
}
```

### 1.2. FindTool

Purpose: Find files matching name patterns (glob).

Implementation:
1. Create tool in `mcp-tools/src/search/find.rs`
2. Use the `globset` crate for flexible pattern matching
3. Add options for recursive search
4. Implement modification time filtering
5. Include file metadata in results
6. Add security checks and sandboxing

Interface:
```rust
// Tool parameters
{
    "pattern": "**/*.rs",          // Glob pattern to match files
    "base_dir": "./src",           // Base directory for search
    "exclude": "**/*.tmp",         // Files to exclude
    "max_depth": 5,                // Maximum directory depth to search
    "modified_after": "2023-01-01", // Only find files modified after this date
    "modified_before": "2023-12-31", // Only find files modified before this date
    "sort_by": "modified_time",    // Sort results by (name, size, modified_time)
    "order": "desc"                // Sort order (asc, desc)
}

// Tool result
{
    "files": [
        {
            "path": "/path/to/file.rs",
            "name": "file.rs",
            "size": 1024,
            "is_dir": false,
            "modified_time": "2023-06-15T14:35:42Z"
        },
        // More files...
    ],
    "total_files": 15,
    "searched_dirs": 8
}
```

## 2. Diff Tools Implementation (Medium Priority)

### 2.1. DiffTool

Purpose: Compare files or strings and show differences.

Implementation:
1. Create tool in `mcp-tools/src/diff/mod.rs`
2. Use the `similar` or `diff` crate for text comparison
3. Implement different output formats (unified, context, side-by-side)
4. Add whitespace-ignoring option
5. Support line-level and word-level diffs
6. Include security checks for file access

Interface:
```rust
// Tool parameters
{
    "old_content": "function hello() {\n  console.log('hello');\n}", // Old content or null
    "new_content": "function hello() {\n  console.log('hello world');\n}", // New content or null
    "old_file": "/path/to/old.js",  // Alternative to old_content
    "new_file": "/path/to/new.js",  // Alternative to new_content
    "context_lines": 3,             // Lines of context around changes
    "ignore_whitespace": true,      // Whether to ignore whitespace
    "output_format": "unified"      // Output format (unified, context, side-by-side)
}

// Tool result
{
    "diff": "@@ -1,3 +1,3 @@\n function hello() {\n-  console.log('hello');\n+  console.log('hello world');\n }",
    "changes": {
        "inserted": 1,
        "deleted": 1,
        "modified": 1,
        "unchanged": 1
    },
    "files_compared": ["old.js", "new.js"]
}
```

### 2.2. PatchTool

Purpose: Apply patches to files.

Implementation:
1. Create tool in `mcp-tools/src/diff/patch.rs`
2. Use the `patch` or `similar` crate for patching
3. Add dry-run option to test patches before applying
4. Implement backup creation
5. Include conflict resolution options
6. Add security checks similar to filesystem tools

Interface:
```rust
// Tool parameters
{
    "target_file": "/path/to/file.js", // File to patch
    "patch_content": "@@ -1,3 +1,3 @@\n...", // Patch in unified diff format
    "create_backup": true,              // Whether to backup the original file
    "dry_run": false,                   // Whether to simulate without changing files
    "ignore_whitespace": false          // Whether to ignore whitespace when applying
}

// Tool result
{
    "success": true,
    "patched_file": "/path/to/file.js",
    "backup_created": "/path/to/file.js.bak",
    "hunks_applied": 3,
    "hunks_failed": 0,
    "conflicts": []
}
```

## 3. Code Analysis Tools (Medium Priority)

These tools require more complex implementation but provide substantial benefits for code understanding.

### 3.1. Project Navigator

Purpose: Analyze project structure and relationships between files.

Implementation:
1. Create tool in `mcp-tools/src/analysis/project.rs`
2. Implement basic directory structure analysis
3. Recognize common project patterns (src, test, etc.)
4. Add detection of build files (Cargo.toml, package.json, etc.)
5. Include language-specific project recognition

Interface:
```rust
// Tool parameters
{
    "project_dir": "/path/to/project", // Project root directory
    "include_hidden": false,            // Whether to include hidden files
    "analyze_dependencies": true,       // Whether to analyze dependencies
    "max_depth": 3                      // Maximum depth to analyze
}

// Tool result
{
    "project_type": "rust",
    "structure": {
        "src": {
            "type": "directory",
            "files": ["main.rs", "lib.rs"],
            "subdirs": ["modules", "utils"]
        },
        "tests": {
            "type": "directory",
            "files": ["integration_test.rs"]
        },
        "Cargo.toml": {
            "type": "file",
            "size": 1024
        }
    },
    "entrypoints": ["src/main.rs"],
    "dependencies": [
        {"name": "serde", "version": "1.0"},
        {"name": "tokio", "version": "1.0"}
    ]
}
```

### 3.2. Language-Specific Analyzers

Purpose: Provide language-aware code analysis.

Implementation:
1. Create `mcp-tools/src/analysis/languages/` directory
2. Implement simple analyzers for key languages:
   - Rust (`rust.rs`)
   - JavaScript/TypeScript (`js.rs`)
   - Python (`python.rs`)
3. Use regex-based parsing for initial implementation
4. Consider tree-sitter for more advanced parsing later
5. Focus on key features:
   - Import/require statements
   - Function/class definitions
   - Symbol references

Interface:
```rust
// Tool parameters
{
    "file": "/path/to/file.rs",         // File to analyze
    "content": "fn main() {...}",        // Alternative to file
    "analysis_type": "definitions",      // Type of analysis to perform
    "detail_level": "high"               // How much detail to include
}

// Tool result
{
    "language": "rust",
    "definitions": [
        {
            "type": "function",
            "name": "main",
            "line": 10,
            "args": [],
            "return_type": "()",
            "visibility": "public"
        },
        // More definitions...
    ],
    "imports": [
        {
            "module": "std::io",
            "line": 1,
            "items": ["Read", "Write"]
        }
    ],
    "usages": [
        {
            "name": "println",
            "line": 12,
            "column": 5,
            "context": "println!(\"Hello, world!\");"
        }
    ]
}
```

## 4. Testing Tools (Medium Priority)

### 4.1. Test Runner

Purpose: Run and analyze test results.

Implementation:
1. Create tool in `mcp-tools/src/testing/runner.rs`
2. Detect common test frameworks based on project type
3. Support running specific tests
4. Parse test output for structured results
5. Implement test filtering options
6. Add timeout controls

Interface:
```rust
// Tool parameters
{
    "command": "cargo test",            // Test command to run
    "dir": "/path/to/project",          // Directory to run in
    "filter": "test_user_login",        // Test filter pattern
    "timeout_seconds": 30,              // Maximum execution time
    "env_vars": {"RUST_BACKTRACE": "1"} // Environment variables
}

// Tool result
{
    "success": true,
    "total_tests": 25,
    "passed": 23,
    "failed": 2,
    "skipped": 0,
    "execution_time_ms": 1536,
    "test_results": [
        {
            "name": "test_user_login_valid",
            "status": "passed",
            "duration_ms": 45
        },
        {
            "name": "test_user_login_invalid",
            "status": "failed",
            "duration_ms": 52,
            "error": "assertion failed: `(left == right)`\n  left: `Unauthorized`,\n right: `BadRequest`"
        }
    ],
    "output": "running 25 tests\ntest test_user_login_valid ... ok\n..."
}
```

## Implementation Timeline

| Phase | Tool | Priority | Estimated Effort | Dependencies |
|-------|------|----------|------------------|--------------|
| 1     | GrepTool | High | 2 days | None |
| 1     | FindTool | High | 1 day | None |
| 2     | DiffTool | Medium | 3 days | None |
| 2     | Project Navigator | Medium | 3 days | FindTool |
| 3     | Language Analyzers (basic) | Medium | 4 days | None |
| 3     | PatchTool | Medium | 2 days | DiffTool |
| 4     | Test Runner | Medium | 3 days | None |
| 4     | Language Analyzers (advanced) | Low | 5 days | Language Analyzers (basic) |

## Implementation Guidelines

To maintain code quality and consistency with the existing architecture:

1. **Follow Existing Patterns**:
   - Match current tool implementation style
   - Reuse security checking code
   - Follow error handling patterns

2. **Incremental Development**:
   - Implement basic functionality first
   - Add advanced features after basic version works
   - Use feature flags for experimental capabilities

3. **Testing Strategy**:
   - Create unit tests for individual tools
   - Add integration tests for tool interaction
   - Include example usage in documentation

4. **Security Considerations**:
   - Apply same security constraints as existing tools
   - Add tool-specific security checks as needed
   - Document security model for each tool

5. **Documentation**:
   - Document tool purpose and capabilities
   - Include example inputs and outputs
   - Note limitations and edge cases