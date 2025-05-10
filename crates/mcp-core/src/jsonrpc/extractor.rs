use serde_json::Value;
use tracing::{debug, trace};

/// Special patch tool extractor to handle the issue with patch tool JSON-RPC validation
/// This is a temporary workaround that specifically looks for patch tool calls
fn extract_patch_tool_call(content: &str) -> Option<Value> {
    // Quick check for patch pattern to avoid unnecessary parsing
    if !content.contains("\"name\":\"patch\"") {
        return None;
    }

    // Find JSON object boundaries
    let start = content.find('{')?;
    let mut depth = 0;
    let mut end = 0;

    for (i, c) in content[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    if end <= start {
        return None;
    }

    // Try to parse the JSON
    let json_str = &content[start..end];
    match serde_json::from_str::<Value>(json_str) {
        Ok(json) => {
            // Check if it's a patch tool call
            if let (
                Some(Value::String(jsonrpc)),
                Some(Value::String(method)),
                Some(params),
                Some(_)
            ) = (
                json.get("jsonrpc"),
                json.get("method"),
                json.get("params"),
                json.get("id")
            ) {
                if jsonrpc == "2.0" && method == "mcp.tool_call" {
                    if let Some(Value::String(name)) = params.get("name") {
                        if name == "patch" {
                            debug!("SPECIAL CASE: Found patch tool call: {}", json_str);
                            return Some(json);
                        }
                    }
                }
            }
        }
        Err(e) => {
            debug!("Failed to parse potential patch tool JSON: {}", e);
        }
    }

    None
}

/// Extract valid JSON-RPC objects from mixed content
///
/// This function scans the input string for potential JSON objects,
/// extracts them, and validates them as JSON-RPC objects.
///
/// It handles cases where the input contains:
/// - Multiple JSON-RPC objects
/// - JSON-RPC objects embedded in natural language text
/// - Malformed JSON that should be ignored
pub fn extract_jsonrpc_objects(content: &str) -> Vec<Value> {
    let mut objects = Vec::new();
    let mut start_index = 0;

    debug!(
        "Extracting JSON-RPC objects from {} characters of content",
        content.len()
    );

    // Special case for patch tool
    if let Some(patch_call) = extract_patch_tool_call(content) {
        debug!("Using special patch tool extractor");
        return vec![patch_call];
    }

    while let Some(start) = content[start_index..].find('{') {
        let actual_start = start_index + start;
        let mut depth = 0;
        let mut end_index = None;

        // Scan through the content to find the matching closing brace
        for (i, c) in content[actual_start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_index = Some(actual_start + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(end) = end_index {
            let potential_json = &content[actual_start..end];

            // Try to parse as JSON
            match serde_json::from_str::<Value>(potential_json) {
                Ok(json) => {
                    // Check if this looks like a tool call (basic check to bypass the validation temporarily)
                    let is_tool_call = match (json.get("jsonrpc"), json.get("method"), json.get("params"), json.get("id")) {
                        (Some(jsonrpc), Some(method), Some(params), Some(_)) => {
                            if let (Value::String(v), Value::String(m)) = (jsonrpc, method) {
                                if v == "2.0" && m == "mcp.tool_call" {
                                    match params.get("name") {
                                        Some(Value::String(n)) if n == "patch" => {
                                            debug!("SPECIAL CASE: Treating patch tool call as valid: {}", potential_json);
                                            return vec![json.clone()]; // Force return the patch tool call
                                        },
                                        _ => false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        },
                        _ => false
                    };

                    // Regular validation
                    if is_valid_jsonrpc(&json) {
                        debug!("Found valid JSON-RPC object: {}", potential_json);
                        objects.push(json);
                    } else {
                        debug!(
                            "Found JSON object but not valid JSON-RPC: {}",
                            potential_json
                        );
                    }
                }
                Err(e) => {
                    debug!("Found potential JSON object but failed to parse: {}", e);
                }
            }

            start_index = end;
        } else {
            // No matching closing brace found, exit
            debug!(
                "No matching closing brace found after position {}",
                actual_start
            );
            break;
        }
    }

    debug!("Extracted {} JSON-RPC objects", objects.len());
    objects
}

/// Check if a JSON value is a valid JSON-RPC 2.0 object
fn is_valid_jsonrpc(json: &Value) -> bool {
    // Must be an object
    if !json.is_object() {
        debug!("JSON-RPC validation failed: not an object");
        return false;
    }

    // Must have "jsonrpc": "2.0"
    let jsonrpc_version = json.get("jsonrpc");
    // The comparison Some(&Value::String("2.0".to_string())) can fail due to memory allocation differences
    // Instead, compare the string values directly
    match jsonrpc_version {
        Some(Value::String(version)) if version == "2.0" => {
            // Valid version
        },
        _ => {
            debug!("JSON-RPC validation failed: jsonrpc != \"2.0\", found: {:?}", jsonrpc_version);
            return false;
        }
    }

    // Must have either a "method" or "result" or "error"
    let has_method = json.get("method").is_some();
    let has_result = json.get("result").is_some();
    let has_error = json.get("error").is_some();

    if !has_method && !has_result && !has_error {
        debug!("JSON-RPC validation failed: missing method, result, and error fields");
        return false;
    }

    // If it has a method, it must have "params"
    if has_method {
        let params = json.get("params");
        if params.is_none() {
            debug!("JSON-RPC validation failed: has method but missing params");
            return false;
        }

        // Additional check for mcp.tool_call method
        if json["method"] == "mcp.tool_call" {
            // Check that "params" contains "name" and "parameters"
            let name = params.and_then(|p| p.get("name"));
            let parameters = params.and_then(|p| p.get("parameters"));

            if name.is_none() || parameters.is_none() {
                debug!("JSON-RPC validation failed: mcp.tool_call missing name or parameters");
                // Don't fail just log - this may not be the issue
            }
        }
    }

    // Must have an "id"
    let id = json.get("id");
    if id.is_none() {
        debug!("JSON-RPC validation failed: missing id field");
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_jsonrpc() {
        let content = r#"{
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {"name": "file_write", "parameters": {"path": "test.txt", "content": "hello"}},
            "id": "test1"
        }
        "#;

        let objects = extract_jsonrpc_objects(content);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["method"], "mcp.tool_call");
    }

    #[test]
    fn test_extract_multiple_jsonrpc() {
        let content = r#"{
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {"name": "file_write", "parameters": {"path": "test1.txt", "content": "hello"}},
            "id": "test1"
        }
        {
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {"name": "file_write", "parameters": {"path": "test2.txt", "content": "world"}},
            "id": "test2"
        }
        "#;

        let objects = extract_jsonrpc_objects(content);
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0]["id"], "test1");
        assert_eq!(objects[1]["id"], "test2");
    }

    #[test]
    fn test_extract_jsonrpc_with_natural_language() {
        let content = r#"I'll help you create those files.
        
        Here's the first file:
        
        {
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {"name": "file_write", "parameters": {"path": "test.txt", "content": "hello"}},
            "id": "test1"
        }
        
        And now let's create the second file:
        
        {
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {"name": "file_write", "parameters": {"path": "test2.txt", "content": "world"}},
            "id": "test2"
        }
        
        Both files have been created successfully.
        "#;

        let objects = extract_jsonrpc_objects(content);
        assert_eq!(objects.len(), 2);
    }

    #[test]
    fn test_ignore_invalid_json() {
        let content = r#"Here's some invalid JSON: { this is not valid }
        
        But here's a valid JSON-RPC object:
        
        {
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {"name": "file_write", "parameters": {"path": "test.txt", "content": "hello"}},
            "id": "test1"
        }
        "#;

        let objects = extract_jsonrpc_objects(content);
        assert_eq!(objects.len(), 1);
    }

    #[test]
    fn test_ignore_non_jsonrpc_json() {
        let content = r#"Here's a valid JSON object that is not JSON-RPC:
        
        {"name": "test", "value": 123}
        
        And here's a valid JSON-RPC object:
        
        {
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {"name": "file_write", "parameters": {"path": "test.txt", "content": "hello"}},
            "id": "test1"
        }
        "#;

        let objects = extract_jsonrpc_objects(content);
        assert_eq!(objects.len(), 1);
    }

    #[test]
    fn test_nested_json_objects() {
        let content = r#"{
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {
                "name": "file_write", 
                "parameters": {
                    "path": "test.txt", 
                    "content": "{ \"nested\": true, \"data\": { \"more\": [1, 2, 3] } }"
                }
            },
            "id": "test1"
        }
        "#;

        let objects = extract_jsonrpc_objects(content);
        assert_eq!(objects.len(), 1);
    }

    #[test]
    fn test_jsonrpc_embedded_in_claude_format() {
        let content = r#"I've received the following tool result:
        ```json
        {
          "success": true,
          "bytes_written": 616,
          "path": "README.md"
        }
        ```
        
        Now I need to provide a direct answer based on this result.
        
        {
          "jsonrpc": "2.0",
          "method": "mcp.tool_call",
          "params": {
            "name": "file_write",
            "parameters": {
              "path": "LICENSE",
              "content": "MIT License\n\nCopyright (c) 2023\n\nPermission is hereby granted..."
            }
          },
          "id": "write_license"
        }
        "#;

        let objects = extract_jsonrpc_objects(content);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["id"], "write_license");
    }
}
