use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::mcp::protocol::error::Error;
use crate::mcp::protocol::message::{Id, Request, Response};

/// Type alias for a method handler function
pub type MethodHandler = Box<dyn Fn(&Request) -> Response + Send + Sync>;

/// MCP handler that processes JSON-RPC requests
#[derive(Clone)]
pub struct McpHandler {
    /// Registered method handlers
    methods: Arc<RwLock<HashMap<String, MethodHandler>>>,
}

impl McpHandler {
    /// Create a new MCP handler
    pub fn new() -> Self {
        Self {
            methods: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a method handler
    pub fn register_method<F>(&self, method: &str, handler: F)
    where
        F: Fn(&Request) -> Response + Send + Sync + 'static,
    {
        let mut methods = self.methods.write().unwrap();
        methods.insert(method.to_string(), Box::new(handler));
    }

    /// Deregister a method handler
    pub fn deregister_method(&self, method: &str) {
        let mut methods = self.methods.write().unwrap();
        methods.remove(method);
    }

    /// Process a JSON-RPC request
    pub fn process(&self, request: &Request) -> Response {
        // Validate request
        if let Err(error) = request.validate() {
            return match &request.id {
                Some(id) => Response::error(error, id.clone()),
                None => Response::error(error, Id::Null),
            };
        }

        // Get method handler
        let methods = self.methods.read().unwrap();
        if let Some(handler) = methods.get(&request.method) {
            // Call handler
            handler(request)
        } else {
            // Method not found
            match &request.id {
                Some(id) => Response::method_not_found(id.clone()),
                None => Response::method_not_found(Id::Null),
            }
        }
    }

    /// Process a request from JSON
    pub fn process_json(&self, json: &str) -> Result<String, Error> {
        // Parse request
        let request: Request = serde_json::from_str(json).map_err(|_| Error::parse_error())?;

        // Process request
        let response = self.process(&request);

        // Serialize response
        let response_json =
            serde_json::to_string(&response).map_err(|_| Error::internal_error())?;

        Ok(response_json)
    }
}
