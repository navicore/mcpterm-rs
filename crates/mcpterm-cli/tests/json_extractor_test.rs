use anyhow::Result;
use mcp_core::extract_jsonrpc_objects;
use serde_json::json;

#[test]
fn test_extract_clean_jsonrpc() -> Result<()> {
    let content = r##"{
        "jsonrpc": "2.0",
        "method": "mcp.tool_call",
        "params": {
            "name": "file_write",
            "parameters": {
                "path": "test.txt",
                "content": "hello world"
            }
        },
        "id": "test1"
    }
    "##;
    
    let objects = extract_jsonrpc_objects(content);
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0]["method"], "mcp.tool_call");
    assert_eq!(objects[0]["params"]["name"], "file_write");
    
    Ok(())
}

#[test]
fn test_extract_multiple_jsonrpc() -> Result<()> {
    let content = r##"{
        "jsonrpc": "2.0",
        "method": "mcp.tool_call",
        "params": {
            "name": "file_write",
            "parameters": {
                "path": "test1.txt",
                "content": "hello world 1"
            }
        },
        "id": "test1"
    }
    {
        "jsonrpc": "2.0",
        "method": "mcp.tool_call",
        "params": {
            "name": "file_write",
            "parameters": {
                "path": "test2.txt",
                "content": "hello world 2"
            }
        },
        "id": "test2"
    }
    "##;
    
    let objects = extract_jsonrpc_objects(content);
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0]["params"]["parameters"]["path"], "test1.txt");
    assert_eq!(objects[1]["params"]["parameters"]["path"], "test2.txt");
    
    Ok(())
}

#[test]
fn test_extract_jsonrpc_with_text() -> Result<()> {
    let content = r##"I'll help you create those files. First, let's create the README:
    
    {
        "jsonrpc": "2.0",
        "method": "mcp.tool_call",
        "params": {
            "name": "file_write",
            "parameters": {
                "path": "README.md",
                "content": "# Test Project\n\nThis is a test project."
            }
        },
        "id": "readme"
    }
    
    Now, let's create the LICENSE file:
    
    {
        "jsonrpc": "2.0",
        "method": "mcp.tool_call",
        "params": {
            "name": "file_write",
            "parameters": {
                "path": "LICENSE",
                "content": "MIT License\n\nCopyright (c) 2023"
            }
        },
        "id": "license"
    }
    
    Both files have been created successfully!
    "##;
    
    let objects = extract_jsonrpc_objects(content);
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0]["id"], "readme");
    assert_eq!(objects[1]["id"], "license");
    
    Ok(())
}

#[test]
fn test_extract_jsonrpc_claude_style() -> Result<()> {
    // This is the style we saw in the logs
    let content = r##"
    {
      "jsonrpc": "2.0",
      "method": "mcp.tool_call",
      "params": {
        "name": "file_write",
        "parameters": {
          "path": "README.md",
          "content": "# Hello World Go Application\n\nA simple Go application that prints \"Hello, World!\" to the console."
        }
      },
      "id": "write_readme"
    }
    
    Executing embedded tool: file_write
    
    I've received the following tool result:
    ```json
    {
      "success": true,
      "bytes_written": 616,
      "path": "README.md"
    }
    ```
    
    Now I need to provide a direct answer based on this result.
    
    {
      "jsonrpc": "2.0",
      "method": "mcp.tool_call",
      "params": {
        "name": "file_write",
        "parameters": {
          "path": "LICENSE",
          "content": "MIT License\n\nCopyright (c) 2023"
        }
      },
      "id": "write_license"
    }
    "##;
    
    let objects = extract_jsonrpc_objects(content);
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0]["id"], "write_readme");
    assert_eq!(objects[1]["id"], "write_license");
    
    Ok(())
}

#[test]
fn test_extract_nested_json() -> Result<()> {
    // Test with nested JSON content inside the parameters
    let content = r##"{
        "jsonrpc": "2.0",
        "method": "mcp.tool_call",
        "params": {
            "name": "file_write",
            "parameters": {
                "path": "data.json",
                "content": "{ \"nested\": true, \"data\": { \"items\": [1, 2, 3] } }"
            }
        },
        "id": "json_data"
    }
    "##;
    
    let objects = extract_jsonrpc_objects(content);
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0]["id"], "json_data");
    assert!(objects[0]["params"]["parameters"]["content"].as_str().unwrap().contains("\"nested\"")); 
    
    Ok(())
}