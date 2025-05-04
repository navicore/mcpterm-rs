use mcpterm_cli::formatter::ResponseFormatter;
use mcp_tools::{ToolResult, ToolStatus};
use serde_json::json;

fn main() {
    // Test shell tool result
    let shell_result = ToolResult {
        tool_id: "shell".to_string(),
        status: ToolStatus::Success,
        output: json!({
            "command": "ls -la",
            "exit_code": 0,
            "stdout": "total 32\ndrwxr-xr-x  5 user  staff  160 May  3 12:34 .\ndrwxr-xr-x  3 user  staff   96 May  3 12:30 ..\n-rw-r--r--  1 user  staff  185 May  3 12:34 Cargo.toml\n-rw-r--r--  1 user  staff  976 May  3 12:34 README.md\ndrwxr-xr-x  4 user  staff  128 May  3 12:34 src",
            "stderr": ""
        }),
        error: None,
    };
    
    // Test file tool result
    let file_result = ToolResult {
        tool_id: "read_file".to_string(),
        status: ToolStatus::Success,
        output: json!({
            "path": "/path/to/file.txt",
            "content": "This is the content of the file.\nIt has multiple lines.\nAnd contains important information.",
            "size": 123
        }),
        error: None,
    };
    
    // Test directory tool result
    let dir_result = ToolResult {
        tool_id: "list_directory".to_string(),
        status: ToolStatus::Success,
        output: json!({
            "path": "/path/to/directory",
            "entries": [
                {
                    "name": "file1.txt",
                    "type": "file",
                    "size": 1024
                },
                {
                    "name": "file2.txt",
                    "type": "file",
                    "size": 2048
                },
                {
                    "name": "subdirectory",
                    "type": "directory",
                    "size": 0
                }
            ]
        }),
        error: None,
    };
    
    // Test error result
    let error_result = ToolResult {
        tool_id: "shell".to_string(),
        status: ToolStatus::Failure,
        output: json!({
            "command": "some_invalid_command",
            "exit_code": 127,
            "stdout": "",
            "stderr": "command not found: some_invalid_command"
        }),
        error: Some("Command execution failed with exit code 127".to_string()),
    };
    
    // Format and display each result
    println!("===== SHELL COMMAND RESULT =====");
    println!("{}", ResponseFormatter::format_tool_result(&shell_result));
    
    println!("\n===== FILE READ RESULT =====");
    println!("{}", ResponseFormatter::format_tool_result(&file_result));
    
    println!("\n===== DIRECTORY LISTING RESULT =====");
    println!("{}", ResponseFormatter::format_tool_result(&dir_result));
    
    println!("\n===== ERROR RESULT =====");
    println!("{}", ResponseFormatter::format_tool_result(&error_result));
    
    // Test JSON-RPC result parsing
    println!("\n===== JSON-RPC PARSING =====");
    let json_rpc_result = r#"{"jsonrpc": "2.0", "result": {"tool_id": "shell", "status": "success", "output": {"command": "echo hello", "exit_code": 0, "stdout": "hello\n", "stderr": ""}, "error": null}, "id": "tool_result"}"#;
    
    if let Some(formatted) = ResponseFormatter::extract_from_jsonrpc(json_rpc_result) {
        println!("Parsed JSON-RPC result:\n{}", formatted);
    } else {
        println!("Failed to parse JSON-RPC result");
    }
}