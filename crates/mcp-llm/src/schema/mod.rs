use anyhow::Result;
use jsonschema::{Draft, Validator};
use mcp_core::prompts::PromptManager;
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
    request_schema: Validator,
    response_schema: Validator,
    tool_call_schema: Validator,
    prompt_manager: PromptManager,
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
            prompt_manager: PromptManager::new(),
        }
    }

    // Compile a schema from JSON
    fn compile_schema(schema_json: serde_json::Value) -> Result<Validator, SchemaError> {
        Validator::options()
            .with_draft(Draft::Draft7)
            .build(&schema_json)
            .map_err(|e| SchemaError::CompilationError(e.to_string()))
    }

    /// Validate a JSON-RPC request
    pub fn validate_request(&self, request_json: &serde_json::Value) -> Result<(), SchemaError> {
        self.request_schema
            .validate(request_json)
            .map_err(|error| SchemaError::ValidationError(error.to_string()))?;
        Ok(())
    }

    /// Validate a JSON-RPC response
    pub fn validate_response(&self, response_json: &serde_json::Value) -> Result<(), SchemaError> {
        self.response_schema
            .validate(response_json)
            .map_err(|error| SchemaError::ValidationError(error.to_string()))?;
        Ok(())
    }

    /// Validate a tool call
    pub fn validate_tool_call(
        &self,
        tool_call_json: &serde_json::Value,
    ) -> Result<(), SchemaError> {
        self.tool_call_schema
            .validate(tool_call_json)
            .map_err(|error| SchemaError::ValidationError(error.to_string()))?;
        Ok(())
    }

    /// Get the system prompt addition that instructs the LLM to use MCP with custom tool documentation
    pub fn get_mcp_system_prompt_with_tools(&self, tools_doc: &str) -> String {
        self.prompt_manager
            .get_mcp_system_prompt_with_tools(tools_doc)
    }

    /// Get the system prompt addition that instructs the LLM to use MCP
    pub fn get_mcp_system_prompt(&self) -> &str {
        self.prompt_manager.get_mcp_system_prompt()
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
