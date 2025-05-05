use mcp_core::commands::mcp::{McpCommand, ToolInfo, ToolProvider};
use mcp_core::{CommandResult, CommandStatus, SlashCommand};
use serde_json::json;

// A simple mock tool provider for testing
struct MockToolProvider {
    tools: Vec<ToolInfo>,
}

impl MockToolProvider {
    fn new() -> Self {
        let tools = vec![
            ToolInfo {
                id: "tool1".to_string(),
                name: "Tool One".to_string(),
                description: "A test tool".to_string(),
                category: "Test".to_string(),
                input_schema: json!({"type": "object"}),
                output_schema: json!({"type": "object"}),
            },
            ToolInfo {
                id: "tool2".to_string(),
                name: "Tool Two".to_string(),
                description: "Another test tool".to_string(),
                category: "Test".to_string(),
                input_schema: json!({"type": "object"}),
                output_schema: json!({"type": "object"}),
            },
        ];
        Self { tools }
    }
}

impl ToolProvider for MockToolProvider {
    fn get_tools(&self) -> Vec<ToolInfo> {
        self.tools.clone()
    }
    
    fn get_tool_details(&self, tool_id: &str) -> Option<ToolInfo> {
        self.tools.iter().find(|t| t.id == tool_id).cloned()
    }
}

#[test]
fn test_mcp_command_help() {
    let provider = MockToolProvider::new();
    let command = McpCommand::new(provider);
    
    // Test help command
    let result = command.execute(&["help"]);
    assert!(matches!(result.status, CommandStatus::Success));
    assert!(result.content.is_some());
    let content = result.content.unwrap();
    assert!(content.contains("MCP Debug Commands"));
}

#[test]
fn test_mcp_command_list() {
    let provider = MockToolProvider::new();
    let command = McpCommand::new(provider);
    
    // Test list command
    let result = command.execute(&["list"]);
    assert!(matches!(result.status, CommandStatus::Success));
    assert!(result.content.is_some());
    let content = result.content.unwrap();
    println!("LIST CONTENT: {}", content);
    assert!(content.contains("Available MCP Tools"));
    assert!(content.contains("tool1"));
    assert!(content.contains("tool2"));
}

#[test]
fn test_mcp_command_show_valid() {
    let provider = MockToolProvider::new();
    let command = McpCommand::new(provider);
    
    // Test show with valid tool
    let result = command.execute(&["show", "tool1"]);
    assert!(matches!(result.status, CommandStatus::Success));
    assert!(result.content.is_some());
    let content = result.content.unwrap();
    assert!(content.contains("Tool: Tool One"));
}

#[test]
fn test_mcp_command_show_invalid() {
    let provider = MockToolProvider::new();
    let command = McpCommand::new(provider);
    
    // Test show with invalid tool
    let result = command.execute(&["show", "invalid"]);
    assert!(matches!(result.status, CommandStatus::Error));
    assert!(result.error.is_some());
    let error = result.error.unwrap();
    assert!(error.contains("not found"));
}

#[test]
fn test_mcp_command_unknown() {
    let provider = MockToolProvider::new();
    let command = McpCommand::new(provider);
    
    // Test unknown command
    let result = command.execute(&["unknown"]);
    assert!(matches!(result.status, CommandStatus::Error));
    assert!(result.error.is_some());
    let error = result.error.unwrap();
    assert!(error.contains("Unknown MCP command"));
}