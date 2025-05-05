use serde::{Deserialize, Serialize};
use thiserror::Error;

// Export the validation module
pub mod validation;

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Format error: {0}")]
    FormatError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    // JSON-RPC 2.0 fields
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    // JSON-RPC 2.0 fields
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<ResponseError>,
    pub id: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// Helper function to create a proper JSON-RPC response
pub fn create_response(result: serde_json::Value, id: &str) -> Response {
    Response {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id: serde_json::Value::String(id.to_string()),
    }
}

// Helper function to create a proper JSON-RPC error response
pub fn create_error_response(code: i32, message: &str, id: &str) -> Response {
    Response {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(ResponseError {
            code,
            message: message.to_string(),
            data: None,
        }),
        id: serde_json::Value::String(id.to_string()),
    }
}
