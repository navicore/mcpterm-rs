use super::{CommandResult, SlashCommand};
use serde_json::to_string_pretty;

/// MCP slash command handler
pub struct McpCommand<T> {
    tool_provider: T,
}

/// A trait for types that can provide tool information
pub trait ToolProvider: Send + Sync {
    /// Get a list of all available tools with metadata
    fn get_tools(&self) -> Vec<ToolInfo>;
    
    /// Get details for a specific tool by ID
    fn get_tool_details(&self, tool_id: &str) -> Option<ToolInfo>;
}

/// Simplified tool information
#[derive(Clone, Debug)]
pub struct ToolInfo {
    /// The unique ID of the tool
    pub id: String,
    
    /// The display name of the tool
    pub name: String,
    
    /// The description of the tool
    pub description: String,
    
    /// The category/type of the tool
    pub category: String,
    
    /// Input schema as JSON Value
    pub input_schema: serde_json::Value,
    
    /// Output schema as JSON Value
    pub output_schema: serde_json::Value,
}

impl<T: ToolProvider> McpCommand<T> {
    /// Create a new MCP command handler
    pub fn new(tool_provider: T) -> Self {
        Self { tool_provider }
    }
    
    /// Handle the 'list' subcommand
    fn handle_list(&self) -> CommandResult {
        let tools = self.tool_provider.get_tools();
        
        let mut content = "\n=== Available MCP Tools ===\n".to_string();
        for (i, tool) in tools.iter().enumerate() {
            content.push_str(&format!("{}. {} - {}\n", i + 1, tool.id, tool.description));
        }
        content.push_str("\nUse '/mcp show <tool_id>' for detailed information about a specific tool.");
        
        CommandResult::success(&content)
    }
    
    /// Handle the 'show' subcommand
    fn handle_show(&self, args: &[&str]) -> CommandResult {
        if args.is_empty() {
            return CommandResult::error("Missing tool ID. Usage: /mcp show <tool_id>");
        }
        
        let tool_id = args[0];
        match self.tool_provider.get_tool_details(tool_id) {
            Some(details) => {
                let mut content = format!("\n=== Tool: {} ===\n", details.name);
                content.push_str(&format!("ID: {}\n", details.id));
                content.push_str(&format!("Description: {}\n", details.description));
                content.push_str(&format!("Category: {}\n", details.category));
                
                content.push_str("\nInput Schema:\n");
                if let Ok(schema_str) = to_string_pretty(&details.input_schema) {
                    content.push_str(&schema_str);
                } else {
                    content.push_str("  (Error formatting input schema)");
                }
                
                content.push_str("\n\nOutput Schema:\n");
                if let Ok(schema_str) = to_string_pretty(&details.output_schema) {
                    content.push_str(&schema_str);
                } else {
                    content.push_str("  (Error formatting output schema)");
                }
                
                CommandResult::success(&content)
            }
            None => {
                CommandResult::error(&format!(
                    "Tool '{}' not found. Use '/mcp list' to see available tools.",
                    tool_id
                ))
            }
        }
    }
    
    /// Handle the 'version' subcommand
    fn handle_version(&self) -> CommandResult {
        let version = env!("CARGO_PKG_VERSION");
        CommandResult::success(&format!("MCP Client Version: {}", version))
    }
}

impl<T: ToolProvider> SlashCommand for McpCommand<T> {
    fn name(&self) -> &str {
        "mcp"
    }
    
    fn description(&self) -> &str {
        "MCP tool debugging commands"
    }
    
    fn help(&self) -> &str {
        r#"
=== MCP Debug Commands ===
/mcp help            - Show this help message
/mcp list            - List all available tools
/mcp show <tool_id>  - Show detailed information for a specific tool
/mcp version         - Show MCP client version
"#
    }
    
    fn execute(&self, args: &[&str]) -> CommandResult {
        if args.is_empty() {
            // No subcommand, show help
            return CommandResult::success(self.help());
        }
        
        match args[0] {
            "help" => CommandResult::success(self.help()),
            "list" => self.handle_list(),
            "show" => self.handle_show(&args[1..]),
            "version" => self.handle_version(),
            unknown => CommandResult::error(&format!(
                "Unknown MCP command: {}. Use '/mcp help' for available commands.",
                unknown
            )),
        }
    }
}