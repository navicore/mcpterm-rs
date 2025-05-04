use anyhow::Result;
use jsonschema::{Draft, JSONSchema};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("Schema validation error: {0}")]
    ValidationError(String),

    #[error("Schema compilation error: {0}")]
    CompilationError(String),
}

/// MCP Schema Manager is responsible for storing and validating against MCP JSON schemas
pub struct McpSchemaManager {
    request_schema: JSONSchema,
    response_schema: JSONSchema,
    tool_call_schema: JSONSchema,
}

impl Default for McpSchemaManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpSchemaManager {
    /// Create a new schema manager with pre-compiled MCP schemas
    pub fn new() -> Self {
        // JSON-RPC 2.0 Request Schema
        let request_schema_json = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "MCP JSON-RPC Request",
            "type": "object",
            "required": ["jsonrpc", "method", "params", "id"],
            "properties": {
                "jsonrpc": {
                    "type": "string",
                    "enum": ["2.0"]
                },
                "method": {
                    "type": "string"
                },
                "params": {
                    "type": "object"
                },
                "id": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "number" }
                    ]
                }
            }
        });

        // JSON-RPC 2.0 Response Schema
        let response_schema_json = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "MCP JSON-RPC Response",
            "type": "object",
            "required": ["jsonrpc", "id"],
            "oneOf": [
                {
                    "required": ["result"],
                    "properties": {
                        "jsonrpc": {
                            "type": "string",
                            "enum": ["2.0"]
                        },
                        "result": {
                            "type": ["object", "string", "array", "number", "boolean", "null"]
                        },
                        "id": {
                            "oneOf": [
                                { "type": "string" },
                                { "type": "number" },
                                { "type": "null" }
                            ]
                        }
                    }
                },
                {
                    "required": ["error"],
                    "properties": {
                        "jsonrpc": {
                            "type": "string",
                            "enum": ["2.0"]
                        },
                        "error": {
                            "type": "object",
                            "required": ["code", "message"],
                            "properties": {
                                "code": { "type": "number" },
                                "message": { "type": "string" },
                                "data": {}
                            }
                        },
                        "id": {
                            "oneOf": [
                                { "type": "string" },
                                { "type": "number" },
                                { "type": "null" }
                            ]
                        }
                    }
                }
            ]
        });

        // Tool Call Schema
        let tool_call_schema_json = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "MCP Tool Call",
            "type": "object",
            "required": ["name", "parameters"],
            "properties": {
                "name": { "type": "string" },
                "parameters": { "type": "object" }
            }
        });

        // Compile schemas
        let request_schema =
            Self::compile_schema(request_schema_json).expect("Failed to compile request schema");
        let response_schema =
            Self::compile_schema(response_schema_json).expect("Failed to compile response schema");
        let tool_call_schema = Self::compile_schema(tool_call_schema_json)
            .expect("Failed to compile tool call schema");

        Self {
            request_schema,
            response_schema,
            tool_call_schema,
        }
    }

    // Compile a schema from JSON
    fn compile_schema(schema_json: serde_json::Value) -> Result<JSONSchema, SchemaError> {
        JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_json)
            .map_err(|e| SchemaError::CompilationError(e.to_string()))
    }

    /// Validate a JSON-RPC request
    pub fn validate_request(&self, request_json: &serde_json::Value) -> Result<(), SchemaError> {
        self.request_schema
            .validate(request_json)
            .map_err(|errors| {
                let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
                SchemaError::ValidationError(error_messages.join(", "))
            })?;
        Ok(())
    }

    /// Validate a JSON-RPC response
    pub fn validate_response(&self, response_json: &serde_json::Value) -> Result<(), SchemaError> {
        self.response_schema
            .validate(response_json)
            .map_err(|errors| {
                let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
                SchemaError::ValidationError(error_messages.join(", "))
            })?;
        Ok(())
    }

    /// Validate a tool call
    pub fn validate_tool_call(
        &self,
        tool_call_json: &serde_json::Value,
    ) -> Result<(), SchemaError> {
        self.tool_call_schema
            .validate(tool_call_json)
            .map_err(|errors| {
                let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
                SchemaError::ValidationError(error_messages.join(", "))
            })?;
        Ok(())
    }

    /// Get the system prompt addition that instructs the LLM to use MCP
    pub fn get_mcp_system_prompt(&self) -> &str {
        r#"
You are an AI assistant that follows the Model Context Protocol (MCP). 
You MUST communicate using valid JSON in the JSON-RPC 2.0 format.

Here are the rules:

1. For regular responses, use:
{
  "jsonrpc": "2.0",
  "result": "Your message here...",
  "id": "<request_id>"
}

2. For tool calls, use:
{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "<tool_name>",
    "parameters": {
      // Tool-specific parameters
    }
  },
  "id": "<request_id>"
}

3. For errors, use:
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Error description"
  },
  "id": "<request_id>"
}

Examples of available tools:
1. "shell": Execute a shell command
   Parameters: { "command": "string" }

2. "file_read": Read a file
   Parameters: { "path": "string" }

3. "file_write": Write to a file
   Parameters: { "path": "string", "content": "string" }

4. "search": Search for files or content
   Parameters: { "query": "string", "path": "string" }

Always ensure your responses are syntactically valid JSON. 
Never include multiple JSON objects in a single response.
If you require more information or the result of a tool call, make a tool call request and wait for the result.
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_validation_request() {
        let schema_manager = McpSchemaManager::new();

        // Valid request
        let valid_request = json!({
            "jsonrpc": "2.0",
            "method": "mcp.tool_call",
            "params": {
                "name": "shell",
                "parameters": {
                    "command": "ls -la"
                }
            },
            "id": "req_123"
        });

        assert!(schema_manager.validate_request(&valid_request).is_ok());

        // Invalid request (missing method)
        let invalid_request = json!({
            "jsonrpc": "2.0",
            "params": {
                "name": "shell",
                "parameters": {
                    "command": "ls -la"
                }
            },
            "id": "req_123"
        });

        assert!(schema_manager.validate_request(&invalid_request).is_err());
    }

    #[test]
    fn test_schema_validation_response() {
        let schema_manager = McpSchemaManager::new();

        // Valid response
        let valid_response = json!({
            "jsonrpc": "2.0",
            "result": "This is a test response",
            "id": "req_123"
        });

        assert!(schema_manager.validate_response(&valid_response).is_ok());

        // Valid error response
        let valid_error = json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32000,
                "message": "An error occurred"
            },
            "id": "req_123"
        });

        assert!(schema_manager.validate_response(&valid_error).is_ok());

        // Invalid response (both result and error)
        let invalid_response = json!({
            "jsonrpc": "2.0",
            "result": "This is a test response",
            "error": {
                "code": -32000,
                "message": "An error occurred"
            },
            "id": "req_123"
        });

        assert!(schema_manager.validate_response(&invalid_response).is_err());
    }
}
