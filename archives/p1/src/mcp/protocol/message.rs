use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

use super::error::{Error, ErrorCode};

/// JSON-RPC 2.0 version string
pub const JSONRPC_VERSION: &str = "2.0";

/// Represents a JSON-RPC 2.0 ID, which can be a string, number, or null
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Id {
    String(String),
    Number(i64),
    Null,
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Id::String(s) => write!(f, "{}", s),
            Id::Number(n) => write!(f, "{}", n),
            Id::Null => write!(f, "null"),
        }
    }
}

/// JSON-RPC 2.0 version identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Version(pub String);

impl Default for Version {
    fn default() -> Self {
        Version(JSONRPC_VERSION.to_string())
    }
}

/// Represents a JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// The JSON-RPC version, must be "2.0"
    pub jsonrpc: Version,

    /// The method to be invoked
    pub method: String,

    /// The method parameters
    pub params: Option<Value>,

    /// Client-provided identifier
    pub id: Option<Id>,
}

impl Request {
    /// Create a new JSON-RPC 2.0 request
    pub fn new(method: String, params: Option<Value>, id: Option<Id>) -> Self {
        Self {
            jsonrpc: Version::default(),
            method,
            params,
            id,
        }
    }

    /// Check if this is a notification (no id)
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// Validate the request structure
    pub fn validate(&self) -> Result<(), Error> {
        // Verify jsonrpc version
        if self.jsonrpc.0 != JSONRPC_VERSION {
            return Err(Error::invalid_request());
        }

        // Method name should not be empty
        if self.method.is_empty() {
            return Err(Error::invalid_request());
        }

        Ok(())
    }
}

/// Represents a JSON-RPC 2.0 successful response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse {
    /// The JSON-RPC version, must be "2.0"
    pub jsonrpc: Version,

    /// The result of the method call
    pub result: Value,

    /// Client-provided identifier from the request
    pub id: Id,
}

/// Represents a JSON-RPC 2.0 error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// The JSON-RPC version, must be "2.0"
    pub jsonrpc: Version,

    /// The error information
    pub error: Error,

    /// Client-provided identifier from the request
    pub id: Id,
}

/// Represents a JSON-RPC 2.0 response (success or error)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    Success(SuccessResponse),
    Error(ErrorResponse),
}

impl Response {
    /// Create a successful response
    pub fn success(result: Value, id: Id) -> Self {
        Response::Success(SuccessResponse {
            jsonrpc: Version::default(),
            result,
            id,
        })
    }

    /// Create an error response
    pub fn error(error: Error, id: Id) -> Self {
        Response::Error(ErrorResponse {
            jsonrpc: Version::default(),
            error,
            id,
        })
    }

    /// Create a parse error response
    pub fn parse_error(id: Id) -> Self {
        Response::error(Error::parse_error(), id)
    }

    /// Create an invalid request error response
    pub fn invalid_request(id: Id) -> Self {
        Response::error(Error::invalid_request(), id)
    }

    /// Create a method not found error response
    pub fn method_not_found(id: Id) -> Self {
        Response::error(Error::method_not_found(), id)
    }

    /// Create an invalid params error response
    pub fn invalid_params(id: Id) -> Self {
        Response::error(Error::invalid_params(), id)
    }

    /// Create an internal error response
    pub fn internal_error(id: Id) -> Self {
        Response::error(Error::internal_error(), id)
    }
}

/// A batch of JSON-RPC 2.0 requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest(pub Vec<Request>);

/// A batch of JSON-RPC 2.0 responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse(pub Vec<Response>);
