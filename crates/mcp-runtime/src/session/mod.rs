use crate::event_bus::{self, ApiEvent, EventBus, EventHandler, ModelEvent, UiEvent};
use crate::executor::ToolExecutor;
use anyhow::{anyhow, Result};
use futures::StreamExt;
use mcp_core::context::ConversationContext;
use mcp_llm::client_trait::{LlmClient, LlmResponse, StreamChunk};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tracing::{debug, error};
use uuid::Uuid;

// Session manages the state of a conversation
pub struct Session {
    id: String,
    context: Arc<RwLock<ConversationContext>>,
    // Additional fields will be added as needed
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            context: Arc::new(RwLock::new(ConversationContext::new())),
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_context(&self) -> Arc<RwLock<ConversationContext>> {
        self.context.clone()
    }

    pub fn add_user_message(&self, content: &str) {
        if let Ok(mut context) = self.context.write() {
            context.add_user_message(content);
        }
    }

    pub fn add_assistant_message(&self, content: &str) {
        if let Ok(mut context) = self.context.write() {
            context.add_assistant_message(content);
        }
    }

    pub fn add_tool_message(&self, content: &str) {
        if let Ok(mut context) = self.context.write() {
            context.add_tool_message(content);
        }
    }

    pub fn reset(&self) {
        if let Ok(mut context) = self.context.write() {
            *context = ConversationContext::new();
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

// SessionManager handles interactions between the UI, model, and tools
pub struct SessionManager<L: LlmClient> {
    session: Arc<Session>,
    llm_client: Arc<L>,
    tool_executor: Arc<ToolExecutor>,
    event_bus: Arc<EventBus>,
    active_requests: Arc<Mutex<HashMap<String, bool>>>,
}

impl<L: LlmClient + 'static> SessionManager<L> {
    pub fn new(llm_client: L, tool_executor: ToolExecutor, event_bus: EventBus) -> Self {
        Self {
            session: Arc::new(Session::new()),
            llm_client: Arc::new(llm_client),
            tool_executor: Arc::new(tool_executor),
            event_bus: Arc::new(event_bus),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_session(&self) -> Arc<Session> {
        self.session.clone()
    }

    // Register all event handlers with the event bus
    pub fn register_handlers(&self) -> Result<()> {
        debug!("Registering session manager handlers with event bus");

        // UI event handler
        let ui_handler = self.create_ui_handler();
        self.event_bus.register_ui_handler(ui_handler)?;
        debug!("Registered UI event handler");

        // Model event handler
        let model_handler = self.create_model_handler();
        // Log handler counts for debugging
        let model_handlers_count_before = self.event_bus.model_handlers();
        debug!("Model handlers before registration: {}", model_handlers_count_before);
        self.event_bus.register_model_handler(model_handler)?;
        let model_handlers_count_after = self.event_bus.model_handlers();
        debug!("Model handlers after registration: {}", model_handlers_count_after);

        // API event handler
        let api_handler = self.create_api_handler();
        self.event_bus.register_api_handler(api_handler)?;
        debug!("Registered API event handler");

        debug!("Successfully registered all session manager handlers");
        Ok(())
    }

    // Get model event sender for testing
    pub fn get_model_sender(&self) -> crossbeam_channel::Sender<ModelEvent> {
        self.event_bus.model_sender()
    }

    // Get event bus instance for testing
    pub fn get_event_bus(&self) -> Arc<EventBus> {
        self.event_bus.clone()
    }

    // Create a handler for UI events
    fn create_ui_handler(&self) -> EventHandler<UiEvent> {
        let session = self.session.clone();
        let model_tx = self.event_bus.model_sender();
        let active_requests = self.active_requests.clone();

        event_bus::create_handler(move |event: UiEvent| {
            let session = session.clone();
            let model_tx = model_tx.clone();
            let active_requests = active_requests.clone();

            Box::pin(async move {
                match event {
                    UiEvent::UserInput(content) => {
                        debug!("SessionManager UI handler received user input: {}", content);
                        session.add_user_message(&content);
                        debug!("Sending ProcessUserMessage to model channel");
                        if let Err(e) = model_tx.send(ModelEvent::ProcessUserMessage(content)) {
                            error!("Failed to send ProcessUserMessage event: {}", e);
                        } else {
                            debug!("Successfully sent ProcessUserMessage to model channel");
                        }
                    }
                    UiEvent::RequestCancellation => {
                        debug!("Request cancellation received");
                        // Mark all active requests as cancelled
                        let mut requests = active_requests.lock().unwrap();
                        for (_, cancelled) in requests.iter_mut() {
                            *cancelled = true;
                        }
                    }
                    UiEvent::ClearConversation => {
                        debug!("Clearing conversation");
                        session.reset();
                        let _ = model_tx.send(ModelEvent::ResetContext);
                    }
                    // Handle other UI events as needed
                    _ => {}
                }
                Ok(())
            })
        })
    }

    // Create a handler for Model events
    fn create_model_handler(&self) -> EventHandler<ModelEvent> {
        let session = self.session.clone();
        let llm_client = self.llm_client.clone();
        let tool_executor = self.tool_executor.clone();
        let api_tx = self.event_bus.api_sender();
        let model_tx = self.event_bus.model_sender();
        let active_requests = self.active_requests.clone();

        event_bus::create_handler(move |event: ModelEvent| {
            let session = session.clone();
            let llm_client = llm_client.clone();
            let tool_executor = tool_executor.clone();
            let api_tx = api_tx.clone();
            let model_tx = model_tx.clone();
            let active_requests = active_requests.clone();

            Box::pin(async move {
                match event {
                    ModelEvent::ProcessUserMessage(message) => {
                        debug!("Processing user message: {} in SessionManager model event handler", message);

                        // Get conversation context
                        let context = match session.get_context().read() {
                            Ok(context) => context.clone(),
                            Err(_) => {
                                error!("Failed to read conversation context");
                                return Err(anyhow!("Failed to read conversation context"));
                            }
                        };

                        // Generate request ID
                        let request_id = Uuid::new_v4().to_string();

                        // Register request for possible cancellation
                        {
                            let mut requests = active_requests.lock().unwrap();
                            requests.insert(request_id.clone(), false);
                        }

                        // Decide between streaming or regular API based on config/preference
                        let use_streaming = true; // This could be a config option

                        if use_streaming {
                            // Start streaming response
                            match llm_client.stream_message(&context).await {
                                Ok(mut stream) => {
                                    while let Some(chunk_result) = stream.next().await {
                                        match chunk_result {
                                            Ok(chunk) => {
                                                // Check if request was cancelled
                                                {
                                                    let requests = active_requests.lock().unwrap();
                                                    if let Some(cancelled) =
                                                        requests.get(&request_id)
                                                    {
                                                        if *cancelled {
                                                            debug!(
                                                                "Request {} was cancelled",
                                                                request_id
                                                            );
                                                            break;
                                                        }
                                                    }
                                                }

                                                // Store is_complete flag before moving chunk
                                                let is_complete = chunk.is_complete;

                                                // Process the chunk
                                                Self::process_stream_chunk(
                                                    chunk,
                                                    &session,
                                                    &model_tx,
                                                    &tool_executor,
                                                )
                                                .await?;

                                                // If this was the completion chunk, we're done
                                                if is_complete {
                                                    let _ = model_tx
                                                        .send(ModelEvent::LlmResponseComplete);
                                                    break;
                                                }
                                            }
                                            Err(e) => {
                                                error!("Error in stream: {:?}", e);
                                                let _ = api_tx.send(ApiEvent::Error(format!(
                                                    "Stream error: {}",
                                                    e
                                                )));
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to start streaming: {:?}", e);
                                    let _ = api_tx.send(ApiEvent::Error(format!(
                                        "Failed to start streaming: {}",
                                        e
                                    )));
                                }
                            }
                        } else {
                            // Use regular response
                            match llm_client.send_message(&context).await {
                                Ok(response) => {
                                    Self::process_llm_response(
                                        response,
                                        &session,
                                        &model_tx,
                                        &tool_executor,
                                    )
                                    .await?;

                                    let _ = model_tx.send(ModelEvent::LlmResponseComplete);
                                }
                                Err(e) => {
                                    error!("Error sending message to LLM: {:?}", e);
                                    let _ =
                                        api_tx.send(ApiEvent::Error(format!("LLM error: {}", e)));
                                }
                            }
                        }

                        // Remove from active requests
                        {
                            let mut requests = active_requests.lock().unwrap();
                            requests.remove(&request_id);
                        }
                    }
                    ModelEvent::ToolResult(tool_id, result) => {
                        debug!("Received tool result from {}: {:?}", tool_id, result);

                        // Format the tool result as a message
                        let result_str = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| format!("{:?}", result));

                        let tool_message =
                            format!("Tool '{}' returned result: {}", tool_id, result_str);
                        session.add_tool_message(&tool_message);

                        // Continue the conversation with the tool result
                        let _ = model_tx.send(ModelEvent::ProcessUserMessage(String::new()));
                    }
                    ModelEvent::ResetContext => {
                        debug!("Resetting conversation context");
                        session.reset();
                    }
                    // Handle other Model events as needed
                    _ => {}
                }
                Ok(())
            })
        })
    }

    // Create a handler for API events
    fn create_api_handler(&self) -> EventHandler<ApiEvent> {
        let llm_client = self.llm_client.clone();
        let active_requests = self.active_requests.clone();

        event_bus::create_handler(move |event: ApiEvent| {
            let llm_client = llm_client.clone();
            let active_requests = active_requests.clone();

            Box::pin(async move {
                match event {
                    ApiEvent::CancelRequest(request_id) => {
                        debug!("Cancelling request: {}", request_id);

                        // Mark the request as cancelled
                        {
                            let mut requests = active_requests.lock().unwrap();
                            if let Some(cancelled) = requests.get_mut(&request_id) {
                                *cancelled = true;
                            }
                        }

                        // Try to cancel it in the LLM client
                        if let Err(e) = llm_client.cancel_request(&request_id) {
                            error!("Failed to cancel request {}: {:?}", request_id, e);
                        }
                    }
                    // Handle other API events as needed
                    _ => {}
                }
                Ok(())
            })
        })
    }

    // Process a streaming chunk from the LLM
    async fn process_stream_chunk(
        chunk: StreamChunk,
        session: &Session,
        model_tx: &crossbeam_channel::Sender<ModelEvent>,
        tool_executor: &ToolExecutor,
    ) -> Result<()> {
        debug!("Processing stream chunk: {:?}", chunk);
        if chunk.is_tool_call {
            if let Some(tool_call) = chunk.tool_call {
                debug!(
                    "Received tool call for {}: {:?}",
                    tool_call.tool, tool_call.params
                );

                // Send tool request event
                let _ = model_tx.send(ModelEvent::ToolRequest(
                    tool_call.tool.clone(),
                    tool_call.params.clone(),
                ));

                // Execute the tool
                match tool_executor
                    .execute_tool(&tool_call.tool, tool_call.params)
                    .await
                {
                    Ok(result) => {
                        // Send the result back to model
                        let _ = model_tx.send(ModelEvent::ToolResult(
                            tool_call.tool,
                            serde_json::to_value(result)?,
                        ));
                    }
                    Err(e) => {
                        error!("Tool execution error: {:?}", e);
                        // Send error as a result
                        let error_value = serde_json::json!({
                            "error": e.to_string(),
                        });
                        let _ = model_tx.send(ModelEvent::ToolResult(tool_call.tool, error_value));
                    }
                }
            }
        } else if !chunk.content.is_empty() {
            // Handle normal content
            debug!("Received content: {}", chunk.content);

            // Check if the content contains JSON-RPC tool calls
            if chunk.content.contains("\"jsonrpc\"") && chunk.content.contains("\"method\"") {
                debug!("Content may contain JSON-RPC tool call, checking for tool call structure");

                // Use the extractor to find JSON-RPC objects
                if let Ok(json_objects) = mcp_core::extract_jsonrpc_objects_with_positions(&chunk.content) {
                    let mut found_tool_call = false;

                    for (json_obj, _start, _end) in json_objects {
                        if json_obj.get("method").is_some() {
                            // This looks like a tool call, process it
                            if let Some(method) = json_obj.get("method").and_then(|v| v.as_str()) {
                                if method == "mcp.tool_call" {
                                    found_tool_call = true;
                                    debug!("Found JSON-RPC tool call in content");

                                    if let (Some(params), Some(_id)) = (json_obj.get("params"), json_obj.get("id")) {
                                        if let Some(tool_name) = params.get("name").and_then(|v| v.as_str()) {
                                            if let Some(parameters) = params.get("parameters") {
                                                debug!("Executing tool call from JSON-RPC: {}", tool_name);

                                                // Send tool request event
                                                let _ = model_tx.send(ModelEvent::ToolRequest(
                                                    tool_name.to_string(),
                                                    parameters.clone(),
                                                ));

                                                // Execute the tool
                                                match tool_executor
                                                    .execute_tool(tool_name, parameters.clone())
                                                    .await
                                                {
                                                    Ok(result) => {
                                                        // Send the result back to model
                                                        let _ = model_tx.send(ModelEvent::ToolResult(
                                                            tool_name.to_string(),
                                                            serde_json::to_value(result)?,
                                                        ));
                                                    }
                                                    Err(e) => {
                                                        error!("Tool execution error: {:?}", e);
                                                        // Send error as a result
                                                        let error_value = serde_json::json!({
                                                            "error": e.to_string(),
                                                        });
                                                        let _ = model_tx.send(ModelEvent::ToolResult(
                                                            tool_name.to_string(),
                                                            error_value
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if found_tool_call {
                        // Found and processed tool call(s)
                        // Don't add the raw JSON-RPC to the conversation
                        debug!("Processed JSON-RPC tool call, not adding to conversation");

                        // Filter out JSON-RPC and send only user-facing content
                        let filtered_content = Self::filter_jsonrpc_from_content(&chunk.content);
                        if !filtered_content.is_empty() {
                            debug!("Sending filtered user-facing content");
                            session.add_assistant_message(&filtered_content);
                            let _ = model_tx.send(ModelEvent::LlmStreamChunk(filtered_content));
                        }

                        return Ok(());
                    }
                }
            }

            // Regular content (no tool calls)
            // Update assistant message
            session.add_assistant_message(&chunk.content);

            // Send event for UI update
            let _ = model_tx.send(ModelEvent::LlmStreamChunk(chunk.content));
        }

        Ok(())
    }

    // Filter JSON-RPC objects from content, returning only the user-facing text
    fn filter_jsonrpc_from_content(content: &str) -> String {
        // Simple case - if there's no potential JSON, return as is
        if !content.contains('{') {
            return content.to_string();
        }

        // Use the splitter to extract text segments
        let split_result = mcp_core::jsonrpc::split_jsonrpc_and_text(content);

        // Combine text segments
        if !split_result.text_segments.is_empty() {
            split_result.text_segments.join("\n\n")
        } else {
            // If there are no text segments, check if the entire content is a JSON-RPC object
            // If so, return an empty string (the tool call will be executed but not displayed)
            if serde_json::from_str::<serde_json::Value>(content).is_ok() {
                String::new()
            } else {
                // If not valid JSON, return the original content
                content.to_string()
            }
        }
    }

    // Process a full response from the LLM
    async fn process_llm_response(
        response: LlmResponse,
        session: &Session,
        model_tx: &crossbeam_channel::Sender<ModelEvent>,
        tool_executor: &ToolExecutor,
    ) -> Result<()> {
        // Check for tool calls
        if !response.tool_calls.is_empty() {
            for tool_call in response.tool_calls {
                debug!(
                    "Received tool call for {}: {:?}",
                    tool_call.tool, tool_call.params
                );

                // Send tool request event
                let _ = model_tx.send(ModelEvent::ToolRequest(
                    tool_call.tool.clone(),
                    tool_call.params.clone(),
                ));

                // Execute the tool
                match tool_executor
                    .execute_tool(&tool_call.tool, tool_call.params)
                    .await
                {
                    Ok(result) => {
                        // Send the result back to model
                        let _ = model_tx.send(ModelEvent::ToolResult(
                            tool_call.tool,
                            serde_json::to_value(result)?,
                        ));
                    }
                    Err(e) => {
                        error!("Tool execution error: {:?}", e);
                        // Send error as a result
                        let error_value = serde_json::json!({
                            "error": e.to_string(),
                        });
                        let _ = model_tx.send(ModelEvent::ToolResult(tool_call.tool, error_value));
                    }
                }
            }
        } else if !response.content.is_empty() {
            // Handle normal content
            debug!("Received content: {}", response.content);

            // Check if the content contains JSON-RPC tool calls
            if response.content.contains("\"jsonrpc\"") && response.content.contains("\"method\"") {
                debug!("Content may contain JSON-RPC tool call, checking for tool call structure");

                // Use the extractor to find JSON-RPC objects
                if let Ok(json_objects) = mcp_core::extract_jsonrpc_objects_with_positions(&response.content) {
                    let mut found_tool_call = false;

                    for (json_obj, _start, _end) in json_objects {
                        if json_obj.get("method").is_some() {
                            // This looks like a tool call, process it
                            if let Some(method) = json_obj.get("method").and_then(|v| v.as_str()) {
                                if method == "mcp.tool_call" {
                                    found_tool_call = true;
                                    debug!("Found JSON-RPC tool call in content");

                                    if let (Some(params), Some(_id)) = (json_obj.get("params"), json_obj.get("id")) {
                                        if let Some(tool_name) = params.get("name").and_then(|v| v.as_str()) {
                                            if let Some(parameters) = params.get("parameters") {
                                                debug!("Executing tool call from JSON-RPC: {}", tool_name);

                                                // Send tool request event
                                                let _ = model_tx.send(ModelEvent::ToolRequest(
                                                    tool_name.to_string(),
                                                    parameters.clone(),
                                                ));

                                                // Execute the tool
                                                match tool_executor
                                                    .execute_tool(tool_name, parameters.clone())
                                                    .await
                                                {
                                                    Ok(result) => {
                                                        // Send the result back to model
                                                        let _ = model_tx.send(ModelEvent::ToolResult(
                                                            tool_name.to_string(),
                                                            serde_json::to_value(result)?,
                                                        ));
                                                    }
                                                    Err(e) => {
                                                        error!("Tool execution error: {:?}", e);
                                                        // Send error as a result
                                                        let error_value = serde_json::json!({
                                                            "error": e.to_string(),
                                                        });
                                                        let _ = model_tx.send(ModelEvent::ToolResult(
                                                            tool_name.to_string(),
                                                            error_value
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if found_tool_call {
                        // Found and processed tool call(s)
                        // Don't add the raw JSON-RPC to the conversation
                        debug!("Processed JSON-RPC tool call, not adding to conversation");

                        // Filter out JSON-RPC and send only user-facing content
                        let filtered_content = Self::filter_jsonrpc_from_content(&response.content);
                        if !filtered_content.is_empty() {
                            debug!("Sending filtered user-facing content");
                            session.add_assistant_message(&filtered_content);
                            let _ = model_tx.send(ModelEvent::LlmMessage(filtered_content));
                        }

                        return Ok(());
                    }
                }
            }

            // Regular content (no tool calls)
            // Update assistant message
            session.add_assistant_message(&response.content);

            // Send event for UI update
            let _ = model_tx.send(ModelEvent::LlmMessage(response.content));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new();
        session.add_user_message("Hello");

        let context = session.get_context();
        let context_read = context.read().unwrap();
        assert_eq!(context_read.messages.len(), 1);
    }
}
