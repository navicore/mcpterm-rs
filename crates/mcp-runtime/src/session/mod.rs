use crate::event_bus::{self, ApiEvent, EventBus, EventHandler, ModelEvent, UiEvent};
use crate::executor::ToolExecutor;
use anyhow::{anyhow, Result};
use futures::StreamExt;
use mcp_core::context::ConversationContext;
use mcp_core::jsonrpc::JsonRpcFilter;
use mcp_llm::client_trait::{LlmClient, LlmResponse, StreamChunk};
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
            // Clear the executed tools cache for a new user message
            // This ensures that tools can be re-executed in a new conversation turn
            crate::executor::clear_executed_tools();

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
use std::collections::{HashMap, HashSet};

pub struct SessionManager<L: LlmClient> {
    session: Arc<Session>,
    llm_client: Arc<L>,
    tool_executor: Arc<ToolExecutor>,
    event_bus: Arc<EventBus>,
    active_requests: Arc<Mutex<HashMap<String, bool>>>,
    // Track executed tool calls to prevent duplicates
    #[allow(dead_code)]
    executed_tools: Arc<Mutex<HashSet<String>>>,
}

impl<L: LlmClient + 'static> SessionManager<L> {
    // Accept an Arc<EventBus> directly - enforce the singleton pattern
    pub fn new(llm_client: L, tool_executor: ToolExecutor, event_bus: Arc<EventBus>) -> Self {
        Self {
            session: Arc::new(Session::new()),
            llm_client: Arc::new(llm_client),
            tool_executor: Arc::new(tool_executor),
            event_bus, // Already an Arc, no need to wrap again
            active_requests: Arc::new(Mutex::new(HashMap::new())),
            executed_tools: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn get_session(&self) -> Arc<Session> {
        self.session.clone()
    }

    /// Checks if a tool call has already been executed and tracks new executions
    /// Returns true if this is a new tool call that hasn't been executed yet
    #[allow(dead_code)]
    fn should_execute_tool(
        &self,
        tool_name: &str,
        tool_params: &serde_json::Value,
        id: &str,
    ) -> bool {
        // Create a unique identifier for this tool call (tool name + parameters + id)
        let tool_call_json = serde_json::json!({
            "tool": tool_name,
            "params": tool_params,
            "id": id
        });

        // Create a hash of this tool call as our tracking key
        let tool_call_str = serde_json::to_string(&tool_call_json).unwrap_or_default();
        let tool_call_hash = format!("{:x}", md5::compute(tool_call_str));

        // Check if we've seen this exact tool call before
        let mut executed_tools = self.executed_tools.lock().unwrap();
        if executed_tools.contains(&tool_call_hash) {
            debug!(
                "Skipping duplicate tool execution: {} (ID: {})",
                tool_name, id
            );
            return false;
        }

        // This is a new tool call, add it to our tracking set
        debug!("New tool execution: {} (ID: {})", tool_name, id);
        executed_tools.insert(tool_call_hash);
        true
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
        debug!(
            "Model handlers before registration: {}",
            model_handlers_count_before
        );
        self.event_bus.register_model_handler(model_handler)?;
        let model_handlers_count_after = self.event_bus.model_handlers();
        debug!(
            "Model handlers after registration: {}",
            model_handlers_count_after
        );

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
        Arc::clone(&self.event_bus)
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
                        debug!(
                            "Processing user message: {} in SessionManager model event handler",
                            message
                        );

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

                        // Create a properly formatted JSON-RPC response for the tool result
                        // The LLM expects a properly formatted JSON-RPC response for tool results

                        // Extract status as a string (ensure it's lowercase)
                        let status_str = result.get("status")
                            .and_then(|s| s.as_str())
                            .map(|s| s.to_lowercase())
                            .unwrap_or_else(|| "success".to_string());

                        let jsonrpc_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": tool_id,  // Use tool_id as the id for simplicity
                            "result": {
                                "tool_id": tool_id,
                                "status": status_str,
                                "output": result,
                                "error": result.get("error").cloned(),
                            }
                        });

                        // Convert to a properly formatted string
                        let tool_message = serde_json::to_string_pretty(&jsonrpc_response)
                            .unwrap_or_else(|_| {
                                // Fallback in case serialization fails
                                error!("Failed to serialize tool result as JSON-RPC");
                                format!("{{\"jsonrpc\":\"2.0\",\"id\":\"{}\",\"result\":{{\"tool_id\":\"{}\",\"status\":\"success\",\"output\":{{}},\"error\":null}}}}",
                                    tool_id, tool_id)
                            });

                        // Add the tool message to the session history
                        session.add_tool_message(&tool_message);

                        // Log that this message was added to help with debugging
                        debug!("Added tool message to session history: {}", tool_id);

                        // IMPORTANT: This is a tool result, not a tool request
                        // It should never be treated as a duplicate of a tool request
                        // Added to highlight the distinction in logs
                        debug!("TOOL RESULT processed (not a duplicate request): {}", tool_id);

                        // DO NOT continue the conversation with an empty user message!
                        // This causes recursive loops of tool calls.
                        // Instead, we'll wait for the LLM to send a final response.
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
        // Create a new JsonRpcFilter for this processing
        let json_filter = JsonRpcFilter::new();
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
                        // Properly format the tool result as a structured Value
                        let result_value = serde_json::json!({
                            "tool_id": tool_call.tool,
                            "status": format!("{:?}", result.status),
                            "output": result.output,
                            "error": result.error
                        });

                        // Send the result back to model
                        let _ = model_tx.send(ModelEvent::ToolResult(
                            tool_call.tool,
                            result_value,
                        ));
                    }
                    Err(e) => {
                        error!("Tool execution error: {:?}", e);
                        // Send error as a result
                        // Properly format the error as a JSON-RPC response
                        let jsonrpc_error = serde_json::json!({
                            "tool_id": tool_call.tool,
                            "status": "failure",
                            "output": {},
                            "error": e.to_string(),
                        });
                        let _ = model_tx.send(ModelEvent::ToolResult(tool_call.tool, jsonrpc_error));
                    }
                }
            }
        } else if !chunk.content.is_empty() {
            // Handle normal content
            debug!("Received content: {}", chunk.content);

            // Use the JsonRpcFilter to filter out JSON-RPC content and keep only user-facing text
            let filtered_content = json_filter.filter_json_rpc(&chunk.content);

            // Check if the content was filtered (meaning it contained JSON-RPC tool calls)
            let content_was_filtered = filtered_content != chunk.content;

            if content_was_filtered {
                debug!("Detected and filtered JSON-RPC tool calls from content");

                // Extract JSON-RPC objects to execute any tool calls
                let json_objects = mcp_core::extract_jsonrpc_objects(&chunk.content);

                for json_obj in &json_objects {
                    if let Some(method) = json_obj.get("method").and_then(|v| v.as_str()) {
                        if method == "mcp.tool_call" {
                            if let Some(params) = json_obj.get("params") {
                                if let Some(tool_name) = params.get("name").and_then(|v| v.as_str())
                                {
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
                                                // Properly format the tool result as a structured Value
                                                let status_str = match result.status {
                                                    mcp_tools::ToolStatus::Success => "success",
                                                    mcp_tools::ToolStatus::Failure => "failure",
                                                    mcp_tools::ToolStatus::Timeout => "timeout",
                                                };

                                                let result_value = serde_json::json!({
                                                    "tool_id": tool_name,
                                                    "status": status_str,
                                                    "output": result.output,
                                                    "error": result.error
                                                });

                                                // Send the result back to model
                                                let _ = model_tx.send(ModelEvent::ToolResult(
                                                    tool_name.to_string(),
                                                    result_value,
                                                ));
                                            }
                                            Err(e) => {
                                                error!("Tool execution error: {:?}", e);

                                                // Properly format the error as a JSON-RPC response
                                                let jsonrpc_error = serde_json::json!({
                                                    "tool_id": tool_name,
                                                    "status": "failure",
                                                    "output": {},
                                                    "error": e.to_string(),
                                                });

                                                let _ = model_tx.send(ModelEvent::ToolResult(
                                                    tool_name.to_string(),
                                                    jsonrpc_error,
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

            // Send the filtered content (without JSON-RPC) to the UI
            // If no filtering was needed, this will be the original content
            if !filtered_content.is_empty() {
                debug!("Sending user-facing content");

                // Check if the filtered content is a valid JSON fragment by looking for unmatched braces
                // If it contains unmatched braces, it's likely a partial JSON object that should be suppressed
                let is_malformed_json = filtered_content.contains('{') != filtered_content.contains('}')
                    || filtered_content.contains('[') != filtered_content.contains(']');

                // Also check if it's just JSON object or array start/end markers with no content
                let is_empty_json_markers = filtered_content.trim() == "{}"
                    || filtered_content.trim() == "[]"
                    || filtered_content.trim() == "},"
                    || filtered_content.trim() == "},}"
                    || filtered_content.trim() == "}"
                    || filtered_content.trim() == "]";

                if !is_malformed_json && !is_empty_json_markers {
                    // Only add properly formatted content to the conversation context
                    session.add_assistant_message(&filtered_content);
                    let _ = model_tx.send(ModelEvent::LlmStreamChunk(filtered_content));
                } else {
                    debug!("Suppressing malformed JSON fragment from context: {}", filtered_content);
                    // Send the event but don't add it to the conversation context
                    let _ = model_tx.send(ModelEvent::LlmStreamChunk(filtered_content));
                }
            }
        }

        Ok(())
    }

    // Process a full response from the LLM
    async fn process_llm_response(
        response: LlmResponse,
        session: &Session,
        model_tx: &crossbeam_channel::Sender<ModelEvent>,
        tool_executor: &ToolExecutor,
    ) -> Result<()> {
        // Create a new JsonRpcFilter for this processing
        let json_filter = JsonRpcFilter::new();
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
                        // Properly format the tool result as a structured Value
                        let result_value = serde_json::json!({
                            "tool_id": tool_call.tool,
                            "status": format!("{:?}", result.status),
                            "output": result.output,
                            "error": result.error
                        });

                        // Send the result back to model
                        let _ = model_tx.send(ModelEvent::ToolResult(
                            tool_call.tool,
                            result_value,
                        ));
                    }
                    Err(e) => {
                        error!("Tool execution error: {:?}", e);
                        // Send error as a result
                        // Properly format the error as a JSON-RPC response
                        let jsonrpc_error = serde_json::json!({
                            "tool_id": tool_call.tool,
                            "status": "failure",
                            "output": {},
                            "error": e.to_string(),
                        });
                        let _ = model_tx.send(ModelEvent::ToolResult(tool_call.tool, jsonrpc_error));
                    }
                }
            }
        } else if !response.content.is_empty() {
            // Handle normal content
            debug!("Received content: {}", response.content);

            // Use JsonRpcFilter to filter out any tool calls
            let filtered_content = json_filter.filter_json_rpc(&response.content);

            // Check if the content was filtered (contained JSON-RPC)
            let content_was_filtered = filtered_content != response.content;

            if content_was_filtered {
                debug!("Detected and filtered JSON-RPC tool calls from content");

                // Extract and process any tool calls
                let json_objects = mcp_core::extract_jsonrpc_objects(&response.content);

                for json_obj in &json_objects {
                    if let Some(method) = json_obj.get("method").and_then(|v| v.as_str()) {
                        if method == "mcp.tool_call" {
                            if let Some(params) = json_obj.get("params") {
                                if let Some(tool_name) = params.get("name").and_then(|v| v.as_str())
                                {
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
                                                // Properly format the tool result as a structured Value
                                                let status_str = match result.status {
                                                    mcp_tools::ToolStatus::Success => "success",
                                                    mcp_tools::ToolStatus::Failure => "failure",
                                                    mcp_tools::ToolStatus::Timeout => "timeout",
                                                };

                                                let result_value = serde_json::json!({
                                                    "tool_id": tool_name,
                                                    "status": status_str,
                                                    "output": result.output,
                                                    "error": result.error
                                                });

                                                // Send the result back to model
                                                let _ = model_tx.send(ModelEvent::ToolResult(
                                                    tool_name.to_string(),
                                                    result_value,
                                                ));
                                            }
                                            Err(e) => {
                                                error!("Tool execution error: {:?}", e);

                                                // Properly format the error as a JSON-RPC response
                                                let jsonrpc_error = serde_json::json!({
                                                    "tool_id": tool_name,
                                                    "status": "failure",
                                                    "output": {},
                                                    "error": e.to_string(),
                                                });

                                                let _ = model_tx.send(ModelEvent::ToolResult(
                                                    tool_name.to_string(),
                                                    jsonrpc_error,
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

            // Send the filtered content (without JSON-RPC) to the UI
            if !filtered_content.is_empty() {
                debug!("Sending user-facing content");

                // Check if the filtered content is a valid JSON fragment by looking for unmatched braces
                // If it contains unmatched braces, it's likely a partial JSON object that should be suppressed
                let is_malformed_json = filtered_content.contains('{') != filtered_content.contains('}')
                    || filtered_content.contains('[') != filtered_content.contains(']');

                // Also check if it's just JSON object or array start/end markers with no content
                let is_empty_json_markers = filtered_content.trim() == "{}"
                    || filtered_content.trim() == "[]"
                    || filtered_content.trim() == "},"
                    || filtered_content.trim() == "},}"
                    || filtered_content.trim() == "}"
                    || filtered_content.trim() == "]";

                if !is_malformed_json && !is_empty_json_markers {
                    // Only add properly formatted content to the conversation context
                    session.add_assistant_message(&filtered_content);
                    let _ = model_tx.send(ModelEvent::LlmMessage(filtered_content));
                } else {
                    debug!("Suppressing malformed JSON fragment from context: {}", filtered_content);
                    // Send the event but don't add it to the conversation context
                    let _ = model_tx.send(ModelEvent::LlmMessage(filtered_content));
                }
            }
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
