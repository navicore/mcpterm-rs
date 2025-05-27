use crate::json_filter::JsonRpcFilter;
use mcp_tools::{ToolResult, ToolStatus};
use serde_json::{json, Value};
use std::fmt::Write;
use tracing::{debug, warn};

/// ANSI color codes for terminal output
struct Colors;

impl Colors {
    // Color support can be disabled with NO_COLOR=1 environment variable
    fn supports_color() -> bool {
        std::env::var("NO_COLOR").is_err() && atty::is(atty::Stream::Stdout)
    }

    fn reset() -> &'static str {
        if Self::supports_color() {
            "\x1b[0m"
        } else {
            ""
        }
    }

    fn bold() -> &'static str {
        if Self::supports_color() {
            "\x1b[1m"
        } else {
            ""
        }
    }

    fn dim() -> &'static str {
        if Self::supports_color() {
            "\x1b[2m"
        } else {
            ""
        }
    }

    fn underline() -> &'static str {
        if Self::supports_color() {
            "\x1b[4m"
        } else {
            ""
        }
    }

    fn red() -> &'static str {
        if Self::supports_color() {
            "\x1b[31m"
        } else {
            ""
        }
    }

    fn green() -> &'static str {
        if Self::supports_color() {
            "\x1b[32m"
        } else {
            ""
        }
    }

    fn yellow() -> &'static str {
        if Self::supports_color() {
            "\x1b[33m"
        } else {
            ""
        }
    }

    fn blue() -> &'static str {
        if Self::supports_color() {
            "\x1b[34m"
        } else {
            ""
        }
    }

    fn magenta() -> &'static str {
        if Self::supports_color() {
            "\x1b[35m"
        } else {
            ""
        }
    }

    fn cyan() -> &'static str {
        if Self::supports_color() {
            "\x1b[36m"
        } else {
            ""
        }
    }

    fn white() -> &'static str {
        if Self::supports_color() {
            "\x1b[37m"
        } else {
            ""
        }
    }
}

/// Format tool results in a human-friendly way
#[derive(Default)]
pub struct ResponseFormatter {
    /// JSON-RPC filter for removing tool calls from user-facing output
    json_filter: JsonRpcFilter,
}

impl ResponseFormatter {
    /// Format a message for display, removing any JSON-RPC tool calls
    pub fn format_message(&self, message: &str) -> String {
        // Filter out any JSON-RPC tool calls, especially focusing on patch tool calls
        self.json_filter.filter_json_rpc(message)
    }

    /// Format a tool result for user-friendly display
    pub fn format_tool_result(result: &ToolResult) -> String {
        let mut output = String::new();

        // Format status header with appropriate styling and colors
        let (status_str, status_color) = match result.status {
            ToolStatus::Success => ("SUCCESS", Colors::green()),
            ToolStatus::Failure => ("FAILURE", Colors::red()),
            ToolStatus::Timeout => ("TIMEOUT", Colors::yellow()),
        };

        // Add the status header with border and color
        writeln!(
            &mut output,
            "{}┌─────────────────────────────────┐{}",
            Colors::cyan(),
            Colors::reset()
        )
        .unwrap();
        writeln!(
            &mut output,
            "{}│{} Tool Result: {}{}{}{} {}│{}",
            Colors::cyan(),
            Colors::reset(),
            Colors::bold(),
            status_color,
            status_str,
            Colors::reset(),
            " ".repeat(21 - status_str.len()),
            Colors::cyan()
        )
        .unwrap();
        writeln!(
            &mut output,
            "{}└─────────────────────────────────┘{}",
            Colors::cyan(),
            Colors::reset()
        )
        .unwrap();

        // Try to format the output in a human-friendly way based on tool ID
        writeln!(&mut output).unwrap();
        match result.tool_id.as_str() {
            "shell" => Self::format_shell_output(&mut output, &result.output),
            "read_file" => Self::format_file_output(&mut output, &result.output),
            "write_file" => Self::format_write_output(&mut output, &result.output),
            "list_directory" => Self::format_directory_output(&mut output, &result.output),
            _ => Self::format_generic_output(&mut output, &result.output),
        }

        // Add error information if present
        if let Some(err) = &result.error {
            writeln!(
                &mut output,
                "\n{}{}Error:{} {}",
                Colors::bold(),
                Colors::red(),
                Colors::reset(),
                err
            )
            .unwrap();
        }

        output
    }

    /// Format shell command output
    fn format_shell_output(output: &mut String, result: &Value) {
        // Extract details from the shell result
        let command = result
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let exit_code = result
            .get("exit_code")
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        let stdout = result.get("stdout").and_then(Value::as_str).unwrap_or("");
        let stderr = result.get("stderr").and_then(Value::as_str).unwrap_or("");

        // Command and exit status with color
        writeln!(
            output,
            "{}{}Command:{} {}",
            Colors::bold(),
            Colors::blue(),
            Colors::reset(),
            command
        )
        .unwrap();

        // Color exit code based on success/failure
        let exit_code_color = if exit_code == 0 {
            Colors::green()
        } else {
            Colors::red()
        };
        writeln!(
            output,
            "{}{}Exit Code:{} {}{}{}",
            Colors::bold(),
            Colors::blue(),
            Colors::reset(),
            exit_code_color,
            exit_code,
            Colors::reset()
        )
        .unwrap();

        // Show stdout if present
        if !stdout.is_empty() {
            writeln!(
                output,
                "\n{}{}Output:{}",
                Colors::bold(),
                Colors::green(),
                Colors::reset()
            )
            .unwrap();
            writeln!(output, "{}", &Self::format_command_output(stdout)).unwrap();
        }

        // Show stderr if present
        if !stderr.is_empty() {
            writeln!(
                output,
                "\n{}{}Errors:{}",
                Colors::bold(),
                Colors::red(),
                Colors::reset()
            )
            .unwrap();
            writeln!(output, "{}", &Self::format_command_output(stderr)).unwrap();
        }
    }

    /// Format file read output
    fn format_file_output(output: &mut String, result: &Value) {
        // Extract details from the file result
        let path = result
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let content = result.get("content").and_then(Value::as_str).unwrap_or("");
        let size = result.get("size").and_then(Value::as_i64).unwrap_or(-1);

        writeln!(
            output,
            "{}{}File:{} {}",
            Colors::bold(),
            Colors::blue(),
            Colors::reset(),
            path
        )
        .unwrap();
        writeln!(
            output,
            "{}{}Size:{} {} bytes",
            Colors::bold(),
            Colors::blue(),
            Colors::reset(),
            size
        )
        .unwrap();

        if !content.is_empty() {
            let preview_line_count = content.lines().take(20).count();
            let total_line_count = content.lines().count();

            writeln!(
                output,
                "\n{}{}Content:{}",
                Colors::bold(),
                Colors::green(),
                Colors::reset()
            )
            .unwrap();
            writeln!(output, "{}", &Self::format_file_content(content, 20)).unwrap();

            if preview_line_count < total_line_count {
                writeln!(
                    output,
                    "\n{}... [+{} more lines] ...{}",
                    Colors::dim(),
                    total_line_count - preview_line_count,
                    Colors::reset()
                )
                .unwrap();
            }
        }
    }

    /// Format file write output
    fn format_write_output(output: &mut String, result: &Value) {
        // Extract details from the write result
        let path = result
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let bytes_written = result
            .get("bytes_written")
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        let success = result
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        writeln!(
            output,
            "{}{}File:{} {}",
            Colors::bold(),
            Colors::blue(),
            Colors::reset(),
            path
        )
        .unwrap();

        // Color status based on success/failure
        let status_color = if success {
            Colors::green()
        } else {
            Colors::red()
        };
        let status_text = if success {
            "Written successfully"
        } else {
            "Write failed"
        };
        writeln!(
            output,
            "{}{}Status:{} {}{}{}",
            Colors::bold(),
            Colors::blue(),
            Colors::reset(),
            status_color,
            status_text,
            Colors::reset()
        )
        .unwrap();

        writeln!(
            output,
            "{}{}Bytes Written:{} {}",
            Colors::bold(),
            Colors::blue(),
            Colors::reset(),
            bytes_written
        )
        .unwrap();
    }

    /// Format directory listing output
    fn format_directory_output(output: &mut String, result: &Value) {
        // Extract details from the directory listing
        let path = result
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let entries = result.get("entries").and_then(Value::as_array);

        writeln!(
            output,
            "{}{}Directory:{} {}",
            Colors::bold(),
            Colors::blue(),
            Colors::reset(),
            path
        )
        .unwrap();

        if let Some(entries) = entries {
            writeln!(
                output,
                "\n{}{}Contents:{}",
                Colors::bold(),
                Colors::green(),
                Colors::reset()
            )
            .unwrap();

            // Table header with colors
            writeln!(
                output,
                "{}{}{:<30} {:<15} {:<10}{}",
                Colors::bold(),
                Colors::underline(),
                "Name",
                "Type",
                "Size",
                Colors::reset()
            )
            .unwrap();

            for entry in entries {
                let name = entry
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let entry_type = entry
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
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

                // Color-code directories and files differently
                let name_color = match entry_type {
                    "directory" => Colors::blue(),
                    "file" => Colors::white(),
                    _ => Colors::dim(),
                };

                writeln!(
                    output,
                    "{}{:<30}{} {}{:<15}{} {:<10}",
                    name_color,
                    display_name,
                    Colors::reset(),
                    Colors::dim(),
                    entry_type,
                    Colors::reset(),
                    size_str
                )
                .unwrap();
            }
        } else {
            writeln!(
                output,
                "\n{}No entries found{}",
                Colors::dim(),
                Colors::reset()
            )
            .unwrap();
        }
    }

    /// Format generic tool output
    fn format_generic_output(output: &mut String, result: &Value) {
        // For tools we don't have specific formatters for, pretty-print the JSON with syntax highlighting
        match serde_json::to_string_pretty(result) {
            Ok(pretty_json) => {
                writeln!(
                    output,
                    "{}{}Result:{}",
                    Colors::bold(),
                    Colors::blue(),
                    Colors::reset()
                )
                .unwrap();
                // Add basic JSON syntax highlighting
                let highlighted = Self::highlight_json(&pretty_json);
                writeln!(output, "{}", highlighted).unwrap();
            }
            Err(_) => {
                writeln!(
                    output,
                    "{}{}Result:{} {}",
                    Colors::bold(),
                    Colors::blue(),
                    Colors::reset(),
                    result
                )
                .unwrap();
            }
        }
    }

    /// Apply simple syntax highlighting to JSON
    fn highlight_json(json: &str) -> String {
        let mut result = String::with_capacity(json.len() * 2); // Allocate extra space for color codes

        // If color is not supported, return the plain JSON
        if !Colors::supports_color() {
            return json.to_string();
        }

        let mut in_string = false;
        let mut escaped = false;

        for c in json.chars() {
            match c {
                '"' if !escaped => {
                    in_string = !in_string;
                    if in_string {
                        result.push_str(Colors::green()); // Start green for strings
                    } else {
                        result.push_str(Colors::reset()); // End green for strings
                    }
                    result.push(c);
                }
                '\\' if in_string => {
                    escaped = !escaped;
                    result.push(c);
                }
                '{' | '[' | '}' | ']' if !in_string => {
                    result.push_str(Colors::cyan()); // Brackets in cyan
                    result.push(c);
                    result.push_str(Colors::reset());
                }
                ':' if !in_string => {
                    result.push_str(Colors::reset());
                    result.push(c);
                    result.push(' '); // Add a space after colons for readability
                }
                ',' if !in_string => {
                    result.push_str(Colors::reset());
                    result.push(c);
                }
                't' | 'f' | 'n'
                    if !in_string
                        && (json[json.find(c).unwrap()..].starts_with("true")
                            || json[json.find(c).unwrap()..].starts_with("false")
                            || json[json.find(c).unwrap()..].starts_with("null")) =>
                {
                    result.push_str(Colors::magenta()); // Keywords in magenta
                    result.push(c);
                    result.push_str(Colors::reset());
                }
                '0'..='9' if !in_string => {
                    result.push_str(Colors::yellow()); // Numbers in yellow
                    result.push(c);
                    result.push_str(Colors::reset());
                }
                _ => {
                    if !in_string && c.is_ascii_digit() {
                        result.push_str(Colors::yellow()); // Numbers in yellow
                        result.push(c);
                        result.push_str(Colors::reset());
                    } else {
                        if in_string && escaped {
                            escaped = false;
                        }
                        result.push(c);
                    }
                }
            }
        }

        result
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

        // Add lines with colored numbers
        for (i, line) in lines.iter().take(max_lines).enumerate() {
            writeln!(
                &mut output,
                "{}{:>width$}{} │ {}",
                Colors::dim(),
                i + 1,
                Colors::reset(),
                line,
                width = line_num_width
            )
            .unwrap();
        }

        // Add truncation indicator if needed
        if lines.len() > max_lines {
            writeln!(
                &mut output,
                "{}... [{} more lines] ...{}",
                Colors::dim(),
                lines.len() - max_lines,
                Colors::reset()
            )
            .unwrap();
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

        // Add lines with colored numbers
        for (i, line) in lines.iter().take(max_lines).enumerate() {
            writeln!(
                &mut output,
                "{}{:>width$}{} │ {}",
                Colors::dim(),
                i + 1,
                Colors::reset(),
                line,
                width = line_num_width
            )
            .unwrap();
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
                    let tool_id = result
                        .get("tool_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let status = match result.get("status").and_then(Value::as_str) {
                        Some("success") => ToolStatus::Success,
                        Some("failure") => ToolStatus::Failure,
                        Some("timeout") => ToolStatus::Timeout,
                        _ => ToolStatus::Failure,
                    };

                    let output = result.get("output").cloned().unwrap_or(json!({}));
                    let error = result
                        .get("error")
                        .and_then(Value::as_str)
                        .map(String::from);

                    let tool_result = ToolResult {
                        tool_id: tool_id.to_string(),
                        status,
                        output,
                        error,
                    };

                    Some(Self::format_tool_result(&tool_result))
                } else if let Some(error) = json.get("error") {
                    // It's an error response
                    let error_message = error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Unknown error");
                    Some(format!(
                        "{}{}Error:{} {}",
                        Colors::bold(),
                        Colors::red(),
                        Colors::reset(),
                        error_message
                    ))
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
/// Our architecture has several formats:
/// 1. LlmResponse with a "content" field from mcp-llm
/// 2. MCP JSON-RPC Response with a "result" field in the protocol
/// 3. Claude API response with content[].text containing JSON-RPC response
///    This formatter handles all cases
pub fn format_llm_response(content: &str) -> String {
    // First, check if this is a tool call JSON-RPC - if so, skip it entirely
    if content.contains("\"jsonrpc\"")
        && content.contains("\"method\"")
        && (content.contains("\"mcp.tool_call\"") || content.contains("\"params\""))
    {
        debug!("Detected tool call JSON-RPC, skipping format: {}", content);
        return String::new(); // Return empty string to avoid printing the tool call
    }

    // For valid JSON, extract content according to our schema
    if content.trim().starts_with('{') && content.trim().ends_with('}') {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) {
            // CASE 1: Claude API direct response structure (contains content array with text field)
            // Format: {"content":[{"type":"text","text":"JSON-RPC response as string"}],...}
            if let Some(content_array) = parsed.get("content").and_then(|v| v.as_array()) {
                if !content_array.is_empty() {
                    if let Some(text) = content_array[0].get("text").and_then(|v| v.as_str()) {
                        // Skip if this is a tool call
                        if text.contains("\"jsonrpc\"")
                            && text.contains("\"method\"")
                            && (text.contains("\"mcp.tool_call\"") || text.contains("\"params\""))
                        {
                            debug!("Detected tool call in content text, skipping format");
                            return String::new();
                        }

                        // This is the Claude Bedrock API format - the text field contains a JSON-RPC response
                        if text.trim().starts_with('{') && text.trim().ends_with('}') {
                            if let Ok(inner_json) = serde_json::from_str::<serde_json::Value>(text)
                            {
                                // Skip if this is a tool call
                                if inner_json.get("method").is_some() {
                                    debug!(
                                        "Detected method field in inner JSON, likely a tool call"
                                    );
                                    return String::new();
                                }

                                // If it's a JSON-RPC response, extract the result field
                                if inner_json.get("jsonrpc").is_some() {
                                    if let Some(result) = inner_json.get("result") {
                                        if let Some(result_str) = result.as_str() {
                                            return result_str.to_string();
                                        }
                                    }
                                }
                            }
                        }
                        // If not a recognized format, return the text directly
                        return text.to_string();
                    }
                }
            }

            // CASE 2: LlmResponse format (with a string "content" field)
            // Format: {"content":"text or JSON string",...}
            if let Some(text) = parsed.get("content").and_then(|v| v.as_str()) {
                // Skip if this is a tool call
                if text.contains("\"jsonrpc\"")
                    && text.contains("\"method\"")
                    && (text.contains("\"mcp.tool_call\"") || text.contains("\"params\""))
                {
                    debug!("Detected tool call in content field, skipping format");
                    return String::new();
                }

                // Check if this content is actually JSON itself (common in LLM responses)
                if text.trim().starts_with('{') && text.trim().ends_with('}') {
                    if let Ok(nested_json) = serde_json::from_str::<serde_json::Value>(text) {
                        // Skip if this is a tool call
                        if nested_json.get("method").is_some() {
                            debug!("Detected method field in nested JSON, likely a tool call");
                            return String::new();
                        }

                        // If it's a JSON-RPC response, extract the result field
                        if nested_json.get("jsonrpc").is_some() {
                            if let Some(result) = nested_json.get("result") {
                                if let Some(result_str) = result.as_str() {
                                    return result_str.to_string();
                                }
                            }
                        }
                    }
                }
                return text.to_string();
            }

            // Skip if this is a tool call
            if parsed.get("method").is_some() {
                debug!("Detected method field in JSON, likely a tool call");
                return String::new();
            }

            // CASE 3: JSON-RPC Response format (with "result" field)
            // Format: {"jsonrpc":"2.0","result":"text or object",...}
            if let Some(result) = parsed.get("result") {
                // If result is a string, return it directly
                if let Some(text) = result.as_str() {
                    return text.to_string();
                }

                // If result is an object with content field, extract that
                if let Some(content) = result.get("content") {
                    if let Some(text) = content.as_str() {
                        return text.to_string();
                    }
                }

                // Handle array of responses (common in some tool results)
                if let Some(array) = result.as_array() {
                    if !array.is_empty() {
                        let mut combined = String::new();
                        for (i, item) in array.iter().enumerate() {
                            if i > 0 {
                                combined.push_str("\n\n");
                            }

                            if let Some(text) = item.as_str() {
                                combined.push_str(text);
                            } else if item.is_object() {
                                // Try to get content field or stringify the object
                                if let Some(text) = item.get("content").and_then(|v| v.as_str()) {
                                    combined.push_str(text);
                                } else if let Ok(pretty) = serde_json::to_string_pretty(item) {
                                    combined.push_str(&pretty);
                                }
                            }
                        }
                        if !combined.is_empty() {
                            return combined;
                        }
                    }
                }

                // If we got here, try to extract just the stringified value without the object wrapper
                if let Ok(result_pretty) = serde_json::to_string_pretty(result) {
                    // The special case that's causing our problems
                    if result_pretty.contains("Here are") || result_pretty.contains("\\n") {
                        // This looks like an escaped string we should unescape
                        let result_str = result_pretty.trim_matches('"');

                        // Unescape the JSON string (replace \\n with \n, etc.)
                        let unescaped = result_str
                            .replace("\\n", "\n")
                            .replace("\\\"", "\"")
                            .replace("\\\\", "\\");
                        return unescaped;
                    }

                    // Otherwise, return the pretty-printed but stringified result
                    return result_pretty;
                }

                // Last resort fallback - just return the original result
                return result.to_string();
            }
        }

        // If we reached here, it's invalid JSON structure
        debug!("Received JSON response in unexpected format: {}", content);
    }

    // Return the content directly if it's not JSON or we couldn't parse it
    content.to_string()
}
