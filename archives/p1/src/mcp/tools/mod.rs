// MCP Tools Module
// Provides executable tools that can be invoked by Claude models

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::mcp::protocol::error::Error;
use crate::mcp::resources::{AccessMode, ResourceManager};

mod coding;
pub mod methods;
mod search;
mod shell;

// Re-export tool implementations
pub use coding::CodingTool;
pub use search::SearchTool;
pub use shell::ShellTool;

/// Tool metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    /// Tool ID (unique identifier)
    pub id: String,

    /// Tool name (human-readable)
    pub name: String,

    /// Tool description
    pub description: String,

    /// Tool category
    pub category: ToolCategory,

    /// Input schema (JSON Schema)
    pub input_schema: Value,

    /// Output schema (JSON Schema)
    pub output_schema: Value,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Tool category
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    /// Shell command execution
    Shell,

    /// File search
    Search,

    /// Code related tools
    Coding,

    /// Other tool category
    Other(String),
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool ID
    pub tool_id: String,

    /// Execution status
    pub status: ToolStatus,

    /// Execution output
    pub output: Value,

    /// Error message (if status is Error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Tool execution status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    /// Tool executed successfully
    Success,

    /// Tool execution failed
    Error,
}

/// Tool trait - all tools must implement this
pub trait Tool: Send + Sync {
    /// Get tool metadata
    fn metadata(&self) -> ToolMetadata;

    /// Execute tool with input parameters
    fn execute(&self, params: Value, resource_manager: &ResourceManager) -> Result<ToolResult>;
}

/// Tool manager
pub struct ToolManager {
    /// Base directory for file operations
    base_dir: PathBuf,

    /// Resource manager
    resource_manager: Arc<Mutex<ResourceManager>>,

    /// Registered tools
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolManager {
    /// Create a new tool manager
    pub fn new(base_dir: PathBuf, resource_manager: Arc<Mutex<ResourceManager>>) -> Self {
        Self {
            base_dir,
            resource_manager,
            tools: HashMap::new(),
        }
    }

    /// Register default tools
    pub fn register_default_tools(&mut self) -> Result<()> {
        // Register shell tool
        let shell_tool = ShellTool::new(&self.base_dir)?;
        self.register_tool(Box::new(shell_tool));

        // Register search tool
        let search_tool = SearchTool::new(&self.base_dir)?;
        self.register_tool(Box::new(search_tool));

        // Register coding tool
        let coding_tool = CodingTool::new(&self.base_dir)?;
        self.register_tool(Box::new(coding_tool));

        Ok(())
    }

    /// Register a tool
    pub fn register_tool(&mut self, tool: Box<dyn Tool>) {
        let metadata = tool.metadata();
        self.tools.insert(metadata.id.clone(), tool);
    }

    /// Deregister a tool
    pub fn deregister_tool(&mut self, tool_id: &str) -> bool {
        self.tools.remove(tool_id).is_some()
    }

    /// Get all registered tools
    pub fn get_tools(&self) -> Vec<ToolMetadata> {
        self.tools.values().map(|tool| tool.metadata()).collect()
    }

    /// Get a tool by ID
    pub fn get_tool(&self, tool_id: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(tool_id)
    }

    /// Execute a tool by ID
    pub fn execute_tool(&self, tool_id: &str, params: Value) -> Result<ToolResult> {
        // Get tool
        let tool = self
            .get_tool(tool_id)
            .ok_or_else(|| anyhow!("Tool not found: {}", tool_id))?;

        // Get resource manager
        let resource_manager = self
            .resource_manager
            .lock()
            .map_err(|_| anyhow!("Failed to lock resource manager"))?;

        // Execute tool
        tool.execute(params, &resource_manager)
            .context(format!("Failed to execute tool: {}", tool_id))
    }
}
