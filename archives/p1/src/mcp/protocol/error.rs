use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Standard JSON-RPC 2.0 error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    // JSON-RPC 2.0 error codes
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,

    // Reserved for implementation-defined server errors
    ServerError = -32000,

    // MCP-specific error codes
    ResourceNotFound = -33000,
    ResourceAccessDenied = -33001,
    ToolExecutionFailed = -33002,
    InvalidTool = -33003,
    PromptExecutionFailed = -33004,
    SamplingFailed = -33005,
    RootNotFound = -33006,
    InvalidRoot = -33007,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as i32)
    }
}

/// JSON-RPC 2.0 error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    /// Error code
    pub code: i32,

    /// Error message
    pub message: String,

    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Error {
    /// Create a new JSON-RPC 2.0 error
    pub fn new(code: i32, message: String, data: Option<Value>) -> Self {
        Self {
            code,
            message,
            data,
        }
    }

    /// Create an error from a predefined error code
    pub fn from_code(code: ErrorCode, data: Option<Value>) -> Self {
        let message = match code {
            ErrorCode::ParseError => "Parse error".to_string(),
            ErrorCode::InvalidRequest => "Invalid request".to_string(),
            ErrorCode::MethodNotFound => "Method not found".to_string(),
            ErrorCode::InvalidParams => "Invalid params".to_string(),
            ErrorCode::InternalError => "Internal error".to_string(),
            ErrorCode::ServerError => "Server error".to_string(),
            ErrorCode::ResourceNotFound => "Resource not found".to_string(),
            ErrorCode::ResourceAccessDenied => "Resource access denied".to_string(),
            ErrorCode::ToolExecutionFailed => "Tool execution failed".to_string(),
            ErrorCode::InvalidTool => "Invalid tool".to_string(),
            ErrorCode::PromptExecutionFailed => "Prompt execution failed".to_string(),
            ErrorCode::SamplingFailed => "Sampling failed".to_string(),
            ErrorCode::RootNotFound => "Root not found".to_string(),
            ErrorCode::InvalidRoot => "Invalid root".to_string(),
        };

        Self {
            code: code as i32,
            message,
            data,
        }
    }

    // Standard error factory methods

    /// Create a parse error
    pub fn parse_error() -> Self {
        Self::from_code(ErrorCode::ParseError, None)
    }

    /// Create an invalid request error
    pub fn invalid_request() -> Self {
        Self::from_code(ErrorCode::InvalidRequest, None)
    }

    /// Create a method not found error
    pub fn method_not_found() -> Self {
        Self::from_code(ErrorCode::MethodNotFound, None)
    }

    /// Create an invalid params error
    pub fn invalid_params() -> Self {
        Self::from_code(ErrorCode::InvalidParams, None)
    }

    /// Create an internal error
    pub fn internal_error() -> Self {
        Self::from_code(ErrorCode::InternalError, None)
    }

    /// Create a server error
    pub fn server_error(data: Option<Value>) -> Self {
        Self::from_code(ErrorCode::ServerError, data)
    }

    // MCP-specific error factory methods

    /// Create a resource not found error
    pub fn resource_not_found(resource_uri: &str) -> Self {
        Self::from_code(
            ErrorCode::ResourceNotFound,
            Some(Value::String(resource_uri.to_string())),
        )
    }

    /// Create a resource access denied error
    pub fn resource_access_denied(resource_uri: &str) -> Self {
        Self::from_code(
            ErrorCode::ResourceAccessDenied,
            Some(Value::String(resource_uri.to_string())),
        )
    }

    /// Create a tool execution failed error
    pub fn tool_execution_failed(tool_name: &str, reason: &str) -> Self {
        Self::from_code(
            ErrorCode::ToolExecutionFailed,
            Some(serde_json::json!({
                "tool": tool_name,
                "reason": reason
            })),
        )
    }

    /// Create an invalid tool error
    pub fn invalid_tool(tool_name: &str) -> Self {
        Self::from_code(
            ErrorCode::InvalidTool,
            Some(Value::String(tool_name.to_string())),
        )
    }
}
