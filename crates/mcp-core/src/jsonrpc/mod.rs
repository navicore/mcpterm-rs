//! JSON-RPC utility functions for MCP

pub mod extractor;
pub mod splitter;

pub use extractor::extract_jsonrpc_objects;
pub use splitter::{split_jsonrpc_and_text, SplitContent};

use serde_json::Value;
use anyhow::Result;

/// Extract JSON-RPC objects and their positions from text
///
/// This function extracts JSON-RPC objects from text and returns them
/// along with their start and end positions in the text.
///
/// Returns a Result with a Vec of tuples containing:
/// - The parsed JSON-RPC object
/// - The start position (index) in the original string
/// - The end position (index) in the original string
pub fn extract_jsonrpc_objects_with_positions(content: &str) -> Result<Vec<(Value, usize, usize)>> {
    let mut objects = Vec::new();
    let mut start_index = 0;

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
            if let Ok(json) = serde_json::from_str::<Value>(potential_json) {
                // Check if it's a valid JSON-RPC object
                if json.get("jsonrpc").is_some() {
                    objects.push((json, actual_start, end));
                }
            }

            start_index = end;
        } else {
            // No matching closing brace found, exit
            break;
        }
    }

    Ok(objects)
}
