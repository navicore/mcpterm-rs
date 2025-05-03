use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::{Agent, BedrockAgentSync};
use crate::config::{Config, ModelConfig};
use crate::mcp::{
    debug_log, init_mcp, McpHandler, Request, ResourceManager, Tool, ToolManager, ToolResult, ToolStatus,
};

/// MCP Agent for processing messages with the MCP protocol
#[derive(Clone)]
pub struct McpAgent {
    /// Base directory for file operations
    base_dir: PathBuf,
    
    /// MCP handler for processing requests
    mcp_handler: McpHandler,
    
    /// Tool manager for managing available tools
    tool_manager: Arc<Mutex<ToolManager>>,
    
    /// Underlying LLM agent for non-MCP messages
    llm_agent: BedrockAgentSync,
}

impl McpAgent {
    /// Direct shell execution with improved reliability and enhanced debugging
    fn direct_shell_execute(&self, cmd: &str) -> Option<String> {
        debug_log(&format!("Attempting direct shell execution with enhanced reliability: {}", cmd));
        
        // First create a working directory to ensure command has a valid context
        let working_dir = if self.base_dir.exists() {
            debug_log(&format!("Using base directory: {}", self.base_dir.display()));
            Some(self.base_dir.clone())
        } else {
            debug_log("Base directory does not exist, falling back to temp directory");
            Some(std::env::temp_dir())
        };
        
        // Log working directory for debugging
        if let Some(dir) = &working_dir {
            debug_log(&format!("Using working directory: {}", dir.display()));
        }
        
        // Use std::process::Command for direct execution
        let mut command = if cfg!(target_os = "windows") {
            let mut c = std::process::Command::new("cmd");
            c.args(&["/C", cmd]);
            // Set working directory if available
            if let Some(dir) = &working_dir {
                c.current_dir(dir);
            }
            c
        } else {
            let mut c = std::process::Command::new("sh");
            c.args(&["-c", cmd]);
            // Set working directory if available
            if let Some(dir) = &working_dir {
                c.current_dir(dir);
            }
            c
        };
        
        // Log the exact command being executed with all arguments
        let cmd_str = format!("{:?}", command);
        debug_log(&format!("Executing command: {}", cmd_str));
        
        // Execute command with timeout protection
        let (tx, rx) = std::sync::mpsc::channel();
        let _cmd_thread = std::thread::spawn(move || {
            match command.output() {
                Ok(output) => {
                    let _ = tx.send(Ok(output));
                },
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            }
        });
        
        // Wait for completion with timeout (120 seconds)
        let timeout = std::time::Duration::from_secs(120);
        let start = std::time::Instant::now();
        
        let output_result = loop {
            match rx.try_recv() {
                Ok(result) => break Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if start.elapsed() > timeout {
                        debug_log(&format!("Command execution timed out after {:?}", timeout));
                        break None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                },
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    debug_log("Command execution thread crashed");
                    break None;
                }
            }
        };
        
        // Process the result
        match output_result {
            Some(Ok(output)) => {
                let mut result = String::new();
                
                // Command executed - include PID for debugging
                result.push_str(&format!("Command: {}\n", cmd));
                result.push_str(&format!("Exit Code: {}\n\n", output.status.code().unwrap_or(-1)));
                
                // Always include stdout, even if empty (for debugging)
                let stdout = String::from_utf8_lossy(&output.stdout);
                result.push_str(&format!("STDOUT:\n{}\n", stdout));
                
                // Always include stderr, even if empty (for debugging)
                let stderr = String::from_utf8_lossy(&output.stderr);
                result.push_str(&format!("STDERR:\n{}\n", stderr));
                
                // Log success with output details
                debug_log(&format!("Direct execution succeeded with exit code: {}", 
                    output.status.code().unwrap_or(-1)));
                debug_log(&format!("Command stdout length: {}", stdout.len()));
                debug_log(&format!("Command stderr length: {}", stderr.len()));
                
                Some(result)
            },
            Some(Err(e)) => {
                // Command instantiation/execution failed
                let error_msg = format!("Direct execution failed to start: {}", e);
                debug_log(&error_msg);
                
                // Return the error as part of the result so the user sees it
                Some(format!("Command: {}\nExecution Error: {}\n", cmd, error_msg))
            },
            None => {
                // Timeout or thread crash
                let error_msg = "Command execution timed out or thread crashed";
                debug_log(error_msg);
                
                // Return the timeout as part of the result so the user sees it
                Some(format!("Command: {}\nExecution Error: {}\n", cmd, error_msg))
            }
        }
    }
    /// Execute an MCP command and return the result
    fn execute_mcp_command(&self, command: &str) -> String {
        // TEMPORARY DEBUG: Log execution start
        debug_log(&format!("Starting to execute MCP command: {}", command));
        
        // Split the command to determine what type it is
        let parts: Vec<&str> = command.splitn(3, ' ').collect();
        
        if parts.len() < 2 {
            debug_log(&format!("Invalid MCP command format: {}", command));
            return format!("Invalid MCP command format: {}", command);
        }
        
        // Check what kind of command it is (help, tools, shell, search, write)
        let result = match parts[1] {
            "help" => {
                // Create a help JSON-RPC request
                let help_request = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "mcp.help"
                });
                
                // Process the help request
                match self.process_mcp_request(&help_request.to_string()) {
                    Ok(response) => self.format_response(&response),
                    Err(err) => format!("Error getting MCP help: {}", err),
                }
            },
            "tools" | "list" => {
                // Create a tools.list JSON-RPC request
                let list_request = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools.list"
                });
                
                // Process the tools.list request
                match self.process_mcp_request(&list_request.to_string()) {
                    Ok(response) => self.format_response(&response),
                    Err(err) => format!("Error listing MCP tools: {}", err),
                }
            },
            "shell" => {
                if parts.len() < 3 {
                    debug_log("Shell command is missing");
                    return "Shell command is missing. Usage: mcp shell <command>".to_string();
                }
                
                let shell_command = parts[2];
                debug_log(&format!("Processing shell command: {}", shell_command));
                
                // CRITICAL FIX: Try direct execution FIRST and ALWAYS USE THE RESULT
                // This ensures commands always run even if MCP protocol fails
                debug_log(&format!("Attempting direct shell execution for command: {}", shell_command));
                
                // Always use direct execution as the primary method
                if let Some(direct_result) = self.direct_shell_execute(shell_command) {
                    debug_log(&format!("Direct shell execution succeeded: {}", shell_command));
                    // Return the direct execution result immediately
                    return direct_result;
                }
                
                // If direct execution fails, log the failure
                debug_log(&format!("Direct shell execution failed for command: {}", shell_command));
                
                // As a final fallback, try the original MCP protocol
                debug_log("Falling back to MCP protocol for shell command");
                
                // Create a tools.execute JSON-RPC request for the shell tool
                let shell_request = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools.execute",
                    "params": {
                        "tool_id": "shell",
                        "params": {
                            "command": shell_command,
                            "timeout": 120 // Increased timeout to 120 seconds
                        }
                    }
                });
                
                // Process the shell request
                match self.process_mcp_request(&shell_request.to_string()) {
                    Ok(response) => self.format_response(&response),
                    Err(err) => format!("Error executing shell command (both direct and MCP methods failed): {}", err),
                }
            },
            "search" => {
                if parts.len() < 3 {
                    return "Search pattern is missing. Usage: mcp search <pattern>".to_string();
                }
                
                let pattern = parts[2];
                
                // Create a tools.execute JSON-RPC request for the search tool
                let search_request = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools.execute",
                    "params": {
                        "tool_id": "search",
                        "params": {
                            "search_type": "content",
                            "pattern": pattern,
                            "max_results": 20
                        }
                    }
                });
                
                // Process the search request
                match self.process_mcp_request(&search_request.to_string()) {
                    Ok(response) => self.format_response(&response),
                    Err(err) => format!("Error searching for content: {}", err),
                }
            },
            "write" => {
                if parts.len() < 3 {
                    return "File path and content are missing. Usage: mcp write <file_path> <content>".to_string();
                }
                
                let cmd_parts: Vec<&str> = parts[2].splitn(2, ' ').collect();
                if cmd_parts.len() < 2 {
                    return "Both file path and content are required. Usage: mcp write <file_path> <content>".to_string();
                }
                
                let file_path = cmd_parts[0];
                let content = cmd_parts[1];
                
                // Create a tools.execute JSON-RPC request for the coding tool
                let write_request = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools.execute",
                    "params": {
                        "tool_id": "coding",
                        "params": {
                            "operation": "replace",
                            "file_path": file_path,
                            "old_text": "",
                            "new_text": content,
                            "expected_replacements": 0
                        }
                    }
                });
                
                // Process the write request
                match self.process_mcp_request(&write_request.to_string()) {
                    Ok(response) => self.format_response(&response),
                    Err(err) => format!("Error writing to file: {}", err),
                }
            },
            _ => {
                debug_log(&format!("Unknown MCP command type: {}", parts[1]));
                format!("Unknown MCP command: {}", parts[1])
            },
        };
        
        // TEMPORARY DEBUG: Log execution result (abbreviated if too long)
        if result.len() > 500 {
            debug_log(&format!("Finished executing MCP command with result length {}: {}...", 
                     result.len(), &result[..500]));
        } else {
            debug_log(&format!("Finished executing MCP command with result: {}", result));
        }
        
        result
    }
    
    /// Extract all MCP commands from markdown code blocks or text
    fn extract_commands_from_text(&self, input: &str) -> Vec<String> {
        let mut commands = Vec::new();
        
        // Extract commands from code blocks
        self.extract_commands_from_code_blocks(input, &mut commands);
        
        // Extract inline commands like `mcp shell ls`
        self.extract_inline_commands(input, &mut commands);
        
        // Extract direct commands (not in code blocks)
        self.extract_direct_commands(input, &mut commands);
        
        commands
    }
    
    /// Extract commands from markdown code blocks
    fn extract_commands_from_code_blocks(&self, input: &str, commands: &mut Vec<String>) {
        // Process each line to find code blocks
        let lines = input.lines().collect::<Vec<_>>();
        let mut i = 0;
        
        while i < lines.len() {
            let line = lines[i].trim();
            
            // Check for single-line code blocks (```mcp command```)
            if line.starts_with("```") && line.ends_with("```") && line.contains("mcp ") {
                // This is a single-line code block containing an MCP command
                // Extract the command by removing the code fence markers
                let content = line.trim_start_matches("```").trim_end_matches("```").trim();
                
                // Check for any language specifier
                let content_parts: Vec<&str> = content.splitn(2, ' ').collect();
                if content_parts.len() == 2 && !content_parts[0].starts_with("mcp") {
                    // There's a language specifier, so check if the rest is an MCP command
                    let potential_command = content_parts[1].trim();
                    if potential_command.starts_with("mcp ") {
                        commands.push(potential_command.to_string());
                    }
                } else if content.starts_with("mcp ") {
                    // Direct MCP command
                    commands.push(content.to_string());
                }
                
                // Move to the next line
                i += 1;
                continue;
            }
            
            // Check for multi-line code block start - more permissive matching
            // Match any code blocks, with or without language specifier
            if line.starts_with("```") {
                let mut code_block_lines = Vec::new();
                let mut j = i + 1;
                
                // Collect lines until the end of the code block
                while j < lines.len() {
                    let current_line = lines[j];  // Don't trim here to preserve full line
                    // Break if we hit the end of the code block
                    if current_line.trim().starts_with("```") {
                        break;
                    }
                    code_block_lines.push(current_line);
                    j += 1;
                }
                
                // Process the code block content
                if !code_block_lines.is_empty() {
                    let code_block = code_block_lines.join("\n");
                    
                    // Look for both cases:
                    // 1. The entire code block is an MCP command
                    let block_trimmed = code_block.trim();
                    if block_trimmed.starts_with("mcp ") {
                        commands.push(block_trimmed.to_string());
                    }
                    
                    // 2. Individual lines within the code block are MCP commands
                    for line in code_block.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("mcp ") && trimmed != block_trimmed {
                            commands.push(trimmed.to_string());
                        }
                    }
                }
                
                // Skip to after the code block
                if j < lines.len() {
                    i = j + 1;
                } else {
                    break;
                }
            } else {
                i += 1;
            }
        }
    }
    
    /// Extract inline commands like `mcp shell ls`
    fn extract_inline_commands(&self, input: &str, commands: &mut Vec<String>) {
        let mut start_idx = 0;
        
        while let Some(start) = input[start_idx..].find("`mcp ") {
            let abs_start = start_idx + start;
            
            // Find the closing backtick that belongs to this command
            if let Some(end_offset) = input[abs_start+1..].find('`') {
                // Calculate the absolute end position
                let end_pos = abs_start + 1 + end_offset;
                
                // Extract command safely
                if end_pos > abs_start + 1 {
                    // Make sure we have a valid slice
                    let command = &input[abs_start+1..end_pos];
                    if command.starts_with("mcp ") {
                        commands.push(command.to_string());
                    }
                }
                
                // Move past this command
                start_idx = end_pos + 1;
            } else {
                // No closing backtick found, break the loop
                break;
            }
        }
    }
    
    /// Extract direct commands (not in code blocks)
    fn extract_direct_commands(&self, input: &str, commands: &mut Vec<String>) {
        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("mcp ") && !trimmed.contains('`') {
                commands.push(trimmed.to_string());
            }
        }
    }
    
    /// Original extract method for backward compatibility
    fn extract_command_from_markdown(&self, input: &str) -> Option<String> {
        let commands = self.extract_commands_from_text(input);
        commands.first().cloned()
    }
    
    /// Format a JSON response for human readability
    fn format_response(&self, json_str: &str) -> String {
        // Try to parse the JSON
        if let Ok(value) = serde_json::from_str::<Value>(json_str) {
            // Check if this is a tools.list response
            if let Some(result) = value.get("result") {
                // Format tools.list response
                if let Some(tools) = result.as_array() {
                    if !tools.is_empty() && tools[0].get("category").is_some() {
                        let mut output = String::from("Available MCP Tools:\n");
                        
                        for tool in tools {
                            let id = tool.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                            let desc = tool.get("description").and_then(|v| v.as_str()).unwrap_or("");
                            let category = tool.get("category").and_then(|v| v.as_str()).unwrap_or("other");
                            
                            output.push_str(&format!("- {} ({}): {} [{}]\n", name, id, desc, category));
                        }
                        
                        return output;
                    }
                }
                
                // Format mcp.help response
                if let Some(description) = result.get("description").and_then(|v| v.as_str()) {
                    let mut output = String::from("MCP Help:\n");
                    output.push_str(&format!("{}\n\n", description));
                    
                    // Add available methods
                    if let Some(methods) = result.get("available_methods").and_then(|v| v.as_array()) {
                        output.push_str("Available Methods:\n");
                        
                        for method in methods {
                            let name = method.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let desc = method.get("description").and_then(|v| v.as_str()).unwrap_or("");
                            
                            output.push_str(&format!("- {}: {}\n", name, desc));
                        }
                        
                        output.push_str("\nYou can use MCP with these commands:\n");
                        output.push_str("- mcp help - Show this help information\n");
                        output.push_str("- mcp tools - List available tools\n");
                        output.push_str("- mcp shell <command> - Execute a shell command\n");
                        output.push_str("- mcp search <pattern> - Search for content\n");
                        output.push_str("- mcp write <file_path> <content> - Create or update a file\n");
                        
                        return output;
                    }
                }
                
                // Format shell execution result
                if let Some(output) = result.get("output") {
                    if let Some(command) = output.get("command").and_then(|v| v.as_str()) {
                        let mut formatted = String::from("Shell Command Result:\n");
                        formatted.push_str(&format!("Command: {}\n", command));
                        
                        // Add exit code
                        if let Some(code) = output.get("exit_code").and_then(|v| v.as_i64()) {
                            formatted.push_str(&format!("Exit Code: {}\n", code));
                        }
                        
                        // Add stdout
                        if let Some(stdout) = output.get("stdout").and_then(|v| v.as_str()) {
                            if !stdout.is_empty() {
                                formatted.push_str("\nOutput:\n");
                                formatted.push_str(stdout);
                            }
                        }
                        
                        // Add stderr
                        if let Some(stderr) = output.get("stderr").and_then(|v| v.as_str()) {
                            if !stderr.is_empty() {
                                formatted.push_str("\nErrors:\n");
                                formatted.push_str(stderr);
                            }
                        }
                        
                        return formatted;
                    }
                }
                
                // Format search results
                if let Some(search_type) = result.get("search_type").and_then(|v| v.as_str()) {
                    let pattern = result.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
                    let total = result.get("total_found").and_then(|v| v.as_u64()).unwrap_or(0);
                    
                    let mut formatted = String::from("Search Results:\n");
                    formatted.push_str(&format!("Pattern: {}\n", pattern));
                    formatted.push_str(&format!("Total matches: {}\n", total));
                    
                    if search_type == "file" {
                        if let Some(matches) = result.get("file_matches").and_then(|v| v.as_array()) {
                            if !matches.is_empty() {
                                formatted.push_str("\nMatching Files:\n");
                                
                                for (i, m) in matches.iter().enumerate().take(20) {
                                    let path = m.get("path").and_then(|v| v.as_str()).unwrap_or("");
                                    let size = m.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let is_dir = m.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
                                    
                                    formatted.push_str(&format!("{}. {} ({} bytes) {}\n", 
                                        i + 1, path, size, if is_dir { "[DIR]" } else { "" }));
                                }
                            }
                        }
                    } else if search_type == "content" {
                        if let Some(matches) = result.get("content_matches").and_then(|v| v.as_array()) {
                            if !matches.is_empty() {
                                formatted.push_str("\nContent Matches:\n");
                                
                                for (i, m) in matches.iter().enumerate().take(20) {
                                    let path = m.get("path").and_then(|v| v.as_str()).unwrap_or("");
                                    let line_num = m.get("line_number").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let line = m.get("line").and_then(|v| v.as_str()).unwrap_or("");
                                    
                                    formatted.push_str(&format!("{}. {}:{}: {}\n", 
                                        i + 1, path, line_num, line));
                                }
                            }
                        }
                    }
                    
                    return formatted;
                }
            }
            
            // Format error response
            if let Some(error) = value.get("error") {
                let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
                let message = error.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                
                return format!("MCP Error ({}): {}", code, message);
            }
            
            // For any other JSON response, pretty print it
            if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                return pretty;
            }
        }
        
        // If we can't parse or format the JSON, return the original string
        json_str.to_string()
    }
    
    /// Create a new MCP agent from configuration
    pub fn from_config(config: &Config) -> Result<Self> {
        // Get base directory for file operations
        let base_dir = if let Some(dir) = &config.mcp.base_dir {
            PathBuf::from(dir)
        } else {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
        };
        
        // Create the underlying LLM agent for non-MCP messages
        let llm_agent = BedrockAgentSync::from_config(config);
        
        // Initialize the MCP framework
        let mcp_handler = init_mcp(base_dir.clone())?;
        
        // Create the resource manager
        let resource_manager = ResourceManager::new(base_dir.clone())?;
        let resource_manager = Arc::new(Mutex::new(resource_manager));
        
        // Create the tool manager
        let mut tool_manager = ToolManager::new(base_dir.clone(), resource_manager.clone());
        
        // Register default tools
        tool_manager.register_default_tools()?;
        
        // Wrap the tool manager in Arc<Mutex<>>
        let tool_manager = Arc::new(Mutex::new(tool_manager));
        
        Ok(Self {
            base_dir,
            mcp_handler,
            tool_manager,
            llm_agent,
        })
    }
    
    /// Process an MCP request message
    fn process_mcp_request(&self, request: &str) -> Result<String> {
        // Parse the request as JSON
        let request_json: Value = serde_json::from_str(request)
            .context("Failed to parse MCP request as JSON")?;
        
        // Extract the method from the request
        let method = request_json["method"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'method' field in MCP request"))?;
        
        match method {
            "tools.list" => self.handle_tools_list(&request_json),
            "tools.execute" => self.handle_tools_execute(&request_json),
            "mcp.help" => self.handle_mcp_help(&request_json),
            _ => Err(anyhow::anyhow!("Unsupported MCP method: {}", method)),
        }
    }
    
    /// Handle a tools.list request
    fn handle_tools_list(&self, request_json: &Value) -> Result<String> {
        // Get request ID
        let id = request_json["id"].clone();
        
        // Lock the tool manager
        let tool_manager = self.tool_manager.lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock tool manager"))?;
        
        // Get all tools
        let tools = tool_manager.get_tools();
        
        // Create response
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": tools
        });
        
        Ok(serde_json::to_string(&response)?)
    }
    
    /// Handle a mcp.help request
    fn handle_mcp_help(&self, request_json: &Value) -> Result<String> {
        // Get request ID
        let id = request_json["id"].clone();
        
        // Create help information
        let help_info = json!({
            "mcp_version": "1.0.0",
            "description": "Model Context Protocol (MCP) allows AI agents to interact with system tools securely",
            "available_methods": [
                {
                    "name": "tools.list",
                    "description": "List all available tools"
                },
                {
                    "name": "tools.execute",
                    "description": "Execute a tool with parameters",
                    "parameters": {
                        "tool_id": "ID of the tool to execute",
                        "params": "Tool-specific parameters"
                    }
                },
                {
                    "name": "mcp.help",
                    "description": "Get help information about MCP"
                }
            ],
            "example": {
                "tools.list": {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools.list"
                },
                "tools.execute": {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools.execute",
                    "params": {
                        "tool_id": "shell",
                        "params": {
                            "command": "ls -la",
                            "timeout": 10
                        }
                    }
                }
            }
        });
        
        // Create response
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": help_info
        });
        
        Ok(serde_json::to_string(&response)?)
    }
    
    /// Handle a tools.execute request
    fn handle_tools_execute(&self, request_json: &Value) -> Result<String> {
        // Get request ID
        let id = request_json["id"].clone();
        
        // Extract parameters
        let params = request_json["params"]
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'params' field in MCP request"))?;
        
        // Extract tool_id
        let tool_id = params["tool_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'tool_id' in tools.execute params"))?;
        
        // Extract tool_params
        let tool_params = params.get("params")
            .ok_or_else(|| anyhow::anyhow!("Missing 'params' in tools.execute params"))?
            .clone();
        
        // Lock the tool manager
        let tool_manager = self.tool_manager.lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock tool manager"))?;
        
        // Execute the tool
        let result = tool_manager.execute_tool(tool_id, tool_params)
            .context(format!("Failed to execute tool: {}", tool_id))?;
        
        // Create response
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        });
        
        Ok(serde_json::to_string(&response)?)
    }
}

impl McpAgent {
    /// Process all MCP commands in a response and return the combined results
    /// Enhanced with detailed debugging and more robust execution
    pub fn process_all_commands(&self, input: &str) -> (String, bool) {
        debug_log(&format!("Starting process_all_commands with input length: {}", input.len()));
        
        // Extract commands using the robust extraction method
        let commands = self.extract_commands_from_text(input);
        
        if commands.is_empty() {
            debug_log("No commands found in input text");
            // No commands to execute, return the original input and false (no commands executed)
            return (input.to_string(), false);
        }
        
        // Log found commands
        debug_log(&format!("Found {} commands to execute:", commands.len()));
        for (i, cmd) in commands.iter().enumerate() {
            debug_log(&format!("  Command {}: {}", i+1, cmd));
        }
        
        // Limit number of commands to prevent abuse
        const MAX_COMMANDS: usize = 20;
        let limited_commands = if commands.len() > MAX_COMMANDS {
            debug_log(&format!("WARNING: Limiting {} commands to {}.", commands.len(), MAX_COMMANDS));
            commands[0..MAX_COMMANDS].to_vec()
        } else {
            commands
        };
        
        // Build the response by replacing commands with their results
        let mut response = input.to_string();
        let mut has_executed_commands = false;
        let mut execution_success_count = 0;
        let mut execution_fail_count = 0;
        
        // Max result length to prevent extremely large outputs
        const MAX_RESULT_LENGTH: usize = 10_000; // 10KB
        
        for (i, command) in limited_commands.iter().enumerate() {
            debug_log(&format!("Executing command {}/{}: {}", i+1, limited_commands.len(), command));
            
            // Execute command with timeout protection
            let (tx, rx) = std::sync::mpsc::channel();
            let cmd_clone = command.clone();
            let self_clone = self.clone();
            
            // Create a thread for command execution
            let _cmd_thread = std::thread::spawn(move || {
                // Use panic handling for safety
                let result = match std::panic::catch_unwind(|| {
                    self_clone.execute_mcp_command(&cmd_clone)
                }) {
                    Ok(r) => r,
                    Err(e) => {
                        // Log the panic if possible
                        let panic_msg = match e.downcast_ref::<&'static str>() {
                            Some(s) => *s,
                            None => match e.downcast_ref::<String>() {
                                Some(s) => s.as_str(),
                                None => "Unknown panic"
                            }
                        };
                        debug_log(&format!("Command execution panicked: {}", panic_msg));
                        format!("ERROR: Command execution failed with panic: {}", cmd_clone)
                    }
                };
                
                let _ = tx.send(result);
            });
            
            // Wait for command execution with timeout (60 seconds)
            let timeout = std::time::Duration::from_secs(60);
            let start = std::time::Instant::now();
            
            let result = loop {
                match rx.try_recv() {
                    Ok(result) => {
                        debug_log(&format!("Command execution completed in {:?}", start.elapsed()));
                        break result;
                    },
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        if start.elapsed() > timeout {
                            debug_log(&format!("Command execution timed out after {:?}", timeout));
                            break format!("ERROR: Command execution timed out after {:?}: {}", timeout, command);
                        }
                        // Avoid tight loop
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    },
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        debug_log("Command execution thread crashed");
                        break format!("ERROR: Command execution thread crashed: {}", command);
                    }
                }
            };
            
            // Check if execution was successful
            let execution_successful = !result.starts_with("ERROR:");
            if execution_successful {
                execution_success_count += 1;
                debug_log("Command executed successfully");
            } else {
                execution_fail_count += 1;
                debug_log(&format!("Command execution failed: {}", result));
            }
            
            // Truncate result if too long
            let truncated_result = if result.len() > MAX_RESULT_LENGTH {
                debug_log(&format!("Truncating command result from {} to {} characters", 
                         result.len(), MAX_RESULT_LENGTH));
                format!(
                    "{}... \n\n[Result truncated from {} characters to {} characters]",
                    &result[..MAX_RESULT_LENGTH],
                    result.len(),
                    MAX_RESULT_LENGTH
                )
            } else {
                result
            };
            
            // Format the replacement with the command and its result
            // Enhanced formatting for better visibility in the UI
            let command_replacement = if execution_successful {
                format!(
                    "```\n{}\n```\n\n**Command Result (executed successfully):**\n```\n{}\n```\n", 
                    command, 
                    truncated_result
                )
            } else {
                format!(
                    "```\n{}\n```\n\n**Command Result (execution failed):**\n```\n{}\n```\n", 
                    command, 
                    truncated_result
                )
            };
            
            debug_log("Attempting to replace command in response text");
            
            // Track if we've replaced this command in the response
            let mut command_replaced = false;
            
            // Try various replacement patterns
            // First, try exact matches with code fences
            if response.contains(&format!("```\n{}\n```", command)) {
                debug_log("Replacing command using pattern 1 (exact match with code fences)");
                response = response.replace(&format!("```\n{}\n```", command), &command_replacement);
                has_executed_commands = true;
                command_replaced = true;
            }
            
            // If already replaced, continue to next command
            if command_replaced {
                continue;
            }
            
            // Try match for code fences without newlines (common pattern from LLMs)
            if response.contains(&format!("```{}", command)) && response.contains(&format!("{}```", command)) {
                // This handles the case of: ```mcp shell command```
                let pattern = format!("```{}```", command);
                if response.contains(&pattern) {
                    debug_log("Replacing command using pattern 2 (inline code fence)");
                    response = response.replace(&pattern, &command_replacement);
                    has_executed_commands = true;
                    command_replaced = true;
                }
            }
            
            // If already replaced, continue to next command
            if command_replaced {
                continue;
            }
            
            // Handle the case where there might be some whitespace or a language indicator after the opening ```
            let lines = response.lines().collect::<Vec<_>>();
            for (i, line) in lines.iter().enumerate() {
                if line.trim().starts_with("```") && line.contains(&*command) {
                    // Found a line that starts with ``` and contains the command
                    // Check if it also ends with ```
                    if line.trim().ends_with("```") {
                        debug_log("Replacing command using pattern 3 (single-line code block)");
                        // This is a single-line code block with the command
                        let mut modified_lines = lines.clone();
                        modified_lines[i] = &command_replacement;
                        response = modified_lines.join("\n");
                        has_executed_commands = true;
                        command_replaced = true;
                        break;
                    }
                }
            }
            
            // If already replaced, continue to next command
            if command_replaced {
                continue;
            }
            
            // Try inline code matches
            if response.contains(&format!("`{}`", command)) {
                debug_log("Replacing command using pattern 4 (inline code)");
                response = response.replace(&format!("`{}`", command), &command_replacement);
                has_executed_commands = true;
                command_replaced = true;
            }
            
            // If already replaced, continue to next command
            if command_replaced {
                continue;
            }
            
            // Try other code fence variations
            let fence_patterns = [
                format!("```bash\n{}\n```", command),
                format!("```shell\n{}\n```", command),
                format!("```sh\n{}\n```", command),
            ];
            
            for (j, pattern) in fence_patterns.iter().enumerate() {
                if response.contains(pattern) {
                    debug_log(&format!("Replacing command using pattern 5.{} (language specific code fence)", j+1));
                    response = response.replace(pattern, &command_replacement);
                    has_executed_commands = true;
                    command_replaced = true;
                    break;
                }
            }
            
            // If already replaced, continue to next command
            if command_replaced {
                continue;
            }
            
            // As a last resort, look for the command text on its own line
            let mut lines = response.lines().collect::<Vec<_>>();
            for i in 0..lines.len() {
                if lines[i].trim() == command {
                    debug_log("Replacing command using pattern 6 (plain line match)");
                    lines[i] = &command_replacement;
                    response = lines.join("\n");
                    has_executed_commands = true;
                    command_replaced = true;
                    break;
                }
            }
            
            // If we still haven't replaced anything, append the result
            if !command_replaced {
                debug_log("Command not found in response, appending result");
                response = format!("{}\n\n**Command Result:**\n```\n{}\n```", response, truncated_result);
                has_executed_commands = true;
            }
        }
        
        // Ensure the response doesn't exceed a reasonable size
        const MAX_RESPONSE_SIZE: usize = 100_000; // 100KB
        let final_response = if response.len() > MAX_RESPONSE_SIZE {
            debug_log(&format!("Truncating final response from {} to {} characters", 
                     response.len(), MAX_RESPONSE_SIZE));
            format!(
                "{}...\n\n[Response truncated from {} characters to {} characters]",
                &response[..MAX_RESPONSE_SIZE],
                response.len(),
                MAX_RESPONSE_SIZE
            )
        } else {
            response
        };
        
        // Log execution summary
        debug_log("Command execution summary:");
        debug_log(&format!("  Total commands: {}", limited_commands.len()));
        debug_log(&format!("  Successfully executed: {}", execution_success_count));
        debug_log(&format!("  Failed to execute: {}", execution_fail_count));
        debug_log(&format!("  Response size: {}", final_response.len()));
        
        (final_response, has_executed_commands)
    }

    /// Execute a multi-turn conversation with the agent
    pub fn execute_multi_turn(&self, input: &str, max_turns: usize) -> String {
        // We already check for simple queries in the process_message method
        // This method should only be called for non-simple queries

        // Execute with an empty progress handler
        self.execute_multi_turn_impl(input, max_turns, |_| {})
    }
    
    /// Execute a multi-turn conversation with progress reporting
    /// (Internal implementation that takes a closure)
    /// Enhanced with improved debug logging and more robust command execution
    fn execute_multi_turn_impl<F>(
        &self, 
        input: &str, 
        max_turns: usize,
        mut progress_callback: F
    ) -> String 
    where F: FnMut(&str) {
        // Log start of multi-turn execution
        debug_log(&format!("Starting multi-turn execution with max {} turns", max_turns));
        
        // Hard maximum number of turns as fail-safe
        const ABSOLUTE_MAX_TURNS: usize = 10; // Increased to 10 turns for more comprehensive interactions
        let actual_max_turns = max_turns.min(ABSOLUTE_MAX_TURNS);
        
        let mut current_input = input.to_string();
        let mut turn_count = 0;
        let mut executed_command_count = 0;
        let mut conversation_log = Vec::new();
        
        // Track if the conversation is making progress
        let mut last_response_hash = 0;
        
        // Initial progress update
        progress_callback(&format!("🔄 Starting multi-turn execution (max {} turns)", actual_max_turns));
        
        // Maximum response size to prevent unbounded growth
        const MAX_RESPONSE_SIZE: usize = 100_000; // 100KB
        
        while turn_count < actual_max_turns {
            // Current turn counter
            turn_count += 1;
            
            // Progress update for current turn
            let turn_msg = format!("🔄 Turn {}/{}: Generating response...", turn_count, actual_max_turns);
            progress_callback(&turn_msg);
            debug_log(&turn_msg);
            
            // Execute the LLM call in a separate thread with timeout
            let llm_response = {
                // Create a thread for LLM execution with timeout
                let (tx, rx) = std::sync::mpsc::channel();
                let agent_clone = self.llm_agent.clone();
                let current_input_clone = current_input.clone();
                
                debug_log(&format!("Creating LLM thread for turn {}", turn_count));
                let _llm_thread = std::thread::spawn(move || {
                    debug_log(&format!("LLM thread started for processing input of length {}", current_input_clone.len()));
                    let response = agent_clone.process_message(&current_input_clone);
                    debug_log(&format!("LLM thread completed with response of length {}", response.len()));
                    let _ = tx.send(response);
                });
                
                // Wait for completion with timeout
                let llm_timeout = std::time::Duration::from_secs(240); // 240-second timeout for LLM (increased from 120)
                let start = std::time::Instant::now();
                
                debug_log(&format!("Waiting for LLM response with timeout of {:?}", llm_timeout));
                let result = loop {
                    // Check if we have a result
                    match rx.try_recv() {
                        Ok(result) => {
                            debug_log(&format!("Received LLM response after {:?}", start.elapsed()));
                            break Ok(result)
                        },
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            // Check for timeout
                            if start.elapsed() > llm_timeout {
                                debug_log(&format!("LLM processing timed out after {:?}", llm_timeout));
                                break Err("LLM processing timed out".to_string());
                            }
                            
                            // Log every 10 seconds for visibility
                            if start.elapsed().as_secs() % 10 == 0 && start.elapsed().as_millis() < 100 {
                                debug_log(&format!("Still waiting for LLM response after {:?}", start.elapsed()));
                            }
                            
                            // Brief pause to avoid tight loop
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        },
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            debug_log("LLM processing thread crashed");
                            break Err("LLM processing thread crashed".to_string());
                        }
                    }
                };
                
                // Handle the result
                match result {
                    Ok(response) => response,
                    Err(err) => {
                        let error_msg = format!("⚠️ LLM error: {}. Aborting multi-turn execution.", err);
                        debug_log(&error_msg);
                        progress_callback(&error_msg);
                        return format!("Multi-turn execution failed: {}", error_msg);
                    }
                }
            };
            
            // Check if response is empty
            if llm_response.trim().is_empty() {
                let error_msg = "⚠️ Empty response from LLM. Aborting multi-turn execution.";
                debug_log(error_msg);
                progress_callback(error_msg);
                return format!("Multi-turn execution failed: {}", error_msg);
            }
            
            // Calculate hash of response to detect cycles
            let response_hash = llm_response.len() as u64;
            if response_hash == last_response_hash {
                let error_msg = "⚠️ Detected response cycle. Aborting multi-turn execution.";
                debug_log(error_msg);
                progress_callback(error_msg);
                return format!("{}\n\n[{}]", llm_response, error_msg);
            }
            last_response_hash = response_hash;
            
            // Extract commands before execution for logging
            let commands = self.extract_commands_from_text(&llm_response);
            
            // Log found commands
            debug_log(&format!("Found {} commands in turn {} response", commands.len(), turn_count));
            for (i, cmd) in commands.iter().enumerate() {
                debug_log(&format!("Command {}: {}", i+1, cmd));
            }
            
            // Limit number of commands per turn to prevent abuse and improve UI responsiveness
            const MAX_COMMANDS_PER_TURN: usize = 5; // Allow more commands per turn since each has its own timeout
            let limited_commands = if commands.len() > MAX_COMMANDS_PER_TURN {
                let warning = format!("⚠️ Too many commands ({}) in single turn. Limiting to {} for better responsiveness.", 
                    commands.len(), MAX_COMMANDS_PER_TURN);
                debug_log(&warning);
                progress_callback(&warning);
                commands[0..MAX_COMMANDS_PER_TURN].to_vec()
            } else {
                commands
            };
            
            // If commands were found, log and notify about them
            if !limited_commands.is_empty() {
                let command_list = limited_commands.join("\n  - ");
                let cmd_msg = format!("🔧 Turn {}/{}: Found {} command{} to execute:\n  - {}", 
                    turn_count, 
                    actual_max_turns,
                    limited_commands.len(),
                    if limited_commands.len() == 1 { "" } else { "s" },
                    command_list);
                    
                debug_log(&cmd_msg);
                progress_callback(&cmd_msg);
                conversation_log.push(cmd_msg);
                
                // Execute commands with improved safety and timeouts
                let mut command_results = Vec::new();
                
                // Process commands one by one with feedback and timeout protection
                for (i, cmd) in limited_commands.iter().enumerate() {
                    let execution_msg = format!("▶️ Executing command {}/{}: {}", i+1, limited_commands.len(), cmd);
                    debug_log(&execution_msg);
                    progress_callback(&execution_msg);
                    
                    // Create a thread for command execution with timeout
                    let (tx, rx) = std::sync::mpsc::channel();
                    let cmd_clone = cmd.clone();
                    let self_clone = self.clone();
                    
                    debug_log(&format!("Creating command execution thread for command: {}", cmd));
                    let _cmd_thread = std::thread::spawn(move || {
                        debug_log(&format!("Command thread started for: {}", cmd_clone));
                        // Use panic handling for safety
                        let result = match std::panic::catch_unwind(|| {
                            self_clone.execute_mcp_command(&cmd_clone)
                        }) {
                            Ok(r) => r,
                            Err(e) => {
                                // Log the panic if possible
                                let panic_msg = match e.downcast_ref::<&'static str>() {
                                    Some(s) => *s,
                                    None => match e.downcast_ref::<String>() {
                                        Some(s) => s.as_str(),
                                        None => "Unknown panic"
                                    }
                                };
                                debug_log(&format!("Command execution panicked: {}", panic_msg));
                                format!("ERROR: Command execution failed with panic: {}", cmd_clone)
                            }
                        };
                        
                        debug_log(&format!("Command thread completed with result length {}", result.len()));
                        let _ = tx.send(result);
                    });
                    
                    // Wait for completion with timeout
                    let cmd_timeout = std::time::Duration::from_secs(60); // Increased to 60 seconds
                    let start = std::time::Instant::now();
                    
                    debug_log(&format!("Waiting for command execution with timeout of {:?}", cmd_timeout));
                    let result = loop {
                        // Check if we have a result
                        match rx.try_recv() {
                            Ok(result) => {
                                debug_log(&format!("Received command result after {:?}", start.elapsed()));
                                break result
                            },
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                // Check for timeout
                                if start.elapsed() > cmd_timeout {
                                    debug_log(&format!("Command execution timed out after {:?}", cmd_timeout));
                                    break format!("⚠️ Command timed out after {:?}: {}", cmd_timeout, cmd);
                                }
                                
                                // Log every 10 seconds for visibility
                                if start.elapsed().as_secs() % 10 == 0 && start.elapsed().as_millis() < 100 {
                                    debug_log(&format!("Still waiting for command result after {:?}", start.elapsed()));
                                }
                                
                                // Brief pause to avoid tight loop
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            },
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                debug_log("Command execution thread crashed");
                                break "⚠️ Command execution thread crashed".to_string();
                            }
                        }
                    };
                    
                    // Check if execution succeeded
                    let execution_successful = !result.starts_with("ERROR:") && !result.starts_with("⚠️");
                    
                    // Store the command and its result
                    command_results.push((cmd.clone(), result.clone(), execution_successful));
                    
                    // Notify about the result (truncate if too long)
                    let truncated_result = if result.len() > 500 {
                        format!("{}... (truncated, {} characters total)", &result[..500], result.len())
                    } else {
                        result.clone()
                    };
                    
                    let result_msg = format!("{} Result of command {}/{}:\n{}", 
                        if execution_successful { "✅" } else { "❌" },
                        i+1, limited_commands.len(), truncated_result);
                    debug_log(&format!("DEBUG: {}", result_msg));
                    progress_callback(&result_msg);
                }
                
                // Process all commands with the results
                let mut processed_response = llm_response.clone();
                
                // Replace each command with its result
                for (cmd, result, execution_successful) in command_results {
                    // Format the command replacement differently based on success/failure
                    let command_replacement = if execution_successful {
                        format!(
                            "```\n{}\n```\n\n**Command Result (executed successfully):**\n```\n{}\n```\n", 
                            cmd, 
                            result
                        )
                    } else {
                        format!(
                            "```\n{}\n```\n\n**Command Result (execution failed):**\n```\n{}\n```\n", 
                            cmd, 
                            result
                        )
                    };
                    
                    // Track if we've replaced this command in the response
                    let mut command_replaced = false;
                    
                    // Try to replace the command in the response with various patterns
                    // First, try exact matches with code fences
                    if processed_response.contains(&format!("```\n{}\n```", cmd)) {
                        debug_log("Replacing command using pattern 1 (exact match with code fences)");
                        processed_response = processed_response.replace(&format!("```\n{}\n```", cmd), &command_replacement);
                        command_replaced = true;
                    }
                    
                    // If already replaced, continue to next command
                    if command_replaced {
                        continue;
                    }
                    
                    // Try inline code fences (```command```)
                    if processed_response.contains(&format!("```{}```", cmd)) {
                        debug_log("Replacing command using pattern 2 (inline code fence)");
                        processed_response = processed_response.replace(&format!("```{}```", cmd), &command_replacement);
                        command_replaced = true;
                    }
                    
                    // If already replaced, continue to next command
                    if command_replaced {
                        continue;
                    }
                    
                    // Try inline backticks (`command`)
                    if processed_response.contains(&format!("`{}`", cmd)) {
                        debug_log("Replacing command using pattern 3 (inline backticks)");
                        processed_response = processed_response.replace(&format!("`{}`", cmd), &command_replacement);
                        command_replaced = true;
                    }
                    
                    // If already replaced, continue to next command
                    if command_replaced {
                        continue;
                    }
                    
                    // Try other code fence variations
                    let fence_patterns = [
                        format!("```bash\n{}\n```", cmd),
                        format!("```shell\n{}\n```", cmd),
                        format!("```sh\n{}\n```", cmd),
                    ];
                    
                    for (j, pattern) in fence_patterns.iter().enumerate() {
                        if processed_response.contains(pattern) {
                            debug_log(&format!("Replacing command using pattern 4.{} (language specific code fence)", j+1));
                            processed_response = processed_response.replace(pattern, &command_replacement);
                            command_replaced = true;
                            break;
                        }
                    }
                    
                    // If already replaced, continue to next command
                    if command_replaced {
                        continue;
                    }
                    
                    // Check for the command in individual lines
                    let lines = processed_response.lines().collect::<Vec<_>>();
                    for (i, line) in lines.iter().enumerate() {
                        if line.trim() == cmd {
                            debug_log("Replacing command using pattern 5 (plain line match)");
                            let mut new_lines = lines.clone();
                            new_lines[i] = &command_replacement;
                            processed_response = new_lines.join("\n");
                            command_replaced = true;
                            break;
                        }
                    }
                    
                    // If we still haven't replaced anything, append the result
                    if !command_replaced {
                        debug_log("Command not found in response, appending result");
                        processed_response = format!("{}\n\n**Command Result:**\n```\n{}\n```", processed_response, result);
                    }
                }
                
                // Update command execution count
                executed_command_count += limited_commands.len();
                
                // Current input is now the processed response
                current_input = processed_response;
                
                let summary_msg = format!("✅ Turn {}/{}: Executed {} command{}. Total so far: {}", 
                    turn_count,
                    actual_max_turns,
                    limited_commands.len(),
                    if limited_commands.len() == 1 { "" } else { "s" },
                    executed_command_count);
                debug_log(&summary_msg);
                progress_callback(&summary_msg);
            } else {
                let info_msg = format!("ℹ️ Turn {}/{}: No commands found in response", turn_count, actual_max_turns);
                debug_log(&info_msg);
                progress_callback(&info_msg);
                
                // Just use the LLM response as is
                current_input = llm_response;
            }
            
            // If we're at the last turn, we're done
            if turn_count >= actual_max_turns {
                // Notify completion
                let completion_msg = format!("⚠️ Reached maximum turns ({}). Multi-turn conversation stopped.", actual_max_turns);
                debug_log(&completion_msg);
                progress_callback(&completion_msg);
                
                // Append detailed execution log at the end if we executed any commands
                let log_str = if !conversation_log.is_empty() {
                    format!("\n\n**Multi-turn Execution Log:**\n{}", conversation_log.join("\n"))
                } else {
                    String::new()
                };
                
                let final_result = format!("{}\n\n[{}]{}",
                    current_input, completion_msg, log_str);
                
                debug_log(&format!("Multi-turn execution completed with final response length: {}", final_result.len()));
                return final_result;
            }
            
            // Continue the conversation with the processed response
            let next_turn_msg = format!("⏩ Moving to turn {}/{} with response...", turn_count + 1, actual_max_turns);
            debug_log(&next_turn_msg);
            progress_callback(&next_turn_msg);
        }
        
        // We should never get here due to the loop condition, but just in case...
        let max_turns_msg = format!("⚠️ Reached maximum turns ({}). Multi-turn conversation stopped.", actual_max_turns);
        debug_log(&max_turns_msg);
        progress_callback(&max_turns_msg);
        
        let final_result = format!("{}\n\n[{}. Total commands executed: {}]",
            current_input, max_turns_msg, executed_command_count);
            
        debug_log(&format!("Multi-turn execution completed with final response length: {}", final_result.len()));
        final_result
    }
}

impl Agent for McpAgent {
    fn process_message(&self, input: &str) -> String {
        // For simpler queries, bypass MCPAgent's multi-turn processing to improve responsiveness
        let trimmed = input.trim();
        
        // First check if this is a simple MCP command
        if trimmed.starts_with("mcp ") || trimmed.starts_with("/mcp ") {
            let command = if trimmed.starts_with("/") {
                trimmed.trim_start_matches("/")
            } else {
                trimmed
            };
            
            return self.execute_mcp_command(command);
        }
        
        // Special handling for test commands to make sure we execute them
        // while avoiding the full MCP multi-turn execution that can lock up the UI
        if trimmed.len() < 20 || trimmed.to_lowercase().contains("test") {
            // First check if this contains any MCP commands that need to be executed
            let commands = self.extract_commands_from_text(input);
            if !commands.is_empty() {
                // There are MCP commands, so execute them and return the results
                let (response, _) = self.process_all_commands(input);
                return response;
            }
            
            // If no commands, just pass to LLM agent
            return self.llm_agent.process_message(input);
        }
        
        // Check if this is a direct JSON-RPC message
        if let Ok(request_json) = serde_json::from_str::<Value>(input) {
            if request_json["jsonrpc"] == "2.0" && request_json["method"].is_string() {
                // This is an MCP request
                return match self.process_mcp_request(input) {
                    Ok(response) => self.format_response(&response),
                    Err(err) => {
                        // Create an error response
                        let id = request_json["id"].clone();
                        let error_response = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32000,
                                "message": format!("Error processing MCP request: {}", err)
                            }
                        });
                        
                        let error_json = serde_json::to_string(&error_response)
                            .unwrap_or_else(|_| format!("Error processing MCP request: {}", err));
                        
                        self.format_response(&error_json)
                    }
                };
            }
        }
        
        // This is a regular conversation, handle it with multi-turn support
        // Use a reasonable limit to prevent infinite loops
        self.execute_multi_turn(input, 10)
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

impl McpAgent {
    /// Process a message with progress reporting and improved responsiveness
    pub fn process_message_with_progress<F>(&self, input: &str, mut progress_callback: F) -> String 
    where F: FnMut(&str) + Send + 'static {
        // First check if this is a simple MCP command
        let trimmed = input.trim();
        if trimmed.starts_with("mcp ") || trimmed.starts_with("/mcp ") {
            let command = if trimmed.starts_with("/") {
                trimmed.trim_start_matches("/")
            } else {
                trimmed
            };
            
            // Log that we're executing the command
            progress_callback(&format!("▶️ Executing single command: {}", command));
            
            // Execute the command with a short timeout
            const CMD_TIMEOUT_SECS: u64 = 5;
            
            // Create a thread to execute the command
            let (tx, rx) = std::sync::mpsc::channel();
            let command_clone = command.to_string();
            let self_clone = self.clone(); // Clone the agent to avoid reference issues
            let _cmd_thread = std::thread::spawn(move || {
                // Execute the command in this thread
                let result = match std::panic::catch_unwind(|| {
                    self_clone.execute_mcp_command(&command_clone)
                }) {
                    Ok(r) => r,
                    Err(_) => format!("Error executing command: {}", command_clone),
                };
                
                // Send the result
                let _ = tx.send(result);
            });
            
            // Wait for the result with a timeout
            let start = std::time::Instant::now();
            loop {
                // Check if we have a result
                match rx.try_recv() {
                    Ok(result) => {
                        // Log completion
                        progress_callback(&format!("✅ Command completed successfully"));
                        return result;
                    },
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // No result yet, check for timeout
                        if start.elapsed() > std::time::Duration::from_secs(CMD_TIMEOUT_SECS) {
                            // Log timeout
                            progress_callback(&format!("⚠️ Command execution timed out after {} seconds", CMD_TIMEOUT_SECS));
                            return format!("Command execution timed out after {} seconds: {}", CMD_TIMEOUT_SECS, command);
                        }
                        
                        // Update progress periodically
                        if start.elapsed().as_millis() % 500 < 100 {
                            progress_callback(&format!("⏳ Still executing command: {}", command));
                        }
                        
                        // Small sleep to avoid tight loop
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    },
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Thread crashed
                        progress_callback(&format!("❌ Command execution thread crashed"));
                        return format!("Error executing command (thread crashed): {}", command);
                    }
                }
            }
        }
        
        // Check if this is a direct JSON-RPC message
        if let Ok(request_json) = serde_json::from_str::<Value>(input) {
            if request_json["jsonrpc"] == "2.0" && request_json["method"].is_string() {
                // This is an MCP request
                progress_callback("🔄 Processing JSON-RPC request");
                let result = match self.process_mcp_request(input) {
                    Ok(response) => {
                        progress_callback("✅ JSON-RPC request completed successfully");
                        self.format_response(&response)
                    },
                    Err(err) => {
                        // Create an error response
                        progress_callback(&format!("❌ JSON-RPC request failed: {}", err));
                        let id = request_json["id"].clone();
                        let error_response = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32000,
                                "message": format!("Error processing MCP request: {}", err)
                            }
                        });
                        
                        let error_json = serde_json::to_string(&error_response)
                            .unwrap_or_else(|_| format!("Error processing MCP request: {}", err));
                        
                        self.format_response(&error_json)
                    }
                };
                return result;
            }
        }
        
        // Quick check for commands to extract
        let commands = self.extract_commands_from_text(input);
        if !commands.is_empty() {
            progress_callback(&format!("🔍 Found {} MCP commands to execute", commands.len()));
            
            // Execute each command with timeout protection
            let mut all_results = Vec::new();
            let mut any_executed = false;
            
            for (i, cmd) in commands.iter().enumerate() {
                progress_callback(&format!("▶️ Executing command {}/{}: {}", i+1, commands.len(), cmd));
                
                // Create a thread for command execution with timeout
                let (tx, rx) = std::sync::mpsc::channel();
                let cmd_clone = cmd.to_string();
                let agent_copy = self.clone();
                
                std::thread::spawn(move || {
                    let result = agent_copy.execute_mcp_command(&cmd_clone);
                    let _ = tx.send(result);
                });
                
                // Wait for completion with timeout
                let cmd_timeout = std::time::Duration::from_secs(5);
                let start = std::time::Instant::now();
                
                let cmd_result = loop {
                    match rx.try_recv() {
                        Ok(result) => break result,
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            if start.elapsed() > cmd_timeout {
                                break format!("Command execution timed out after {:?}", cmd_timeout);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        },
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            break "Command execution thread crashed".to_string();
                        }
                    }
                };
                
                // Add the result to our collection
                all_results.push((cmd.clone(), cmd_result.clone()));
                any_executed = true;
                
                // Log the result
                progress_callback(&format!("✅ Command {}/{} completed", i+1, commands.len()));
            }
            
            // Build response with command results
            if any_executed {
                let mut response = input.to_string();
                
                // Replace each command with its result
                for (cmd, result) in all_results {
                    let command_replacement = format!(
                        "```\n{}\n```\n\n**Command Result (executed successfully):**\n```\n{}\n```\n", 
                        cmd, 
                        result
                    );
                    
                    // Try to replace the command in the input
                    // Similar to our process_all_commands but simpler
                    if response.contains(&format!("```\n{}\n```", cmd)) {
                        response = response.replace(&format!("```\n{}\n```", cmd), &command_replacement);
                    } else if response.contains(&format!("```{}```", cmd)) {
                        response = response.replace(&format!("```{}```", cmd), &command_replacement);
                    } else if response.contains(&format!("`{}`", cmd)) {
                        response = response.replace(&format!("`{}`", cmd), &command_replacement);
                    } else {
                        // If we couldn't replace, just append
                        response = format!("{}\n\n{}", response, command_replacement);
                    }
                }
                
                progress_callback(&format!("✅ Completed execution of {} commands", commands.len()));
                return response;
            }
        }
        
        // This is a regular conversation, handle it with multi-turn support
        // Use a reasonable limit to prevent infinite loops
        progress_callback("🔄 Starting LLM processing (limited to 5 turns for balanced UI responsiveness)");
        
        // Allow for 5 turns - increased from 3 for better interactions while maintaining responsiveness
        // We previously had this set to 1, which was too limiting for most interactions
        let result = self.execute_multi_turn_impl(input, 5, |status| {
            progress_callback(status);
        });
        
        // Call the callback one last time
        progress_callback("✅ LLM processing completed");
        result
    }
}
