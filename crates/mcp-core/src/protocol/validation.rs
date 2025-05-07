use serde_json::Value;
use tracing::{debug, warn};

/// Validation result for LLM responses
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Response is valid JSON-RPC
    Valid(Value),

    /// Response is not JSON-RPC at all
    InvalidFormat(String),

    /// Response contains both text and JSON-RPC mixed together
    Mixed {
        text: String,
        json_rpc: Option<Value>,
    },

    /// Response is JSON but not valid JSON-RPC
    NotJsonRpc(Value),

    /// Response contains multiple valid JSON-RPC objects
    MultipleJsonRpc(Vec<Value>),
}

/// Check if a value is a valid JSON-RPC object
#[cfg(test)]
fn is_valid_jsonrpc(value: &Value) -> bool {
    // Must be an object
    if !value.is_object() {
        return false;
    }

    // Must have "jsonrpc": "2.0"
    if value.get("jsonrpc") != Some(&Value::String("2.0".to_string())) {
        return false;
    }

    // Must have either result or error (not both), plus ID
    let has_result = value.get("result").is_some();
    let has_error = value.get("error").is_some();
    let has_id = value.get("id").is_some();

    // For tool calls, must have method and params
    let has_method = value.get("method").is_some();
    let has_params = value.get("params").is_some();

    // Valid if (has result XOR has error) AND has id, OR has method and params and id
    ((has_result != has_error) && has_id) || (has_method && has_params && has_id)
}

// We now use jsonrpc::extract_jsonrpc_objects instead

/// Validate an LLM response to ensure it follows JSON-RPC format
pub fn validate_llm_response(content: &str) -> ValidationResult {
    debug!("Validating LLM response format");

    // Trim the content
    let trimmed = content.trim();

    // Use our JSON-RPC extractor to find all JSON-RPC objects
    let json_objects = crate::jsonrpc::extract_jsonrpc_objects(trimmed);
    debug!("Extracted {} JSON-RPC objects", json_objects.len());
    
    // If we found multiple JSON-RPC objects
    if json_objects.len() > 1 {
        debug!("Found {} valid JSON-RPC objects", json_objects.len());
        return ValidationResult::MultipleJsonRpc(json_objects);
    }
    
    // If we found exactly one JSON-RPC object
    if json_objects.len() == 1 {
        let json_value = &json_objects[0];
        
        // Check if the entire content is just this JSON object
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            if parsed == *json_value {
                debug!("Response is valid JSON-RPC");
                return ValidationResult::Valid(parsed);
            }
        }
        
        // Otherwise, it's a mixed response with both text and JSON-RPC
        warn!("Response contains both text and valid JSON-RPC");
        
        // Extract the text part - this is a bit tricky, let's find the JSON in the original text
        if let Some(json_start) = trimmed.find('{') {
            let text = trimmed[..json_start].trim().to_string();
            return ValidationResult::Mixed {
                text,
                json_rpc: Some(json_value.clone()),
            };
        } else {
            // Fallback - shouldn't happen since we found a JSON object
            return ValidationResult::Mixed {
                text: trimmed.to_string(),
                json_rpc: Some(json_value.clone()),
            };
        }
    }
    
    // No JSON-RPC objects found, check if it's regular JSON
    if let Ok(json_value) = serde_json::from_str::<Value>(trimmed) {
        warn!("Response is JSON but not valid JSON-RPC");
        return ValidationResult::NotJsonRpc(json_value);
    }
    
    // Check if it's a mixed response with invalid JSON
    if trimmed.contains('{') && trimmed.contains('}') {
        warn!("Response contains text mixed with invalid JSON");
        return ValidationResult::Mixed {
            text: trimmed.to_string(),
            json_rpc: None,
        };
    }

    // Not valid JSON and not a mixed response
    warn!("Response is not in JSON-RPC format");
    ValidationResult::InvalidFormat(trimmed.to_string())
}

/// Create a correction prompt to send back to the LLM
pub fn create_correction_prompt(validation_result: &ValidationResult) -> String {
    match validation_result {
        ValidationResult::Valid(_) => {
            // No correction needed
            String::new()
        }
        ValidationResult::MultipleJsonRpc(objects) => {
            format!(
                "Your last response contained multiple JSON-RPC objects ({}). \
                According to the MCP protocol, you should respond with a single JSON-RPC object at a time. \
                If you need to perform multiple actions, make one tool call at a time and wait for the result.
                
                Please reformat your response as a single JSON-RPC object. For your next response, choose ONE of:
                
                1. A text response using:
                {{
                  \"jsonrpc\": \"2.0\",
                  \"result\": \"Your message here...\",
                  \"id\": \"response_id\"
                }}
                
                2. OR a single tool call using:
                {{
                  \"jsonrpc\": \"2.0\",
                  \"method\": \"mcp.tool_call\",
                  \"params\": {{
                    \"name\": \"tool_name\",
                    \"parameters\": {{...}}
                  }},
                  \"id\": \"tool_call_id\"
                }}
                
                Please respond with just ONE JSON-RPC object.",
                objects.len()
            )
        }
        ValidationResult::InvalidFormat(text) => {
            format!(
                "Your last response was not in the required JSON-RPC 2.0 format. \
                Please reformat your response according to the MCP protocol. \
                Your message should be formatted as a single, valid JSON-RPC object like this:
                
                {{
                  \"jsonrpc\": \"2.0\",
                  \"result\": \"Your message here...\",
                  \"id\": \"response_id\"
                }}
                
                Your original message content was: {}
                
                Please respond ONLY with a valid JSON-RPC object.",
                if text.len() > 200 {
                    // Truncate long responses
                    format!("\"{}...\" (truncated)", &text[..200])
                } else {
                    format!("\"{}\"", text)
                }
            )
        }
        ValidationResult::Mixed { text, json_rpc } => {
            format!(
                "Your last response mixed regular text with JSON-RPC, which breaks the protocol. \
                According to the MCP protocol, you should respond ONLY with a valid JSON-RPC object, \
                not with a combination of text and JSON.
                
                Your text content was: {}
                
                {}
                
                Please respond ONLY with a valid JSON-RPC object for your ENTIRE message:",
                if text.len() > 200 {
                    // Truncate long responses
                    format!("\"{}...\" (truncated)", &text[..200])
                } else {
                    format!("\"{}\"", text)
                },
                if let Some(json) = json_rpc {
                    format!(
                        "Your JSON part was: {}",
                        serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string())
                    )
                } else {
                    "".to_string()
                }
            )
        }
        ValidationResult::NotJsonRpc(json) => {
            format!(
                "Your last response was valid JSON but not a valid JSON-RPC 2.0 object. \
                According to the MCP protocol, your response must be a single JSON-RPC object \
                with the required fields: jsonrpc, result/error, and id.
                
                Your JSON was: {}
                
                Please respond with a proper JSON-RPC object like this:
                
                {{
                  \"jsonrpc\": \"2.0\",
                  \"result\": \"Your message here...\",
                  \"id\": \"response_id\"
                }}",
                serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string())
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_valid_jsonrpc_result() {
        let content = r#"{"jsonrpc":"2.0","result":"Hello, world!","id":"123"}"#;
        let result = validate_llm_response(content);

        match result {
            ValidationResult::Valid(_) => {
                // Test passed
            }
            _ => panic!("Expected Valid, got {:?}", result),
        }
    }

    #[test]
    fn test_validate_valid_jsonrpc_error() {
        let content =
            r#"{"jsonrpc":"2.0","error":{"code":-32000,"message":"Error occurred"},"id":"123"}"#;
        let result = validate_llm_response(content);

        match result {
            ValidationResult::Valid(_) => {
                // Test passed
            }
            _ => panic!("Expected Valid, got {:?}", result),
        }
    }

    #[test]
    fn test_validate_valid_jsonrpc_tool_call() {
        let content = r#"{"jsonrpc":"2.0","method":"mcp.tool_call","params":{"name":"test","parameters":{}},"id":"123"}"#;
        let result = validate_llm_response(content);

        match result {
            ValidationResult::Valid(_) => {
                // Test passed
            }
            _ => panic!("Expected Valid, got {:?}", result),
        }
    }

    #[test]
    fn test_validate_invalid_format() {
        let content = "This is just plain text, not JSON";
        let result = validate_llm_response(content);

        match result {
            ValidationResult::InvalidFormat(_) => {
                // Test passed
            }
            _ => panic!("Expected InvalidFormat, got {:?}", result),
        }
    }

    #[test]
    fn test_validate_mixed_content() {
        let content = r#"I'll help you with that. Here's the JSON-RPC call:
            {"jsonrpc":"2.0","method":"mcp.tool_call","params":{"name":"file_read","parameters":{"path":"test.txt"}},"id":"123"}"#;
        let result = validate_llm_response(content);

        match result {
            ValidationResult::Mixed { text, json_rpc } => {
                assert!(text.contains("I'll help you with that"));
                assert!(json_rpc.is_some());
            }
            _ => panic!("Expected Mixed, got {:?}", result),
        }
    }

    #[test]
    fn test_validate_not_jsonrpc() {
        let content = r#"{"message":"Hello, world!"}"#;
        let result = validate_llm_response(content);

        match result {
            ValidationResult::NotJsonRpc(_) => {
                // Test passed
            }
            _ => panic!("Expected NotJsonRpc, got {:?}", result),
        }
    }

    #[test]
    fn test_is_valid_jsonrpc() {
        // Valid result response
        let valid_result = json!({
            "jsonrpc": "2.0",
            "result": "Hello",
            "id": "123"
        });
        assert!(is_valid_jsonrpc(&valid_result));

        // Valid error response
        let valid_error = json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32000,
                "message": "Error"
            },
            "id": "123"
        });
        assert!(is_valid_jsonrpc(&valid_error));

        // Valid tool call
        let valid_tool_call = json!({
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {
                "name": "test",
                "parameters": {}
            },
            "id": "123"
        });
        assert!(is_valid_jsonrpc(&valid_tool_call));

        // Invalid - not an object
        let invalid_1 = json!("Hello");
        assert!(!is_valid_jsonrpc(&invalid_1));

        // Invalid - wrong jsonrpc version
        let invalid_2 = json!({
            "jsonrpc": "1.0",
            "result": "Hello",
            "id": "123"
        });
        assert!(!is_valid_jsonrpc(&invalid_2));

        // Invalid - missing jsonrpc
        let invalid_3 = json!({
            "result": "Hello",
            "id": "123"
        });
        assert!(!is_valid_jsonrpc(&invalid_3));

        // Invalid - both result and error
        let invalid_4 = json!({
            "jsonrpc": "2.0",
            "result": "Hello",
            "error": {
                "code": -32000,
                "message": "Error"
            },
            "id": "123"
        });
        assert!(!is_valid_jsonrpc(&invalid_4));

        // Invalid - missing ID
        let invalid_5 = json!({
            "jsonrpc": "2.0",
            "result": "Hello"
        });
        assert!(!is_valid_jsonrpc(&invalid_5));
    }
}
