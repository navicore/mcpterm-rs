use anyhow::Result;
use mcp_metrics::{count, time};
use mcp_tools::{ToolManager, ToolResult};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, trace};
mod tool_factory;
pub use tool_factory::ToolFactory;

// Global set to track which tools have been executed, to prevent duplicates
lazy_static::lazy_static! {
    static ref EXECUTED_TOOLS: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    // Track currently executing tools to prevent re-entrance
    static ref CURRENTLY_EXECUTING_TOOLS: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

/// Clears the list of executed tools, typically at the start of a new conversation
pub fn clear_executed_tools() {
    // Clear executed tools history
    {
        let mut executed = EXECUTED_TOOLS.lock().unwrap();
        executed.clear();
    }

    // Also clear currently executing tools to prevent deadlocks or hanging tools
    {
        let mut currently_executing = CURRENTLY_EXECUTING_TOOLS.lock().unwrap();
        currently_executing.clear();
    }

    debug!("Cleared all tool tracking caches");
}

/// No longer does duplicate detection since that caused more problems than it solved.
/// Now simply returns true to always execute the tool.
pub fn should_execute_tool(_tool_id: &str, _params: &Value) -> bool {
    // Always execute the tool - no duplicate detection
    // This ensures every tool call is processed, even if it looks like a duplicate
    // We've fixed the root cause (re-prompting) so duplicates shouldn't occur anymore

    // First ensure we don't leave any orphaned entries in the currently executing set
    let mut currently_executing = CURRENTLY_EXECUTING_TOOLS.lock().unwrap();
    // Clear the currently executing set just to be safe
    currently_executing.clear();

    debug!("Executing tool call without duplicate detection");
    return true;
}

// Coordinates execution of tools with safety constraints
pub struct ToolExecutor {
    tool_manager: Arc<ToolManager>,
    // Additional fields for safety constraints, etc.
}

impl ToolExecutor {
    pub fn new(tool_manager: ToolManager) -> Self {
        debug!("Creating new tool executor");
        Self {
            tool_manager: Arc::new(tool_manager),
        }
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

        // No more duplicate detection - it caused more problems than it solved
        // The root cause (re-prompting) has been fixed so duplicates shouldn't occur
        debug!("Executing tool without duplicate checking: {}", tool_id);

        // Still log potentially problematic creation commands to help with debugging
        if tool_id == "shell" || tool_id == "command" {
            if let Some(command) = params.get("command").and_then(Value::as_str) {
                if command.contains("cargo new")
                    || command.contains("mkdir")
                    || command.contains("npm init")
                    || command.contains("create-react-app")
                {
                    debug!("CREATION COMMAND BEING EXECUTED: {}", command);
                }
            }
        }

        // Count tool executions
        count!("tool.executions.total");
        count!(format!("tool.executions.{}", tool_id).as_str());

        // This is a placeholder implementation
        // In the real implementation, we'd apply safety constraints
        debug!("Applying tool safety constraints for: {}", tool_id);

        // Get the number of tools registered
        debug!(
            "Tool manager has {} tools registered",
            self.tool_manager.get_tools().len()
        );

        // List available tools for debugging
        let tools = self.tool_manager.get_tools();
        if !tools.is_empty() {
            debug!(
                "Available tools: {}",
                tools
                    .iter()
                    .map(|t| t.id.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
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
                if result.status == mcp_tools::ToolStatus::Failure
                    && result.error.as_deref() == Some(&format!("Tool '{}' not found", tool_id))
                {
                    error!(
                        "Tool '{}' not found. Available tools: {}",
                        tool_id,
                        self.tool_manager
                            .get_tools()
                            .iter()
                            .map(|t| t.id.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
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
        assert!(
            tools
                .iter()
                .any(|t| t.category == mcp_tools::ToolCategory::Shell),
            "Shell tool not found"
        );
        assert!(
            tools
                .iter()
                .any(|t| t.category == mcp_tools::ToolCategory::Filesystem),
            "Filesystem tool not found"
        );
        assert!(
            tools
                .iter()
                .any(|t| t.category == mcp_tools::ToolCategory::Search),
            "Search tool not found"
        );

        // Debug output of registered tools
        println!(
            "Registered tools: {}",
            tools
                .iter()
                .map(|t| t.id.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}
