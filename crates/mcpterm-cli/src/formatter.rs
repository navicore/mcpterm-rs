use mcp_tools::{ToolResult, ToolStatus};
use serde_json::{json, Value};
use std::fmt::Write;
use tracing::{debug, warn};

/// Format tool results in a human-friendly way
pub struct ResponseFormatter;

impl ResponseFormatter {
    /// Format a tool result for user-friendly display
    pub fn format_tool_result(result: &ToolResult) -> String {
        let mut output = String::new();
        
        // Format status header with appropriate styling
        let status_str = match result.status {
            ToolStatus::Success => "SUCCESS",
            ToolStatus::Failure => "FAILURE",
            ToolStatus::Timeout => "TIMEOUT",
        };
        
        // Add the status header
        writeln!(&mut output, "┌─────────────────────────────────┐").unwrap();
        writeln!(&mut output, "│ Tool Result: {}{}│", status_str, " ".repeat(21 - status_str.len())).unwrap();
        writeln!(&mut output, "└─────────────────────────────────┘").unwrap();
        
        // Try to format the output in a human-friendly way based on tool ID
        writeln!(&mut output, "").unwrap();
        match result.tool_id.as_str() {
            "shell" => Self::format_shell_output(&mut output, &result.output),
            "read_file" => Self::format_file_output(&mut output, &result.output),
            "write_file" => Self::format_write_output(&mut output, &result.output),
            "list_directory" => Self::format_directory_output(&mut output, &result.output),
            _ => Self::format_generic_output(&mut output, &result.output),
        }
        
        // Add error information if present
        if let Some(err) = &result.error {
            writeln!(&mut output, "\nError: {}", err).unwrap();
        }
        
        output
    }
    
    /// Format shell command output
    fn format_shell_output(output: &mut String, result: &Value) -> () {
        // Extract details from the shell result
        let command = result.get("command").and_then(Value::as_str).unwrap_or("unknown");
        let exit_code = result.get("exit_code").and_then(Value::as_i64).unwrap_or(-1);
        let stdout = result.get("stdout").and_then(Value::as_str).unwrap_or("");
        let stderr = result.get("stderr").and_then(Value::as_str).unwrap_or("");
        
        // Command and exit status
        writeln!(output, "Command: {}", command).unwrap();
        writeln!(output, "Exit Code: {}", exit_code).unwrap();
        
        // Show stdout if present
        if !stdout.is_empty() {
            writeln!(output, "\nOutput:").unwrap();
            writeln!(output, "{}", &Self::format_command_output(stdout)).unwrap();
        }
        
        // Show stderr if present
        if !stderr.is_empty() {
            writeln!(output, "\nErrors:").unwrap();
            writeln!(output, "{}", &Self::format_command_output(stderr)).unwrap();
        }
    }
    
    /// Format file read output
    fn format_file_output(output: &mut String, result: &Value) -> () {
        // Extract details from the file result
        let path = result.get("path").and_then(Value::as_str).unwrap_or("unknown");
        let content = result.get("content").and_then(Value::as_str).unwrap_or("");
        let size = result.get("size").and_then(Value::as_i64).unwrap_or(-1);
        
        writeln!(output, "File: {}", path).unwrap();
        writeln!(output, "Size: {} bytes", size).unwrap();
        
        if !content.is_empty() {
            let preview_line_count = content.lines().take(20).count();
            let total_line_count = content.lines().count();
            
            writeln!(output, "\nContent:").unwrap();
            writeln!(output, "{}", &Self::format_file_content(content, 20)).unwrap();
            
            if preview_line_count < total_line_count {
                writeln!(output, "\n... [+{} more lines] ...", total_line_count - preview_line_count).unwrap();
            }
        }
    }
    
    /// Format file write output
    fn format_write_output(output: &mut String, result: &Value) -> () {
        // Extract details from the write result
        let path = result.get("path").and_then(Value::as_str).unwrap_or("unknown");
        let bytes_written = result.get("bytes_written").and_then(Value::as_i64).unwrap_or(-1);
        let success = result.get("success").and_then(Value::as_bool).unwrap_or(false);
        
        writeln!(output, "File: {}", path).unwrap();
        writeln!(output, "Status: {}", if success { "Written successfully" } else { "Write failed" }).unwrap();
        writeln!(output, "Bytes Written: {}", bytes_written).unwrap();
    }
    
    /// Format directory listing output
    fn format_directory_output(output: &mut String, result: &Value) -> () {
        // Extract details from the directory listing
        let path = result.get("path").and_then(Value::as_str).unwrap_or("unknown");
        let entries = result.get("entries").and_then(Value::as_array);
        
        writeln!(output, "Directory: {}", path).unwrap();
        
        if let Some(entries) = entries {
            writeln!(output, "\nContents:").unwrap();
            writeln!(output, "{:<30} {:<15} {:<10}", "Name", "Type", "Size").unwrap();
            writeln!(output, "{}", "─".repeat(55)).unwrap();
            
            for entry in entries {
                let name = entry.get("name").and_then(Value::as_str).unwrap_or("unknown");
                let entry_type = entry.get("type").and_then(Value::as_str).unwrap_or("unknown");
                let size = entry.get("size").and_then(Value::as_i64).unwrap_or(-1);
                
                let size_str = if entry_type == "directory" {
                    "-".to_string()
                } else {
                    Self::format_size(size)
                };
                
                let display_name = if name.len() > 28 {
                    format!("{}...", &name[0..25])
                } else {
                    name.to_string()
                };
                
                writeln!(output, "{:<30} {:<15} {:<10}", display_name, entry_type, size_str).unwrap();
            }
        } else {
            writeln!(output, "\nNo entries found").unwrap();
        }
    }
    
    /// Format generic tool output
    fn format_generic_output(output: &mut String, result: &Value) -> () {
        // For tools we don't have specific formatters for, pretty-print the JSON
        match serde_json::to_string_pretty(result) {
            Ok(pretty_json) => {
                writeln!(output, "Result:").unwrap();
                writeln!(output, "{}", pretty_json).unwrap();
            }
            Err(_) => {
                writeln!(output, "Result: {}", result).unwrap();
            }
        }
    }
    
    /// Format command output with line numbers
    fn format_command_output(content: &str) -> String {
        let max_lines = 30; // Maximum number of lines to show
        let lines: Vec<&str> = content.lines().collect();
        
        let mut output = String::new();
        
        // Get the total number of digits needed for line numbers
        let line_count = lines.len();
        let line_num_width = if line_count > 0 {
            (line_count as f64).log10().floor() as usize + 1
        } else {
            1
        };
        
        // Add lines with numbers
        for (i, line) in lines.iter().take(max_lines).enumerate() {
            writeln!(&mut output, "{:>width$} │ {}", i + 1, line, width = line_num_width).unwrap();
        }
        
        // Add truncation indicator if needed
        if lines.len() > max_lines {
            writeln!(&mut output, "... [{} more lines]", lines.len() - max_lines).unwrap();
        }
        
        output
    }
    
    /// Format file content with line numbers
    fn format_file_content(content: &str, max_lines: usize) -> String {
        let lines: Vec<&str> = content.lines().collect();
        
        let mut output = String::new();
        
        // Get the total number of digits needed for line numbers
        let line_count = lines.len();
        let line_num_width = if line_count > 0 {
            (line_count as f64).log10().floor() as usize + 1
        } else {
            1
        };
        
        // Add lines with numbers
        for (i, line) in lines.iter().take(max_lines).enumerate() {
            writeln!(&mut output, "{:>width$} │ {}", i + 1, line, width = line_num_width).unwrap();
        }
        
        output
    }
    
    /// Format a size in bytes to a human-readable string
    fn format_size(size: i64) -> String {
        if size < 1024 {
            format!("{} B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
    
    /// Extract and format tool output from a JSON-RPC result
    pub fn extract_from_jsonrpc(json_str: &str) -> Option<String> {
        match serde_json::from_str::<Value>(json_str) {
            Ok(json) => {
                if let Some(result) = json.get("result") {
                    // Try to parse as a tool result
                    let tool_id = result.get("tool_id").and_then(Value::as_str).unwrap_or("unknown");
                    let status = match result.get("status").and_then(Value::as_str) {
                        Some("success") => ToolStatus::Success,
                        Some("failure") => ToolStatus::Failure,
                        Some("timeout") => ToolStatus::Timeout,
                        _ => ToolStatus::Failure,
                    };
                    
                    let output = result.get("output").cloned().unwrap_or(json!({}));
                    let error = result.get("error").and_then(Value::as_str).map(String::from);
                    
                    let tool_result = ToolResult {
                        tool_id: tool_id.to_string(),
                        status,
                        output,
                        error,
                    };
                    
                    Some(Self::format_tool_result(&tool_result))
                } else if let Some(error) = json.get("error") {
                    // It's an error response
                    let error_message = error.get("message").and_then(Value::as_str).unwrap_or("Unknown error");
                    Some(format!("Error: {}", error_message))
                } else {
                    debug!("Unable to parse JSON-RPC result: {}", json_str);
                    None
                }
            }
            Err(e) => {
                warn!("Failed to parse JSON-RPC response: {}", e);
                None
            }
        }
    }
}

/// Format LLM responses to enhance readability
pub fn format_llm_response(content: &str) -> String {
    // For now, simply pass through the response content
    // In the future, we could add formatting for markdown, code blocks, etc.
    content.to_string()
}