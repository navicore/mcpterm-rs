use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub mod filesystem;
pub mod registry;
pub mod search;
pub mod shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    Shell,
    Filesystem,
    Search,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    pub input_schema: Value,
    pub output_schema: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolStatus {
    Success,
    Failure,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_id: String,
    pub status: ToolStatus,
    pub output: Value,
    pub error: Option<String>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn metadata(&self) -> ToolMetadata;
    async fn execute(&self, params: Value) -> Result<ToolResult>;
}

pub struct ToolManager {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolManager {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn Tool>) {
        let metadata = tool.metadata();
        self.tools.insert(metadata.id.clone(), tool);
    }

    pub async fn execute_tool(&self, tool_id: &str, params: Value) -> Result<ToolResult> {
        // This is a placeholder implementation
        if let Some(tool) = self.tools.get(tool_id) {
            tool.execute(params).await
        } else {
            Ok(ToolResult {
                tool_id: tool_id.to_string(),
                status: ToolStatus::Failure,
                output: Value::Null,
                error: Some(format!("Tool '{}' not found", tool_id)),
            })
        }
    }
}

impl Default for ToolManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_manager_creation() {
        let manager = ToolManager::new();
        assert_eq!(manager.tools.len(), 0);
    }
}
