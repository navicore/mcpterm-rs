use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub mod diff;
pub mod filesystem;
pub mod registry;
pub mod search;
pub mod shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    Shell,
    Filesystem,
    Search,
    Utility,
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
    
    /// Get a list of all registered tools
    pub fn get_tools(&self) -> Vec<ToolMetadata> {
        self.tools
            .values()
            .map(|tool| tool.metadata())
            .collect()
    }
    
    /// Generate documentation for all registered tools
    pub fn generate_tool_documentation(&self) -> String {
        let mut doc = String::from("Available tools:\n\n");
        
        for (i, tool) in self.tools.values().enumerate() {
            let metadata = tool.metadata();
            
            // Add tool name and description
            doc.push_str(&format!("{}. \"{}\": {}\n", i + 1, metadata.id, metadata.description));
            
            // Add parameters documentation
            doc.push_str("   Parameters: {\n");
            
            // Extract parameter information from the schema
            if let Some(props) = metadata.input_schema.get("properties") {
                if let Some(props_obj) = props.as_object() {
                    for (param_name, param_schema) in props_obj {
                        let param_type = param_schema
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("any");
                            
                        let description = param_schema
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                            
                        let required = metadata
                            .input_schema
                            .get("required")
                            .and_then(|r| r.as_array())
                            .map(|arr| arr.iter().any(|v| v.as_str() == Some(param_name)))
                            .unwrap_or(false);
                            
                        if required {
                            doc.push_str(&format!("     \"{}\": \"{}\"", param_name, param_type));
                            if !description.is_empty() {
                                doc.push_str(&format!(",           // {}", description));
                            }
                            doc.push_str("\n");
                        } else {
                            doc.push_str(&format!("     \"{}\": \"{}\"", param_name, param_type));
                            doc.push_str(&format!(",           // Optional: {}", description));
                            doc.push_str("\n");
                        }
                    }
                }
            }
            
            doc.push_str("   }\n\n");
        }
        
        doc
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
    use serde_json::json;

    #[test]
    fn test_tool_manager_creation() {
        let manager = ToolManager::new();
        assert_eq!(manager.tools.len(), 0);
    }
    
    // A simple mock tool for testing
    struct MockTool {
        metadata: ToolMetadata,
    }
    
    #[async_trait]
    impl Tool for MockTool {
        fn metadata(&self) -> ToolMetadata {
            self.metadata.clone()
        }
        
        async fn execute(&self, _params: Value) -> Result<ToolResult> {
            Ok(ToolResult {
                tool_id: self.metadata.id.clone(),
                status: ToolStatus::Success,
                output: json!({"result": "mock result"}),
                error: None,
            })
        }
    }
    
    #[test]
    fn test_generate_tool_documentation() {
        let mut manager = ToolManager::new();
        
        // Create and register a mock tool
        let mock_tool = MockTool {
            metadata: ToolMetadata {
                id: "mock_tool".to_string(),
                name: "Mock Tool".to_string(),
                description: "A mock tool for testing".to_string(),
                category: ToolCategory::General,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "required_param": {
                            "type": "string",
                            "description": "A required parameter"
                        },
                        "optional_param": {
                            "type": "number",
                            "description": "An optional parameter"
                        }
                    },
                    "required": ["required_param"]
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "result": {
                            "type": "string"
                        }
                    }
                }),
            },
        };
        
        manager.register_tool(Box::new(mock_tool));
        
        // Generate documentation
        let docs = manager.generate_tool_documentation();
        
        // Verify the documentation contains expected elements
        assert!(docs.contains("\"mock_tool\""));
        assert!(docs.contains("A mock tool for testing"));
        assert!(docs.contains("\"required_param\": \"string\""));
        assert!(docs.contains("\"optional_param\": \"number\""));
        assert!(docs.contains("A required parameter"));
        assert!(docs.contains("Optional: An optional parameter"));
    }
    
    #[test]
    fn test_generate_tool_documentation_multiple_tools() {
        let mut manager = ToolManager::new();
        
        // Create and register two mock tools
        let tool1 = MockTool {
            metadata: ToolMetadata {
                id: "tool1".to_string(),
                name: "Tool One".to_string(),
                description: "First mock tool".to_string(),
                category: ToolCategory::General,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "param1": {
                            "type": "string",
                            "description": "Parameter 1"
                        }
                    },
                    "required": ["param1"]
                }),
                output_schema: json!({}),
            },
        };
        
        let tool2 = MockTool {
            metadata: ToolMetadata {
                id: "tool2".to_string(),
                name: "Tool Two".to_string(),
                description: "Second mock tool".to_string(),
                category: ToolCategory::Search,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "param2": {
                            "type": "boolean",
                            "description": "Parameter 2"
                        }
                    },
                    "required": []
                }),
                output_schema: json!({}),
            },
        };
        
        manager.register_tool(Box::new(tool1));
        manager.register_tool(Box::new(tool2));
        
        // Generate documentation
        let docs = manager.generate_tool_documentation();
        
        // Verify the documentation contains both tools
        assert!(docs.contains("\"tool1\""));
        assert!(docs.contains("First mock tool"));
        assert!(docs.contains("\"param1\": \"string\""));
        
        assert!(docs.contains("\"tool2\""));
        assert!(docs.contains("Second mock tool"));
        assert!(docs.contains("\"param2\": \"boolean\""));
        assert!(docs.contains("Optional: Parameter 2"));
        
        // Should have numbered the tools
        assert!(docs.contains("1. \"tool"));  // Either tool could be first
        assert!(docs.contains("2. \"tool"));  // And the other would be second
    }
}
