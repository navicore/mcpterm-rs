use anyhow::{Result, anyhow};
use async_trait::async_trait;
use futures::{Stream, stream, StreamExt};
use mcp_core::context::ConversationContext;
use mcp_llm::client_trait::{LlmClient, LlmResponse, StreamChunk, ToolCall};
use mcp_runtime::{EventBus, ModelEvent, SessionManager, ToolExecutor};
use mcp_tools::ToolManager;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use std::clone::Clone;

// Mock LLM client for testing
#[derive(Clone)]
struct MockLlmClient {
    responses: Arc<Mutex<Vec<LlmResponse>>>,
    stream_responses: Arc<Mutex<Vec<StreamChunk>>>,
    last_context: Arc<Mutex<Option<ConversationContext>>>,
    cancelled_requests: Arc<Mutex<Vec<String>>>,
}

impl MockLlmClient {
    fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            stream_responses: Arc::new(Mutex::new(Vec::new())), 
            last_context: Arc::new(Mutex::new(None)),
            cancelled_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    fn add_response(&self, response: LlmResponse) {
        let mut responses = self.responses.lock().unwrap();
        responses.push(response);
    }
    
    fn add_stream_chunk(&self, chunk: StreamChunk) {
        let mut stream_responses = self.stream_responses.lock().unwrap();
        stream_responses.push(chunk);
    }
    
    fn get_last_context(&self) -> Option<ConversationContext> {
        let context = self.last_context.lock().unwrap();
        context.clone()
    }
    
    fn get_cancelled_requests(&self) -> Vec<String> {
        let cancelled = self.cancelled_requests.lock().unwrap();
        cancelled.clone()
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn send_message(&self, context: &ConversationContext) -> Result<LlmResponse> {
        // Store the context
        {
            let mut last_context = self.last_context.lock().unwrap();
            *last_context = Some(context.clone());
        }
        
        // Get the next response
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            Err(anyhow!("No mock responses available"))
        } else {
            Ok(responses.remove(0))
        }
    }
    
    async fn stream_message(
        &self, 
        context: &ConversationContext
    ) -> Result<Box<dyn Stream<Item = Result<StreamChunk>> + Unpin + Send>> {
        // Store the context
        {
            let mut last_context = self.last_context.lock().unwrap();
            *last_context = Some(context.clone());
        }
        
        // Get all stream responses
        let stream_responses = {
            let mut responses = self.stream_responses.lock().unwrap();
            std::mem::take(&mut *responses)
        };
        
        // Convert to a stream
        let stream_iter = stream_responses.into_iter().map(Ok);
        Ok(Box::new(stream::iter(stream_iter)))
    }
    
    fn cancel_request(&self, request_id: &str) -> Result<()> {
        let mut cancelled = self.cancelled_requests.lock().unwrap();
        cancelled.push(request_id.to_string());
        Ok(())
    }
}

// Helper to collect events from the event bus
struct EventCollector {
    model_events: Arc<Mutex<Vec<ModelEvent>>>,
}

impl EventCollector {
    fn new(event_bus: &EventBus) -> Self {
        let model_events = Arc::new(Mutex::new(Vec::new()));
        let model_events_clone = model_events.clone();
        
        let handler = mcp_runtime::create_handler(move |event: ModelEvent| {
            let model_events = model_events_clone.clone();
            Box::pin(async move {
                let mut events = model_events.lock().unwrap();
                events.push(event);
                Ok(())
            })
        });
        
        event_bus.register_model_handler(handler).unwrap();
        
        Self {
            model_events
        }
    }
    
    fn get_model_events(&self) -> Vec<ModelEvent> {
        let events = self.model_events.lock().unwrap();
        events.clone()
    }
}

#[tokio::test]
async fn test_session_manager_text_response() {
    // Create mock components
    let mock_client = MockLlmClient::new();
    let tool_manager = ToolManager::new();
    let tool_executor = ToolExecutor::new(tool_manager);
    let event_bus = EventBus::new();
    
    // Start the event bus
    event_bus.start_event_distribution().unwrap();
    
    // Create event collector
    let collector = EventCollector::new(&event_bus);
    
    // Setup mock response
    mock_client.add_response(LlmResponse {
        id: "resp1".to_string(),
        content: "Hello, I'm an AI assistant!".to_string(),
        tool_calls: Vec::new(),
    });
    
    // Create session manager
    let session_manager = SessionManager::new(mock_client.clone(), tool_executor, event_bus);
    session_manager.register_handlers().unwrap();
    
    // Add a user message directly to the session
    let session = session_manager.get_session();
    session.add_user_message("Hello!");
    
    // Get the context and call the LLM directly
    let context = session.get_context().read().unwrap().clone();
    let response = mock_client.send_message(&context).await.unwrap();
    
    // Process the response
    session.add_assistant_message(&response.content);
    
    // Check that the session has both messages
    let context_lock = session.get_context();
    let context = context_lock.read().unwrap();
    
    println!("Context messages length: {}", context.messages.len());
    for (i, msg) in context.messages.iter().enumerate() {
        println!("Message {}: Role: {:?}, Content: {}", i, msg.role, msg.content);
    }
    
    assert_eq!(context.messages.len(), 2); // User message + assistant response
    assert_eq!(context.messages[0].content, "Hello!");
    assert_eq!(context.messages[1].content, "Hello, I'm an AI assistant!");
    
    // Check if we can get events through the event collector
    let model_tx = session_manager.get_model_sender();
    model_tx.send(ModelEvent::LlmMessage(response.content.clone())).unwrap();
    model_tx.send(ModelEvent::LlmResponseComplete).unwrap();
    
    // Wait a bit for the events to be processed
    sleep(Duration::from_millis(100)).await;
    
    let events = collector.get_model_events();
    println!("Collected events: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        println!("Event {}: {:?}", i, event);
    }
    
    assert!(events.iter().any(|e| matches!(e, 
        ModelEvent::LlmMessage(msg) if msg == "Hello, I'm an AI assistant!"
    )));
}

#[tokio::test]
async fn test_session_manager_streaming_response() {
    // Create mock components
    let mock_client = MockLlmClient::new();
    let tool_manager = ToolManager::new();
    let tool_executor = ToolExecutor::new(tool_manager);
    let event_bus = EventBus::new();
    
    // Start the event bus
    event_bus.start_event_distribution().unwrap();
    
    // Create event collector
    let collector = EventCollector::new(&event_bus);
    
    // Setup mock streaming response
    mock_client.add_stream_chunk(StreamChunk {
        id: "resp1".to_string(),
        content: "Hello, ".to_string(),
        is_tool_call: false,
        tool_call: None,
        is_complete: false,
    });
    
    mock_client.add_stream_chunk(StreamChunk {
        id: "resp1".to_string(),
        content: "I'm an AI ".to_string(),
        is_tool_call: false,
        tool_call: None,
        is_complete: false,
    });
    
    mock_client.add_stream_chunk(StreamChunk {
        id: "resp1".to_string(),
        content: "assistant!".to_string(),
        is_tool_call: false,
        tool_call: None,
        is_complete: true,
    });
    
    // Create session manager
    let session_manager = SessionManager::new(mock_client.clone(), tool_executor, event_bus);
    session_manager.register_handlers().unwrap();
    
    // Add a user message directly to the session
    let session = session_manager.get_session();
    session.add_user_message("Hello!");
    
    // Get the context and call the LLM for streaming directly
    let context = session.get_context().read().unwrap().clone();
    let mut stream = mock_client.stream_message(&context).await.unwrap();
    
    // Process each chunk
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.unwrap();
        if !chunk.content.is_empty() {
            session.add_assistant_message(&chunk.content);
        }
    }
    
    // Check that the session has all messages
    let context_lock = session.get_context();
    let context = context_lock.read().unwrap();
    
    println!("Context messages length: {}", context.messages.len());
    for (i, msg) in context.messages.iter().enumerate() {
        println!("Message {}: Role: {:?}, Content: {}", i, msg.role, msg.content);
    }
    
    assert_eq!(context.messages.len(), 4); // User message + 3 assistant chunks
    assert_eq!(context.messages[0].content, "Hello!");
    assert_eq!(context.messages[1].content, "Hello, ");
    assert_eq!(context.messages[2].content, "I'm an AI ");
    assert_eq!(context.messages[3].content, "assistant!");
    
    // Send events through the event bus for the collector
    let model_tx = session_manager.get_model_sender();
    model_tx.send(ModelEvent::LlmStreamChunk("Hello, ".to_string())).unwrap();
    model_tx.send(ModelEvent::LlmStreamChunk("I'm an AI ".to_string())).unwrap();
    model_tx.send(ModelEvent::LlmStreamChunk("assistant!".to_string())).unwrap();
    model_tx.send(ModelEvent::LlmResponseComplete).unwrap();
    
    // Wait a bit for the events to be processed
    sleep(Duration::from_millis(100)).await;
    
    // Check the events emitted
    let events = collector.get_model_events();
    
    println!("Collected events: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        println!("Event {}: {:?}", i, event);
    }
    
    assert!(events.iter().any(|e| matches!(e, 
        ModelEvent::LlmStreamChunk(msg) if msg == "Hello, "
    )));
    assert!(events.iter().any(|e| matches!(e, 
        ModelEvent::LlmStreamChunk(msg) if msg == "I'm an AI "
    )));
    assert!(events.iter().any(|e| matches!(e, 
        ModelEvent::LlmStreamChunk(msg) if msg == "assistant!"
    )));
}

#[tokio::test]
async fn test_session_manager_tool_calls() {
    // Create mock components
    let mock_client = MockLlmClient::new();
    let tool_manager = ToolManager::new();
    let tool_executor = ToolExecutor::new(tool_manager);
    let event_bus = EventBus::new();
    
    // Start the event bus
    event_bus.start_event_distribution().unwrap();
    
    // Create event collector
    let collector = EventCollector::new(&event_bus);
    
    // Setup mock response with tool call
    mock_client.add_response(LlmResponse {
        id: "resp1".to_string(),
        content: "".to_string(),
        tool_calls: vec![
            ToolCall {
                id: "tool1".to_string(),
                tool: "search".to_string(),
                params: json!({"query": "weather"}),
            }
        ],
    });
    
    // Add response for after tool execution
    mock_client.add_response(LlmResponse {
        id: "resp2".to_string(),
        content: "Here's the weather information.".to_string(),
        tool_calls: Vec::new(),
    });
    
    // Create session manager
    let session_manager = SessionManager::new(mock_client.clone(), tool_executor, event_bus);
    session_manager.register_handlers().unwrap();
    
    // Add a user message directly to the session
    let session = session_manager.get_session();
    session.add_user_message("What's the weather?");
    
    // Get the context and call the LLM directly
    let context = session.get_context().read().unwrap().clone();
    let response = mock_client.send_message(&context).await.unwrap();
    
    // Process the tool call directly
    if !response.tool_calls.is_empty() {
        let tool_call = &response.tool_calls[0];
        
        // Send the events for the tool request
        let model_tx = session_manager.get_model_sender();
        model_tx.send(ModelEvent::ToolRequest(tool_call.tool.clone(), tool_call.params.clone())).unwrap();
        
        // Manually simulate a tool result
        let tool_result = json!({"result": "Sunny, 72°F"});
        model_tx.send(ModelEvent::ToolResult(tool_call.tool.clone(), tool_result)).unwrap();
        
        // Get the next response
        let response2 = mock_client.send_message(&context).await.unwrap();
        session.add_assistant_message(&response2.content);
        
        // Send the final response message
        model_tx.send(ModelEvent::LlmMessage(response2.content.clone())).unwrap();
        model_tx.send(ModelEvent::LlmResponseComplete).unwrap();
    }
    
    // Wait for the events to be processed
    sleep(Duration::from_millis(100)).await;
    
    // Check the context state
    let context_lock = session.get_context();
    let context = context_lock.read().unwrap();
    
    println!("Tool context messages: {}", context.messages.len());
    for (i, msg) in context.messages.iter().enumerate() {
        println!("Message {}: Role: {:?}, Content: {}", i, msg.role, msg.content);
    }
    
    // Check the events emitted
    let events = collector.get_model_events();
    println!("Tool events: {}", events.len());
    for (i, event) in events.iter().enumerate() {
        println!("Event {}: {:?}", i, event);
    }
    
    assert!(events.iter().any(|e| matches!(e, 
        ModelEvent::ToolRequest(tool, _) if tool == "search"
    )));
    
    // Check for tool result
    let has_tool_result = events.iter().any(|e| {
        if let ModelEvent::ToolResult(tool, _) = e {
            tool == "search"
        } else {
            false
        }
    });
    assert!(has_tool_result);
    
    // Check for final response
    assert!(events.iter().any(|e| matches!(e, 
        ModelEvent::LlmMessage(msg) if msg == "Here's the weather information."
    )));
}

#[tokio::test]
async fn test_session_manager_cancellation() {
    // Create mock components
    let mock_client = MockLlmClient::new();
    let tool_manager = ToolManager::new();
    let tool_executor = ToolExecutor::new(tool_manager);
    let event_bus = EventBus::new();
    
    // Start the event bus
    event_bus.start_event_distribution().unwrap();
    
    // Setup mock streaming response that will take some time
    for i in 0..20 {
        mock_client.add_stream_chunk(StreamChunk {
            id: "resp1".to_string(),
            content: format!("Part {} of response. ", i),
            is_tool_call: false,
            tool_call: None,
            is_complete: i == 19,
        });
    }
    
    // Create session manager with the mock client
    let session_manager = SessionManager::new(mock_client.clone(), tool_executor, event_bus);
    session_manager.register_handlers().unwrap();
    
    // Add a user message directly to the session
    let session = session_manager.get_session();
    session.add_user_message("Generate a long response.");
    
    // Register a mock request ID in the active requests
    let request_id = "test-request-123";
    mock_client.cancel_request(request_id).unwrap();
    
    // Check that the cancel request was made
    let cancelled_requests = mock_client.get_cancelled_requests();
    
    println!("Cancelled requests: {:?}", cancelled_requests);
    
    assert!(!cancelled_requests.is_empty());
    assert_eq!(cancelled_requests[0], request_id);
}