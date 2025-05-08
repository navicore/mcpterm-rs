//! Functions for splitting mixed content with JSON-RPC objects and text

use serde_json::Value;
use tracing::debug;

use super::extractor::extract_jsonrpc_objects;

/// Represents the result of splitting mixed content
pub struct SplitContent {
    /// The original content
    pub original: String,
    /// Text segments between JSON-RPC objects
    pub text_segments: Vec<String>,
    /// JSON-RPC objects found in the content
    pub json_objects: Vec<Value>,
}

/// Split content into text segments and JSON-RPC objects
///
/// This function creates a structured representation of mixed content,
/// preserving both the JSON-RPC objects and the text segments between them.
pub fn split_jsonrpc_and_text(content: &str) -> SplitContent {
    // Extract the JSON-RPC objects
    let json_objects = extract_jsonrpc_objects(content);
    debug!("Found {} JSON-RPC objects", json_objects.len());

    // If no JSON objects, just return the content as one text segment
    if json_objects.is_empty() {
        return SplitContent {
            original: content.to_string(),
            text_segments: vec![content.to_string()],
            json_objects: vec![],
        };
    }

    // We need to locate where each JSON object appears in the original text
    // This requires using a different approach

    // First, find all JSON object positions by looking for opening and closing braces
    let mut json_start_positions = Vec::new();
    let mut brace_depth = 0;
    let mut start_index = None;

    for (i, c) in content.char_indices() {
        match c {
            '{' => {
                if brace_depth == 0 {
                    start_index = Some(i);
                }
                brace_depth += 1;
            }
            '}' => {
                if brace_depth > 0 {
                    brace_depth -= 1;
                    if brace_depth == 0 && start_index.is_some() {
                        // Check if this JSON substring is a valid JSON-RPC object
                        let json_str = &content[start_index.unwrap()..=i];
                        if let Ok(json_value) = serde_json::from_str::<Value>(json_str) {
                            // Check if it's a valid JSON-RPC
                            if json_value.get("jsonrpc").is_some() {
                                json_start_positions.push((start_index.unwrap(), i + 1));
                            }
                        }
                        start_index = None;
                    }
                }
            }
            _ => {}
        }
    }

    // Sort positions by start index
    json_start_positions.sort_by_key(|&(start, _)| start);

    // Now extract text segments and JSON objects
    let mut text_segments = Vec::new();
    let mut ordered_json_objects = Vec::new();
    let mut current_pos = 0;

    for (start, end) in json_start_positions {
        // Add text segment before the JSON if there is any
        if start > current_pos {
            let text = content[current_pos..start].trim();
            if !text.is_empty() {
                text_segments.push(text.to_string());
            }
        }

        // Parse the JSON object again
        let json_str = &content[start..end];
        if let Ok(json_value) = serde_json::from_str::<Value>(json_str) {
            ordered_json_objects.push(json_value);
        }

        // Update current position
        current_pos = end;
    }

    // Add any remaining text after the last JSON object
    if current_pos < content.len() {
        let text = content[current_pos..].trim();
        if !text.is_empty() {
            text_segments.push(text.to_string());
        }
    }

    SplitContent {
        original: content.to_string(),
        text_segments,
        json_objects: ordered_json_objects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_pure_text() {
        let content = "This is just text with no JSON-RPC objects.";
        let result = split_jsonrpc_and_text(content);

        assert_eq!(result.text_segments.len(), 1);
        assert_eq!(result.text_segments[0], content);
        assert_eq!(result.json_objects.len(), 0);
    }

    #[test]
    fn test_split_pure_json() {
        let content = r#"{"jsonrpc":"2.0","result":"Hello","id":"1"}
        "#;
        let result = split_jsonrpc_and_text(content);

        assert_eq!(result.text_segments.len(), 0);
        assert_eq!(result.json_objects.len(), 1);
        assert_eq!(result.json_objects[0]["result"], "Hello");
    }

    #[test]
    fn test_split_mixed_content() {
        let content = r#"Here is some text.
        
        {"jsonrpc":"2.0","result":"Hello","id":"1"}
        
        Here is more text between objects.
        
        {"jsonrpc":"2.0","method":"mcp.tool_call","params":{"name":"test","parameters":{}},"id":"2"}
        
        And here is text at the end.
        "#;

        let result = split_jsonrpc_and_text(content);

        assert_eq!(result.text_segments.len(), 3);
        assert_eq!(result.json_objects.len(), 2);

        assert!(result.text_segments[0].contains("Here is some text"));
        assert!(result.text_segments[1].contains("Here is more text between objects"));
        assert!(result.text_segments[2].contains("And here is text at the end"));

        assert_eq!(result.json_objects[0]["result"], "Hello");
        assert_eq!(result.json_objects[1]["method"], "mcp.tool_call");
    }

    #[test]
    fn test_split_text_before_only() {
        let content = r#"Here is some text before the JSON.
        
        {"jsonrpc":"2.0","result":"Hello","id":"1"}
        "#;

        let result = split_jsonrpc_and_text(content);

        assert_eq!(result.text_segments.len(), 1);
        assert_eq!(result.json_objects.len(), 1);

        assert!(result.text_segments[0].contains("Here is some text before the JSON"));
        assert_eq!(result.json_objects[0]["result"], "Hello");
    }

    #[test]
    fn test_split_text_after_only() {
        let content = r#"{"jsonrpc":"2.0","result":"Hello","id":"1"}
        
        Here is some text after the JSON.
        "#;

        let result = split_jsonrpc_and_text(content);

        assert_eq!(result.text_segments.len(), 1);
        assert_eq!(result.json_objects.len(), 1);

        assert!(result.text_segments[0].contains("Here is some text after the JSON"));
        assert_eq!(result.json_objects[0]["result"], "Hello");
    }
}
