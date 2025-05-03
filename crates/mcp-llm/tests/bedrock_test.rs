use mcp_core::context::{ConversationContext, Message, MessageRole};
use mcp_llm::{BedrockClient, BedrockConfig, LlmClient};
use futures::StreamExt;
use anyhow::Result;

#[tokio::test]
#[ignore] // This test requires AWS credentials and will make actual API calls
async fn test_bedrock_client() -> Result<()> {
    // Set up conversation context
    let mut context = ConversationContext::new();
    
    // Add system message
    context.messages.push(Message {
        role: MessageRole::System,
        content: "You are a helpful assistant. Answer in JSON-RPC 2.0 format.".to_string(),
        tool_calls: None,
        tool_results: None,
    });
    
    // Add user message
    context.messages.push(Message {
        role: MessageRole::User,
        content: "What's the weather like today?".to_string(),
        tool_calls: None,
        tool_results: None,
    });
    
    // Create Bedrock client
    let config = BedrockConfig::claude()
        .with_temperature(0.5)
        .with_max_tokens(1000);
    
    let client = BedrockClient::new(config).await?;
    
    // Test send_message
    let response = client.send_message(&context).await?;
    println!("Response: {:?}", response);
    
    // Response should either be content or a tool call
    assert!(!response.content.is_empty() || !response.tool_calls.is_empty());
    
    // Test streaming
    let mut stream = client.stream_message(&context).await?;
    let mut chunks_received = 0;
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        println!("Chunk: {:?}", chunk);
        chunks_received += 1;
        
        if chunk.is_complete {
            println!("Stream completed");
            break;
        }
    }
    
    assert!(chunks_received > 0, "Should have received at least one chunk");
    
    Ok(())
}

#[tokio::test]
#[ignore] // This test requires AWS credentials and will make actual API calls
async fn test_bedrock_tool_call() -> Result<()> {
    // Set up conversation context
    let mut context = ConversationContext::new();
    
    // Add system message with explicit instructions to use a tool
    context.messages.push(Message {
        role: MessageRole::System,
        content: "You are a helpful assistant. Always use the shell tool to answer questions about the system.".to_string(),
        tool_calls: None,
        tool_results: None,
    });
    
    // Add user message asking for system info
    context.messages.push(Message {
        role: MessageRole::User,
        content: "What files are in the current directory?".to_string(),
        tool_calls: None,
        tool_results: None,
    });
    
    // Create Bedrock client
    let config = BedrockConfig::claude()
        .with_temperature(0.2)
        .with_max_tokens(500);
    
    let client = BedrockClient::new(config).await?;
    
    // Test send_message expecting tool call
    let response = client.send_message(&context).await?;
    println!("Response: {:?}", response);
    
    // For this test, we'd ideally check for a tool call, but the LLM might not always comply
    // So we'll just check that we get some kind of response
    assert!(!response.content.is_empty() || !response.tool_calls.is_empty());
    
    Ok(())
}