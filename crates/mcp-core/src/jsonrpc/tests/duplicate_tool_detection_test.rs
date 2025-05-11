use serde_json::json;
use crate::jsonrpc::extractor::extract_jsonrpc_objects;

/// Test to verify that a single tool call in multiple message chunks is only processed once
#[test]
fn test_duplicate_jsonrpc_detection() {
    // Simulating a scenario where the same tool call appears in multiple message chunks
    let content1 = r#"
I'll help you create a Rust hello world project. First, let me check if Rust is installed and then create a new project.

{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "shell",
    "parameters": {
      "command": "rustc --version"
    }
  },
  "id": "1"
}
"#;

    let content2 = r#"
Tool 'shell' returned result: {
  "error": null,
  "output": {
    "exit_code": 0,
    "stderr": "",
    "stdout": "rustc 1.86.0 (05f9846f8 2025-03-31)\n"
  },
  "status": "Success",
  "tool_id": "shell"
}

Now I'll create a new Rust project called hello_world.

{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "shell",
    "parameters": {
      "command": "cargo new hello_world"
    }
  },
  "id": "2"
}
"#;

    let content3 = r#"
Tool 'shell' returned result: {
  "error": null,
  "output": {
    "exit_code": 0,
    "stderr": "    Creating binary (application) `hello_world` package\nnote: see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html\n",
    "stdout": ""
  },
  "status": "Success",
  "tool_id": "shell"
}

{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "shell",
    "parameters": {
      "command": "cargo new hello_world"
    }
  },
  "id": "3"
}
"#;

    // Extract the JSON-RPC objects
    let objects1 = extract_jsonrpc_objects(content1);
    let objects2 = extract_jsonrpc_objects(content2);
    let objects3 = extract_jsonrpc_objects(content3);

    // Verify we correctly extract each tool call
    assert_eq!(objects1.len(), 1, "Should extract one tool call from first chunk");
    assert_eq!(objects2.len(), 1, "Should extract one tool call from second chunk");
    assert_eq!(objects3.len(), 1, "Should extract one tool call from third chunk");

    // Now verify that the command is the same in the last two tool calls
    let cmd2 = objects2[0]["params"]["parameters"]["command"].as_str().unwrap();
    let cmd3 = objects3[0]["params"]["parameters"]["command"].as_str().unwrap();
    
    assert_eq!(cmd2, "cargo new hello_world", "Second tool call should be cargo new");
    assert_eq!(cmd3, "cargo new hello_world", "Third tool call should be cargo new");
    assert_eq!(cmd2, cmd3, "The commands in the 2nd and 3rd chunks should be identical");

    // They're identical - this means we need to detect and prevent duplicate execution
}

/// Test to simulate the conversation from the log with duplicate tool calls
#[test]
fn test_real_world_duplicate_scenario() {
    // This test recreates the scenario from the log where cargo new was executed twice
    
    // Combine the chunks as they might be seen by the system
    let full_conversation = r#"
I need to create a Rust Hello World project. Let me help you with that. First, I'll check if Rust is installed and then create a new project.

{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "shell",
    "parameters": {
      "command": "rustc --version"
    }
  },
  "id": "1"
}

Tool 'shell' returned result: {
  "error": null,
  "output": {
    "exit_code": 0,
    "stderr": "",
    "stdout": "rustc 1.86.0 (05f9846f8 2025-03-31)\n"
  },
  "status": "Success",
  "tool_id": "shell"
}

{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "shell",
    "parameters": {
      "command": "cargo new hello_world"
    }
  },
  "id": "2"
}

Tool 'shell' returned result: {
  "error": null,
  "output": {
    "exit_code": 0,
    "stderr": "    Creating binary (application) `hello_world` package\nnote: see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html\n",
    "stdout": ""
  },
  "status": "Success",
  "tool_id": "shell"
}

{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "shell",
    "parameters": {
      "command": "cargo new hello_world"
    }
  },
  "id": "3"
}

Tool 'shell' returned result: {
  "error": "Command exited with non-zero status: 101",
  "output": {
    "exit_code": 101,
    "stderr": "    Creating binary (application) `hello_world` package\nerror: destination `/Users/navicore/tmp/mcp/test1/hello_world` already exists\n\nUse `cargo init` to initialize the directory\n",
    "stdout": ""
  },
  "status": "Failure",
  "tool_id": "shell"
}
"#;

    // Extract all JSON-RPC objects
    let all_objects = extract_jsonrpc_objects(full_conversation);
    
    // This will extract all objects, but the session manager processes them sequentially
    // In reality, we want to track which tool calls have already been executed
    
    // Count unique tool IDs
    let mut tool_ids = Vec::new();
    for obj in &all_objects {
        if let Some(id) = obj.get("id") {
            tool_ids.push(id.to_string());
        }
    }
    
    // Count the number of "cargo new hello_world" commands
    let mut cargo_new_count = 0;
    for obj in &all_objects {
        if let Some(params) = obj.get("params") {
            if let Some(parameters) = params.get("parameters") {
                if let Some(cmd) = parameters.get("command") {
                    if cmd == "cargo new hello_world" {
                        cargo_new_count += 1;
                    }
                }
            }
        }
    }
    
    // Verify that we have multiple identical tool calls, confirming the issue
    assert!(cargo_new_count > 1, "Should detect multiple identical cargo new commands");
}