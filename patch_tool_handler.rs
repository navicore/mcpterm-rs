// This file contains a custom handler for patch tool commands
// that can be used in testing or experimental settings where
// the core JSON-RPC extractor is having issues.

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

/// A simple utility to detect and handle patch tool JSON-RPC commands
/// in the input stream.
///
/// This is meant to be used as a standalone utility during testing
/// until the main JSON-RPC handler issues are resolved.
pub fn handle_patch_commands() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    println!("Patch Tool Handler");
    println!("Enter JSON-RPC commands (including patch tool calls)");
    println!("Type 'exit' to quit");
    
    for line in stdin.lock().lines() {
        let input = line?;
        let trimmed = input.trim();
        
        if trimmed == "exit" {
            break;
        }
        
        // Try to parse as JSON
        match serde_json::from_str::<Value>(trimmed) {
            Ok(json) => {
                // Check if it's a patch tool call
                if is_patch_tool_call(&json) {
                    println!("Detected patch tool call");
                    
                    // Extract parameters
                    if let Some(params) = json.get("params") {
                        if let Some(parameters) = params.get("parameters") {
                            if let (Some(target), Some(patch)) = (
                                parameters.get("target_file").and_then(|v| v.as_str()),
                                parameters.get("patch_content").and_then(|v| v.as_str())
                            ) {
                                println!("Target file: {}", target);
                                println!("Patch content:\n{}", patch);
                                
                                // Here you would call the patch tool directly
                                // For now, just echo back a success response
                                let response = json!({
                                    "jsonrpc": "2.0",
                                    "result": {
                                        "success": true,
                                        "target_file": target,
                                        "hunks_applied": 1,
                                        "hunks_failed": 0,
                                        "conflicts": []
                                    },
                                    "id": json.get("id").unwrap_or(&json!(null))
                                });
                                
                                println!("Response:\n{}", serde_json::to_string_pretty(&response)?);
                            } else {
                                println!("Missing required parameters");
                            }
                        }
                    }
                } else {
                    println!("Not a patch tool call");
                }
            },
            Err(e) => {
                println!("Invalid JSON: {}", e);
            }
        }
        
        stdout.write_all(b"> ")?;
        stdout.flush()?;
    }
    
    Ok(())
}

/// Check if the JSON represents a patch tool call
fn is_patch_tool_call(json: &Value) -> bool {
    if let Some(method) = json.get("method").and_then(|v| v.as_str()) {
        if method == "mcp.tool_call" {
            if let Some(params) = json.get("params") {
                if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                    return name == "patch";
                }
            }
        }
    }
    false
}

/// Entry point for the standalone tool
fn main() -> io::Result<()> {
    handle_patch_commands()
}