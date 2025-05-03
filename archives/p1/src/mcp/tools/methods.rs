use anyhow::Result as AnyhowResult;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

use super::{ToolManager, ToolResult};
use crate::mcp::protocol::error::Error;
use crate::mcp::protocol::message::{Id, Request, Response};

/// Handle a tool-related JSON-RPC request
pub fn handle_request(
    request: &Request,
    tool_manager: Arc<Mutex<ToolManager>>,
) -> Result<Response, Error> {
    match request.method.as_str() {
        "tools.list" => handle_list_tools(request, tool_manager),
        "tools.execute" => handle_execute_tool(request, tool_manager),
        _ => Err(Error::method_not_found()),
    }
}

/// Handle the tools.list method
pub fn handle_list_tools(
    request: &Request,
    tool_manager: Arc<Mutex<ToolManager>>,
) -> Result<Response, Error> {
    // Lock the tool manager
    let tool_manager = tool_manager.lock().map_err(|_| Error::internal_error())?;

    // Get all tools
    let tools = tool_manager.get_tools();

    // Create response
    Ok(Response::success(
        json!(tools),
        request.id.clone().unwrap_or(Id::Null),
    ))
}

/// Handle the tools.execute method
pub fn handle_execute_tool(
    request: &Request,
    tool_manager: Arc<Mutex<ToolManager>>,
) -> Result<Response, Error> {
    // Extract parameters
    let params = request
        .params
        .as_ref()
        .ok_or_else(|| Error::invalid_params())?;

    // Extract tool_id
    let tool_id = params
        .get("tool_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::invalid_params())?;

    // Extract tool_params
    let tool_params = params
        .get("params")
        .ok_or_else(|| Error::invalid_params())?
        .clone();

    // Lock the tool manager
    let tool_manager = tool_manager.lock().map_err(|_| Error::internal_error())?;

    // Execute the tool
    let result = tool_manager
        .execute_tool(tool_id, tool_params)
        .map_err(|err| Error::new(1000, format!("Tool execution failed: {}", err), None))?;

    // Create response
    let result_value = serde_json::to_value(result).map_err(|_| Error::internal_error())?;

    Ok(Response::success(
        result_value,
        request.id.clone().unwrap_or(Id::Null),
    ))
}
