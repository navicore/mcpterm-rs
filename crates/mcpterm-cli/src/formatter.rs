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
pub struct ResponseFormatter {
    /// JSON-RPC filter for removing tool calls from user-facing output
    json_filter: JsonRpcFilter,
}

impl Default for ResponseFormatter {
    fn default() -> Self {
        Self {
            json_filter: JsonRpcFilter::new(),
        }
    }
}

/// Extract non-JSON content from a message
/// This function removes anything that looks like JSON or a code block
fn extract_non_json_content(content: &str) -> String {
    // Fast path for empty content
    if content.trim().is_empty() {
        return String::new();
    }

    // If the entire content is valid JSON, return empty string
    if content.trim().starts_with('{') && content.trim().ends_with('}') {
        if serde_json::from_str::<serde_json::Value>(content.trim()).is_ok() {
            debug!("Entire content is valid JSON, returning empty string");
            return String::new();
        }
    }

    let mut result = String::new();
    let mut text_buffer = String::new();
    let mut in_json = false;
    let mut in_code_block = false;
    let mut brace_count = 0;
    let mut bracket_count = 0;

    // Extra indicators for JSON-like content
    let has_jsonrpc = content.contains("\"jsonrpc\"");
    let has_method = content.contains("\"method\"");
    let has_params = content.contains("\"params\"");
    let has_tool_call = content.contains("\"mcp.tool_call\"");

    // Split the content into paragraphs for better handling
    let paragraphs: Vec<&str> = content.split("\n\n").collect();

    for para in paragraphs {
        let trimmed = para.trim();

        // Skip empty paragraphs
        if trimmed.is_empty() {
            continue;
        }

        // Skip paragraphs that look like JSON (matching braces/brackets)
        if (trimmed.starts_with('{') && trimmed.ends_with('}')) ||
           (trimmed.starts_with('[') && trimmed.ends_with(']')) {
            // Check if it's parsable JSON
            if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                debug!("Skipping JSON paragraph: {}", if trimmed.len() > 30 { &trimmed[0..30] } else { trimmed });
                continue;
            }
        }

        // Skip paragraphs with JSON-RPC markers
        if has_jsonrpc && has_method && has_params &&
           (trimmed.contains("\"jsonrpc\"") || trimmed.contains("\"method\"") ||
            trimmed.contains("\"params\"") || trimmed.contains("\"mcp.tool_call\"")) {
            debug!("Skipping paragraph with JSON-RPC markers: {}", if trimmed.len() > 30 { &trimmed[0..30] } else { trimmed });
            continue;
        }

        // Process the paragraph line by line
        let mut para_has_json = false;
        text_buffer.clear();

        for line in para.lines() {
            let line_trimmed = line.trim();

            // Check for code block markers
            if line_trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }

            // Skip code block content
            if in_code_block {
                continue;
            }

            // Calculate brace/bracket balance for this line
            for c in line_trimmed.chars() {
                match c {
                    '{' => { brace_count += 1; if brace_count == 1 { in_json = true; } },
                    '}' => { brace_count = (brace_count - 1).max(0); if brace_count == 0 { in_json = false; } },
                    '[' => { bracket_count += 1; if bracket_count == 1 { in_json = true; } },
                    ']' => { bracket_count = (bracket_count - 1).max(0); if bracket_count == 0 { in_json = false; } },
                    _ => {}
                }
            }

            // Skip lines that look like JSON fragments
            if in_json ||
               line_trimmed.starts_with('{') || line_trimmed.starts_with('[') ||
               line_trimmed.ends_with('}') || line_trimmed.ends_with(']') ||
               line_trimmed.contains("\"jsonrpc\"") ||
               line_trimmed.contains("\"id\":") ||
               line_trimmed.contains("\"method\":") ||
               line_trimmed.contains("\"params\":") ||
               line_trimmed.contains("\"status\":") ||
               line_trimmed.contains("\"tool_id\":") ||
               line_trimmed.contains("\"error\":") ||
               (line_trimmed.contains(':') && line_trimmed.contains('\"')) {
                para_has_json = true;
                continue;
            }

            // If the line is not json-like and not empty, add it to the buffer
            if !line_trimmed.is_empty() {
                text_buffer.push_str(line);
                text_buffer.push('\n');
            }
        }

        // If the paragraph had no JSON-like content, or if we extracted some text despite JSON markers
        if !para_has_json || !text_buffer.trim().is_empty() {
            result.push_str(&text_buffer);
            result.push('\n');
        }
    }

    // Clean up the result
    let cleaned = result.trim();

    // Handle cases where there's no natural language content extracted
    if cleaned.is_empty() && has_tool_call {
        return String::new(); // This was likely just a tool call with no natural language
    }

    cleaned.to_string()
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
        // For skipped duplicate tool executions, show a friendly message
        if let Some(skipped) = result.get("skipped").and_then(Value::as_bool) {
            if skipped {
                if let Some(msg) = result.get("message").and_then(Value::as_str) {
                    writeln!(
                        output,
                        "{}{}Info:{} {}",
                        Colors::bold(),
                        Colors::blue(),
                        Colors::reset(),
                        msg
                    )
                    .unwrap();
                    return;
                }
            }
        }

        // Check if the result is empty or invalid
        if !result.is_object() || result.as_object().unwrap().is_empty() {
            writeln!(
                output,
                "{}{}Note:{} No command details available",
                Colors::bold(),
                Colors::yellow(),
                Colors::reset()
            )
            .unwrap();
            return;
        }

        // Extract details from the shell result
        let command = result
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        // Skip displaying entirely if command is unknown - don't even show a note
        if command == "unknown" {
            return;
        }

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

/// Format LLM responses using a simple rule: remove all JSON, show only non-JSON text
pub fn format_llm_response(content: &str) -> String {
    // 1. Tool result messages are handled by event adapter
    if content.starts_with("Tool '") && content.contains("returned result:") {
        debug!("Detected tool result message, this needs to be handled by the event adapter");
        return String::new(); // Return empty string because event_adapter will handle this directly
    }

    // 2. Check if this is structured JSON that we need to extract content from
    if content.trim().starts_with('{') && content.trim().ends_with('}') {
        // Try to parse the JSON to handle special cases
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) {
            // Handle Claude API direct response ({"content":[{"type":"text","text":"..."}]})
            if let Some(content_array) = parsed.get("content").and_then(|v| v.as_array()) {
                if !content_array.is_empty() {
                    if let Some(text) = content_array[0].get("text").and_then(|v| v.as_str()) {
                        // Check if text is a JSON string that needs to be parsed (common in Bedrock response)
                        if text.trim().starts_with('{') && text.trim().ends_with('}') {
                            debug!("Found JSON in content[0].text field, attempting to parse");
                            if let Ok(inner_json) = serde_json::from_str::<serde_json::Value>(text) {
                                // If it's a JSON-RPC message with a result, extract the result
                                if inner_json.get("jsonrpc").is_some() {
                                    if let Some(result) = inner_json.get("result") {
                                        if let Some(result_text) = result.as_str() {
                                            return extract_non_json_content(result_text);
                                        }
                                    }
                                }
                            }
                        }

                        // If not JSON or couldn't parse, apply our non-JSON extraction rule to the inner text
                        return extract_non_json_content(text);
                    }
                }
            }
            
            // Handle simple content field ({"content":"..."})
            if let Some(text) = parsed.get("content").and_then(|v| v.as_str()) {
                // Check if content is a JSON string that needs to be parsed
                if text.trim().starts_with('{') && text.trim().ends_with('}') {
                    // Try to parse the embedded JSON
                    if let Ok(inner_json) = serde_json::from_str::<serde_json::Value>(text) {
                        // If it has jsonrpc field, it's likely a JSON-RPC response
                        if inner_json.get("jsonrpc").is_some() {
                            // Extract the result field
                            if let Some(inner_result) = inner_json.get("result") {
                                if let Some(inner_text) = inner_result.as_str() {
                                    return extract_non_json_content(inner_text);
                                }
                            }
                        }
                    }
                }

                // If not JSON or couldn't parse, just extract non-JSON
                return extract_non_json_content(text);
            }
            
            // Handle JSON-RPC result ({"jsonrpc":"2.0","result":"..."})
            if let Some(result) = parsed.get("result") {
                if let Some(text) = result.as_str() {
                    // Result is a string - apply non-JSON extraction
                    return extract_non_json_content(text);
                } else if result.is_object() {
                    // Result is an object - check for common fields
                    if let Some(content) = result.get("content").and_then(|v| v.as_str()) {
                        return extract_non_json_content(content);
                    } else if let Some(id) = result.get("id").and_then(|v| v.as_str()) {
                        // This might be a response object
                        debug!("Found ID in result object: {}", id);

                        // Best effort - stringify the entire result object
                        let result_str = serde_json::to_string_pretty(result).unwrap_or_default();
                        return extract_non_json_content(&result_str);
                    }
                }

                // If we reach here, just stringify the whole result and extract non-JSON
                let result_str = serde_json::to_string_pretty(result).unwrap_or_default();
                let extracted = extract_non_json_content(&result_str);
                if !extracted.is_empty() {
                    return extracted;
                }

                // Last resort - convert escapes and try again
                if let Ok(json_str) = serde_json::to_string(result) {
                    let unescaped = json_str
                        .trim_matches('"')
                        .replace("\\n", "\n")
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\");
                    return extract_non_json_content(&unescaped);
                }
            }
        }
    }

    // 3. For any other content, apply our core extraction rule
    debug!("Extracting non-JSON content from message");
    let non_json = extract_non_json_content(content);
    
    // If we found non-JSON content, return it
    if !non_json.is_empty() {
        debug!("Showing non-JSON content to user: {}", non_json);
        return non_json;
    }

    // If there's nothing left after extracting non-JSON content, check for informational phrases
    // that might indicate there was some useful information in the original content
    let trimmed = content.trim();
    if trimmed.contains("I'll") || trimmed.contains("let me") || trimmed.contains("Let me") ||
       trimmed.contains("help") || trimmed.contains("create") || 
       trimmed.contains("I need") || trimmed.contains("I will") ||
       trimmed.contains("First") || trimmed.contains("Now") ||
       trimmed.contains("check") || trimmed.contains("make") ||
       trimmed.contains("build") || trimmed.contains("implement") {
        // Try to extract just the first line, which might be helpful
        if let Some(line) = trimmed.lines().next() {
            return line.to_string();
        }
    }

    // Nothing useful found
    debug!("No non-JSON content found to display");
    String::new()
}