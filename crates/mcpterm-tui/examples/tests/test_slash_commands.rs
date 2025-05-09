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

fn main() {
    // Create a mock tool provider
    let provider = MockToolProvider::new();
    
    // Create the MCP command handler
    let command = McpCommand::new(provider);
    
    println!("====== Testing /mcp command ======");
    println!("\nTest: /mcp (no args)");
    println!("{:?}", command.execute(&[]));
    
    println!("\nTest: /mcp help");
    println!("{:?}", command.execute(&["help"]));
    
    println!("\nTest: /mcp list");
    let list_result = command.execute(&["list"]);
    println!("Status: {:?}", list_result.status);
    if let Some(content) = list_result.content {
        println!("{}", content);
    }
    
    println!("\nTest: /mcp show tool1");
    let show_result = command.execute(&["show", "tool1"]);
    println!("Status: {:?}", show_result.status);
    if let Some(content) = show_result.content {
        println!("{}", content);
    }
    
    println!("\nTest: /mcp show invalid");
    let invalid_result = command.execute(&["show", "invalid"]);
    println!("Status: {:?}", invalid_result.status);
    if let Some(error) = invalid_result.error {
        println!("Error: {}", error);
    }
    
    println!("\nTest: /mcp unknown");
    let unknown_result = command.execute(&["unknown"]);
    println!("Status: {:?}", unknown_result.status);
    if let Some(error) = unknown_result.error {
        println!("Error: {}", error);
    }
    
    println!("\n====== Tests complete ======");
}