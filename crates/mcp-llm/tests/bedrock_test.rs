use anyhow::Result;
use futures::StreamExt;
use mcp_core::context::{ConversationContext, MessageRole};
use mcp_llm::bedrock::{BedrockClient, BedrockConfig};
use mcp_llm::client_trait::LlmClient;
use mcp_llm::schema::McpSchemaManager;
use std::env;

#[tokio::test]
#[ignore] // This test requires AWS credentials and will make actual API calls
async fn test_bedrock_client_with_mcp_validation() -> Result<()> {
    // Check for AWS credentials in environment
    if env::var("AWS_ACCESS_KEY_ID").is_err() || env::var("AWS_SECRET_ACCESS_KEY").is_err() {
        println!("Skipping test because AWS credentials not found in environment");
        return Ok(());
    }

    // Create the client
    let config = BedrockConfig::claude()
        .with_temperature(0.5)
        .with_max_tokens(1000);
    
    let client = BedrockClient::new(config).await?;
    
    // Create a schema validator
    let schema_manager = McpSchemaManager::new();
    
    // Create a simple conversation
    let mut context = ConversationContext::new();
    
    // Add system message with specific MCP instructions
    context.add_system_message(
        "You MUST respond using the Model Context Protocol (MCP) format. \
        Always structure your responses as valid JSON-RPC 2.0 messages with the following format:\
        {\
            \"jsonrpc\": \"2.0\",\
            \"result\": \"Your response here\",\
            \"id\": \"request-123\"\
        }"
    );
    
    // Add user message
    context.add_user_message("Please give me a brief greeting.");
    
    // Test send_message
    let response = client.send_message(&context).await?;
    println!("Response: {:?}", response);
    
    // Response should have content
    assert!(!response.content.is_empty(), "Response should have content");
    
    // Content should be valid JSON
    let json_value = serde_json::from_str::<serde_json::Value>(&response.content)?;
    
    // Validate against MCP schema
    let validation_result = schema_manager.validate_response(&json_value);
    assert!(validation_result.is_ok(), "Response should be valid MCP format: {:?}", validation_result);
    
    // JSON-RPC specific validation
    assert!(json_value.get("jsonrpc").is_some(), "Should have jsonrpc field");
    assert_eq!(json_value["jsonrpc"].as_str().unwrap(), "2.0", "Should have jsonrpc version 2.0");
    assert!(json_value.get("id").is_some(), "Should have id field");
    assert!(json_value.get("result").is_some() || json_value.get("error").is_some(), 
            "Should have either result or error field");
    
    Ok(())
}

#[tokio::test]
#[ignore] // This test requires AWS credentials and will make actual API calls
async fn test_bedrock_streaming_with_mcp() -> Result<()> {
    // Check for AWS credentials in environment
    if env::var("AWS_ACCESS_KEY_ID").is_err() || env::var("AWS_SECRET_ACCESS_KEY").is_err() {
        println!("Skipping test because AWS credentials not found in environment");
        return Ok(());
    }

    // Create the client
    let config = BedrockConfig::claude()
        .with_temperature(0.5)
        .with_max_tokens(1000);
    
    let client = BedrockClient::new(config).await?;
    
    // Create a schema validator
    let schema_manager = McpSchemaManager::new();
    
    // Create a simple conversation
    let mut context = ConversationContext::new();
    
    // Add system message with specific MCP instructions
    context.add_system_message(
        "You MUST respond using the Model Context Protocol (MCP) format. \
        Always structure your responses as valid JSON-RPC 2.0 messages."
    );
    
    // Add user message
    context.add_user_message("List three famous computer scientists and their contributions.");
    
    // Test streaming
    let mut stream = client.stream_message(&context).await?;
    let mut chunks_received = 0;
    let mut complete_content = String::new();
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        println!("Chunk: {:?}", chunk);
        chunks_received += 1;
        
        if !chunk.content.is_empty() {
            complete_content.push_str(&chunk.content);
        }
        
        if chunk.is_complete {
            println!("Stream completed");
            break;
        }
        
        // Limit chunks to prevent test from running too long
        if chunks_received > 50 {
            break;
        }
    }
    
    assert!(chunks_received > 0, "Should have received at least one chunk");
    
    // Try to parse the final accumulated content
    if !complete_content.is_empty() {
        // Content should be valid JSON by the end of streaming
        let json_result = serde_json::from_str::<serde_json::Value>(&complete_content);
        
        if let Ok(json_value) = json_result {
            // Validate against MCP schema
            let validation_result = schema_manager.validate_response(&json_value);
            assert!(validation_result.is_ok(), 
                "Final streamed content should be valid MCP format: {:?}", validation_result);
            
            // Basic JSON-RPC validation
            assert!(json_value.get("jsonrpc").is_some(), "Should have jsonrpc field");
            assert!(json_value.get("id").is_some(), "Should have id field");
        } else {
            println!("Warning: Final streamed content is not valid JSON: {}", complete_content);
            // It's possible streaming was interrupted before complete
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // This test requires AWS credentials and will make actual API calls
async fn test_bedrock_tool_call_with_mcp() -> Result<()> {
    // Check for AWS credentials in environment
    if env::var("AWS_ACCESS_KEY_ID").is_err() || env::var("AWS_SECRET_ACCESS_KEY").is_err() {
        println!("Skipping test because AWS credentials not found in environment");
        return Ok(());
    }

    // Create the client
    let config = BedrockConfig::claude()
        .with_temperature(0.2)
        .with_max_tokens(500);
    
    let client = BedrockClient::new(config).await?;
    
    // Create a schema validator
    let schema_manager = McpSchemaManager::new();
    
    // Create a conversation context
    let mut context = ConversationContext::new();
    
    // Add system message with specific MCP instructions AND tool use
    context.add_system_message(
        "You MUST respond using the Model Context Protocol (MCP) format. \
        When asked about system information, you MUST use the shell tool. \
        For tool calls, use this format: \
        { \
            \"jsonrpc\": \"2.0\", \
            \"method\": \"mcp.tool_call\", \
            \"params\": { \
                \"name\": \"shell\", \
                \"parameters\": { \
                    \"command\": \"your command here\" \
                } \
            }, \
            \"id\": \"request-123\" \
        }"
    );
    
    // Add user message asking for system info
    context.add_user_message("What files are in the current directory?");
    
    // Test send_message expecting tool call
    let response = client.send_message(&context).await?;
    println!("Response: {:?}", response);
    
    // We want to check for a tool call, but the LLM might not always comply
    if !response.tool_calls.is_empty() {
        // Got a tool call - check it
        let tool_call = &response.tool_calls[0];
        
        // Should be for the shell tool
        assert_eq!(tool_call.tool, "shell", "Should use shell tool");
        
        // Should have command parameter
        assert!(tool_call.params.get("command").is_some(), "Should have command parameter");
    } else if !response.content.is_empty() {
        // Got content instead - validate it as MCP
        let json_result = serde_json::from_str::<serde_json::Value>(&response.content);
        
        if let Ok(json_value) = json_result {
            // Check if it's at least a valid JSON-RPC format
            assert!(json_value.get("jsonrpc").is_some(), "Should have jsonrpc field");
            assert!(json_value.get("id").is_some(), "Should have id field");
            
            // Could be either a result or method call
            if json_value.get("method").is_some() {
                assert_eq!(json_value["method"].as_str().unwrap(), "mcp.tool_call", 
                          "Method should be mcp.tool_call");
                
                // Validate the tool call format
                let params = json_value.get("params").expect("Should have params");
                assert!(params.get("name").is_some(), "Should have tool name");
                assert!(params.get("parameters").is_some(), "Should have tool parameters");
            }
        }
    }
    
    Ok(())
}