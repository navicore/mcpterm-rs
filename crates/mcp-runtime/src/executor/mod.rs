use anyhow::Result;
use mcp_metrics::{count, time};
use mcp_tools::{ToolManager, ToolResult};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, trace};

// Import the tool factory
mod tool_factory;
pub use tool_factory::ToolFactory;

// Coordinates execution of tools with safety constraints
pub struct ToolExecutor {
    tool_manager: Arc<ToolManager>,
    // Additional fields for safety constraints, etc.
}

impl ToolExecutor {
    pub fn new(tool_manager: ToolManager) -> Self {
        debug!("Creating new tool executor");
        Self { tool_manager: Arc::new(tool_manager) }
    }

    pub fn with_shared_manager(tool_manager: Arc<ToolManager>) -> Self {
        debug!("Creating new tool executor with shared tool manager");
        Self { tool_manager }
    }

    /// Create a ToolExecutor with standard configured tools using default settings
    pub fn new_with_standard_tools() -> Self {
        debug!("Creating new tool executor with standard tools");
        ToolFactory::create_executor()
    }

    pub async fn execute_tool(&self, tool_id: &str, params: Value) -> Result<ToolResult> {
        debug!("Executing tool: {}", tool_id);
        trace!("Tool parameters: {}", params);

        // Count tool executions
        count!("tool.executions.total");
        count!(format!("tool.executions.{}", tool_id).as_str());

        // This is a placeholder implementation
        // In the real implementation, we'd apply safety constraints
        debug!("Applying tool safety constraints for: {}", tool_id);

        // Get the number of tools registered
        debug!("Tool manager has {} tools registered", self.tool_manager.get_tools().len());

        // List available tools for debugging
        let tools = self.tool_manager.get_tools();
        if !tools.is_empty() {
            debug!("Available tools: {}", tools.iter().map(|t| t.id.clone()).collect::<Vec<_>>().join(", "));
        } else {
            debug!("No tools are registered in the tool manager!");
        }

        // Time the tool execution
        let result = time!(format!("tool.execution_time.{}", tool_id).as_str(), {
            self.tool_manager
                .execute_tool(tool_id, params.clone())
                .await
        });

        match result {
            Ok(result) => {
                if result.status == mcp_tools::ToolStatus::Failure && result.error.as_deref() == Some(&format!("Tool '{}' not found", tool_id)) {
                    error!("Tool '{}' not found. Available tools: {}", tool_id,
                           self.tool_manager.get_tools().iter().map(|t| t.id.clone()).collect::<Vec<_>>().join(", "));
                } else {
                    debug!("Tool {} executed successfully", tool_id);
                    trace!("Tool result: {:?}", result);
                }
                count!("tool.executions.success");
                Ok(result)
            }
            Err(err) => {
                error!("Tool {} execution failed: {}", tool_id, err);
                count!("tool.executions.failure");
                count!(format!("tool.failures.{}", tool_id).as_str());
                Err(err)
            }
        }
    }

    /// Get access to the underlying tool manager (useful for testing)
    pub fn get_tool_manager(&self) -> &Arc<ToolManager> {
        &self.tool_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let tool_manager = ToolManager::new();
        let _executor = ToolExecutor::new(tool_manager);
    }

    #[test]
    fn test_executor_standard_tools() {
        let executor = ToolExecutor::new_with_standard_tools();
        let tools = executor.get_tool_manager().get_tools();
        assert!(!tools.is_empty());

        // Verify at least one of each tool type is registered
        assert!(tools.iter().any(|t| t.category == mcp_tools::ToolCategory::Shell), "Shell tool not found");
        assert!(tools.iter().any(|t| t.category == mcp_tools::ToolCategory::Filesystem), "Filesystem tool not found");
        assert!(tools.iter().any(|t| t.category == mcp_tools::ToolCategory::Search), "Search tool not found");

        // Debug output of registered tools
        println!("Registered tools: {}", tools.iter().map(|t| t.id.clone()).collect::<Vec<_>>().join(", "));
    }
}
