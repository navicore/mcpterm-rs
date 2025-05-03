use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

use super::{AccessMode, ResourceManager, ResourceMetadata, ResourceType};
use crate::mcp::protocol::error::Error;
use crate::mcp::protocol::message::{Id, Request, Response};

/// Resource list request params
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceListParams {
    /// Base URI to list resources from
    pub uri: String,

    /// Recursive listing flag
    #[serde(default)]
    pub recursive: bool,

    /// Filter by resource type
    #[serde(default)]
    pub resource_type: Option<String>,
}

/// Resource get request params
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceGetParams {
    /// Resource URI
    pub uri: String,

    /// Encoding (default: base64)
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

/// Resource set request params
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceSetParams {
    /// Resource URI
    pub uri: String,

    /// Content
    pub content: String,

    /// Encoding (default: base64)
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

/// Resource delete request params
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceDeleteParams {
    /// Resource URI
    pub uri: String,
}

/// Default encoding function
fn default_encoding() -> String {
    "base64".to_string()
}

/// Register resource methods with the handler
pub fn register_resource_methods(
    handler: &mut crate::mcp::handler::McpHandler,
    resource_manager: Arc<Mutex<ResourceManager>>,
) {
    // Clone for each method
    let resource_manager_list = resource_manager.clone();

    // Register resource/list method
    handler.register_method("resources/list", move |request: &Request| -> Response {
        // Get ID for response
        let id = match &request.id {
            Some(id) => id.clone(),
            None => Id::Null,
        };

        // Parse params
        let params = match &request.params {
            Some(params) => match serde_json::from_value::<ResourceListParams>(params.clone()) {
                Ok(params) => params,
                Err(_) => return Response::invalid_params(id),
            },
            None => return Response::invalid_params(id),
        };

        // Get resource manager
        let resource_manager = match resource_manager_list.lock() {
            Ok(manager) => manager,
            Err(_) => return Response::internal_error(id),
        };

        // List resources
        match resource_manager.list_resources(&params.uri) {
            Ok(resources) => {
                // Filter by resource type if specified
                let filtered_resources = if let Some(type_filter) = params.resource_type {
                    resources
                        .into_iter()
                        .filter(|r| match &r.resource_type {
                            ResourceType::File => type_filter == "file",
                            ResourceType::Directory => type_filter == "directory",
                            ResourceType::Memory => type_filter == "memory",
                            ResourceType::Other(s) => type_filter == *s,
                        })
                        .collect::<Vec<_>>()
                } else {
                    resources
                };

                // Create response
                Response::success(
                    json!({
                        "resources": filtered_resources,
                    }),
                    id,
                )
            }
            Err(_) => {
                // Create error response
                Response::error(Error::resource_access_denied(&params.uri), id)
            }
        }
    });

    // Register resource/get method
    let resource_manager_get = resource_manager.clone();
    handler.register_method("resources/get", move |request: &Request| -> Response {
        // Get ID for response
        let id = match &request.id {
            Some(id) => id.clone(),
            None => Id::Null,
        };

        // Parse params
        let params = match &request.params {
            Some(params) => match serde_json::from_value::<ResourceGetParams>(params.clone()) {
                Ok(params) => params,
                Err(_) => return Response::invalid_params(id),
            },
            None => return Response::invalid_params(id),
        };

        // Get resource manager
        let resource_manager = match resource_manager_get.lock() {
            Ok(manager) => manager,
            Err(_) => return Response::internal_error(id),
        };

        // Get resource
        match resource_manager.get_resource(&params.uri) {
            Ok(resource) => {
                // Read resource content
                match resource.read() {
                    Ok(content) => {
                        // Encode content based on encoding parameter
                        let encoded_content = match params.encoding.as_str() {
                            "base64" => base64::encode(&content),
                            "utf8" | "utf-8" => match String::from_utf8(content) {
                                Ok(text) => text,
                                Err(_) => return Response::error(Error::invalid_params(), id),
                            },
                            _ => return Response::error(Error::invalid_params(), id),
                        };

                        // Get resource metadata
                        match resource.metadata() {
                            Ok(metadata) => {
                                // Create response
                                Response::success(
                                    json!({
                                        "uri": params.uri,
                                        "content": encoded_content,
                                        "encoding": params.encoding,
                                        "metadata": metadata,
                                    }),
                                    id,
                                )
                            }
                            Err(_) => {
                                // Create response without metadata
                                Response::success(
                                    json!({
                                        "uri": params.uri,
                                        "content": encoded_content,
                                        "encoding": params.encoding,
                                    }),
                                    id,
                                )
                            }
                        }
                    }
                    Err(_) => {
                        // Create error response
                        Response::error(Error::resource_access_denied(&params.uri), id)
                    }
                }
            }
            Err(_) => {
                // Create error response
                Response::error(Error::resource_not_found(&params.uri), id)
            }
        }
    });

    // Register resource/set method
    let resource_manager_set = resource_manager.clone();
    handler.register_method("resources/set", move |request: &Request| -> Response {
        // Get ID for response
        let id = match &request.id {
            Some(id) => id.clone(),
            None => Id::Null,
        };

        // Parse params
        let params = match &request.params {
            Some(params) => match serde_json::from_value::<ResourceSetParams>(params.clone()) {
                Ok(params) => params,
                Err(_) => return Response::invalid_params(id),
            },
            None => return Response::invalid_params(id),
        };

        // Get resource manager
        let mut resource_manager = match resource_manager_set.lock() {
            Ok(manager) => manager,
            Err(_) => return Response::internal_error(id),
        };

        // Decode content based on encoding parameter
        let content = match params.encoding.as_str() {
            "base64" => match base64::decode(&params.content) {
                Ok(bytes) => bytes,
                Err(_) => return Response::error(Error::invalid_params(), id),
            },
            "utf8" | "utf-8" => params.content.into_bytes(),
            _ => return Response::error(Error::invalid_params(), id),
        };

        // Create or update resource
        match resource_manager.create_resource(&params.uri, AccessMode::Write) {
            Ok(mut resource) => {
                // Write resource content
                match resource.write(&content) {
                    Ok(_) => {
                        // Get resource metadata
                        match resource.metadata() {
                            Ok(metadata) => {
                                // Create response
                                Response::success(
                                    json!({
                                        "uri": params.uri,
                                        "metadata": metadata,
                                    }),
                                    id,
                                )
                            }
                            Err(_) => {
                                // Create response without metadata
                                Response::success(
                                    json!({
                                        "uri": params.uri,
                                        "success": true,
                                    }),
                                    id,
                                )
                            }
                        }
                    }
                    Err(_) => {
                        // Create error response
                        Response::error(Error::resource_access_denied(&params.uri), id)
                    }
                }
            }
            Err(_) => {
                // Create error response
                Response::error(Error::resource_access_denied(&params.uri), id)
            }
        }
    });

    // Register resource/delete method
    let resource_manager_delete = resource_manager;
    handler.register_method("resources/delete", move |request: &Request| -> Response {
        // Get ID for response
        let id = match &request.id {
            Some(id) => id.clone(),
            None => Id::Null,
        };

        // Parse params
        let params = match &request.params {
            Some(params) => match serde_json::from_value::<ResourceDeleteParams>(params.clone()) {
                Ok(params) => params,
                Err(_) => return Response::invalid_params(id),
            },
            None => return Response::invalid_params(id),
        };

        // Get resource manager
        let mut resource_manager = match resource_manager_delete.lock() {
            Ok(manager) => manager,
            Err(_) => return Response::internal_error(id),
        };

        // Delete resource
        match resource_manager.delete_resource(&params.uri) {
            Ok(_) => {
                // Create response
                Response::success(
                    json!({
                        "uri": params.uri,
                        "success": true,
                    }),
                    id,
                )
            }
            Err(_) => {
                // Create error response
                Response::error(Error::resource_not_found(&params.uri), id)
            }
        }
    });
}
