// This is a standalone utility to filter out patch tool JSON-RPC messages
// from text input. It can be compiled and run directly with:
//
// rustc patch_filter.rs
// ./patch_filter < input.txt
//
// It takes input from stdin and outputs to stdout, with JSON messages replaced.

use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    println!("Patch Tool JSON Filter");
    println!("Enter text (with potential JSON). Ctrl+D to finish.");
    
    for line in stdin.lock().lines() {
        let input = match line {
            Ok(text) => text,
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        };
        
        // Simple filter - if the line contains patch tool indicators,
        // replace it with a helpful message
        let filtered = if contains_patch_pattern(&input) {
            "[Detected patch tool command - processing...]".to_string()
        } else {
            input
        };
        
        // Output the filtered line
        writeln!(stdout, "{}", filtered).unwrap();
    }
}

// Check if text contains patch tool JSON patterns
fn contains_patch_pattern(text: &str) -> bool {
    // Look for JSON-RPC with patch tool indicators
    text.contains("\"name\":\"patch\"") && 
    text.contains("\"method\":\"mcp.tool_call\"") &&
    text.contains("\"jsonrpc\":\"2.0\"")
}