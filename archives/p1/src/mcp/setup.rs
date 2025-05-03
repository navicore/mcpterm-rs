use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use crate::mcp::handler::McpHandler;
use crate::mcp::protocol::message::Request;
use crate::mcp::resources::ResourceManager;
use crate::mcp::tools::{methods, ToolManager};

/// Initialize the MCP handler with all resources and tools
pub fn init_mcp(base_dir: PathBuf) -> Result<McpHandler> {
    // Create MCP handler
    let handler = McpHandler::new();

    // Create resource manager
    let resource_manager = ResourceManager::new(base_dir.clone())?;
    let resource_manager = Arc::new(Mutex::new(resource_manager));

    // Create tool manager
    let mut tool_manager = ToolManager::new(base_dir, resource_manager.clone());

    // Register default tools
    tool_manager.register_default_tools()?;

    // Wrap tool manager in Arc<Mutex<>>
    let tool_manager = Arc::new(Mutex::new(tool_manager));

    // Register tool methods
    let tool_manager_clone = Arc::clone(&tool_manager);
    handler.register_method(
        "tools.list",
        move |request| match methods::handle_list_tools(request, Arc::clone(&tool_manager_clone)) {
            Ok(response) => response,
            Err(error) => {
                let id = request
                    .id
                    .clone()
                    .unwrap_or(crate::mcp::protocol::message::Id::Null);
                crate::mcp::protocol::message::Response::error(error, id)
            }
        },
    );

    let tool_manager_clone = Arc::clone(&tool_manager);
    handler.register_method(
        "tools.execute",
        move |request| match methods::handle_execute_tool(request, Arc::clone(&tool_manager_clone))
        {
            Ok(response) => response,
            Err(error) => {
                let id = request
                    .id
                    .clone()
                    .unwrap_or(crate::mcp::protocol::message::Id::Null);
                crate::mcp::protocol::message::Response::error(error, id)
            }
        },
    );

    // Return the initialized handler
    Ok(handler)
}
