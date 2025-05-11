use anyhow::{anyhow, Result};
use mcp_core::commands::mcp::{ToolInfo, ToolProvider};
use mcp_llm::{BedrockClient, BedrockConfig, LlmClient};
use mcp_runtime::{EventBus, SessionManager, ToolExecutor};
use mcp_tools::ToolManager;
use std::sync::Arc;
use tracing::debug;

use crate::event_adapter::CliEventAdapter;

#[derive(Debug, Clone)]
pub struct CliSessionConfig {
    pub model: String,
    pub use_mcp: bool,
    pub region: Option<String>,
    pub streaming: bool,
    pub enable_tools: bool,
    pub require_tool_confirmation: bool,
    pub auto_approve_tools: bool,
    pub interactive: bool,
}

impl Default for CliSessionConfig {
    fn default() -> Self {
        Self {
            model: "us.anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
            use_mcp: true,
            region: None,
            streaming: true,
            enable_tools: true,
            require_tool_confirmation: false,
            auto_approve_tools: false,
            interactive: false,
        }
    }
}

pub struct CliSession<L: LlmClient + 'static> {
    config: CliSessionConfig,
    session_manager: Option<Arc<SessionManager<L>>>,
    event_adapter: Option<CliEventAdapter>,
    tool_manager: Arc<ToolManager>,
}

// Implementation for any LLM client
impl<L: LlmClient + 'static> Clone for CliSession<L> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            session_manager: self.session_manager.clone(),
            event_adapter: None, // The adapter can't be cloned
            tool_manager: self.tool_manager.clone(), // Now safe to clone as it's Arc
        }
    }
}

impl<L: LlmClient + 'static> CliSession<L> {
    pub fn new(config: CliSessionConfig) -> Self {
        // Create a new tool manager with the same configuration as the original CliApp
        let tool_manager = Self::create_tool_manager(&config);

        Self {
            config,
            session_manager: None,
            event_adapter: None,
            tool_manager,
        }
    }

    // Create a tool manager with the standard tools
    fn create_tool_manager(config: &CliSessionConfig) -> Arc<ToolManager> {
        if config.enable_tools {
            // Use the ToolFactory to create a shared tool manager with standard tools
            mcp_runtime::ToolFactory::create_shared_tool_manager()
        } else {
            // If tools are disabled, return an empty tool manager
            Arc::new(ToolManager::new())
        }
    }

    // Set a specific LLM client (used for testing)
    pub fn with_llm_client(mut self, client: L) -> Self {
        let event_bus = EventBus::new();
        let tool_executor = self.create_tool_executor();

        // Create the session manager with the client
        let session_manager = SessionManager::new(client, tool_executor, event_bus);

        // Store it - we know the type is correct here
        self.session_manager = Some(Arc::new(session_manager));

        self
    }

    // Create a tool executor that will handle tool execution with user confirmation
    fn create_tool_executor(&self) -> ToolExecutor {
        // In a complete implementation, we would create a specialized wrapper tool manager
        // that includes confirmation logic for each tool.

        // Pass our shared tool manager to the ToolExecutor
        mcp_runtime::executor::ToolFactory::create_executor_with_shared_manager(
            self.tool_manager.clone(),
        )
    }

    // Initialize the session with the event bus and Session Manager
    pub async fn initialize(&mut self) -> Result<()> {
        // If the session manager is already initialized (e.g., from with_llm_client), skip init
        if self.session_manager.is_some() {
            debug!("Session manager already initialized");

            // Get the event bus from the session manager
            let event_bus = self.session_manager.as_ref().unwrap().get_event_bus();

            // Create the event adapter with the event bus (sharing the same event bus)
            let event_adapter = CliEventAdapter::new((*event_bus).clone(), self.config.interactive);

            // Get the model sender from the session manager for direct communication
            let model_sender = self.session_manager.as_ref().unwrap().get_model_sender();
            event_adapter.set_direct_model_sender(model_sender);

            // Register handlers and start event distribution
            event_adapter.register_handlers()?;
            event_bus.start_event_distribution()?;

            self.event_adapter = Some(event_adapter);

            return Ok(());
        }

        // Create BedrockConfig
        let mut bedrock_config = BedrockConfig::new(&self.config.model)
            .with_max_tokens(4096)
            .with_temperature(0.7);

        // Add region if provided
        if let Some(region) = &self.config.region {
            debug!("Using AWS region: {}", region);
            bedrock_config = bedrock_config.with_region(region.clone());
        }

        // Add a system prompt based on whether MCP is enabled
        let system_prompt = if self.config.use_mcp {
            "You are Claude, a helpful AI assistant by Anthropic. You will follow the Model Context Protocol (MCP) for structured communication.".to_string()
        } else {
            "You are Claude, a helpful AI assistant by Anthropic.".to_string()
        };

        bedrock_config = bedrock_config.with_system_prompt(system_prompt);

        // Create a single event bus for everything - wrapped in Arc for sharing
        let event_bus = Arc::new(EventBus::new());

        // Create the tool executor
        let tool_executor = self.create_tool_executor();

        // Create the Bedrock client
        debug!("Creating BedrockClient");

        let client = if self.config.use_mcp {
            // Generate tool documentation
            let tools_doc = self.tool_manager.generate_tool_documentation();
            debug!(
                "Generated tool documentation with {} characters",
                tools_doc.len()
            );

            // Create client with tool documentation
            match BedrockClient::with_tool_documentation(bedrock_config, tools_doc).await {
                Ok(client) => {
                    debug!("Successfully created BedrockClient with dynamic tool documentation");
                    client
                }
                Err(e) => {
                    debug!("Failed to create BedrockClient: {}", e);
                    return Err(anyhow!("Error connecting to AWS Bedrock: {}", e));
                }
            }
        } else {
            // Create client without tool documentation for non-MCP mode
            match BedrockClient::new(bedrock_config).await {
                Ok(client) => {
                    debug!("Successfully created BedrockClient");
                    client
                }
                Err(e) => {
                    debug!("Failed to create BedrockClient: {}", e);
                    return Err(anyhow!("Error connecting to AWS Bedrock: {}", e));
                }
            }
        };

        // Create a session manager with a reference to our shared event bus
        let session_manager = SessionManager::new(client, tool_executor, (*event_bus).clone());

        // Create the event adapter with our shared event bus
        let event_adapter = CliEventAdapter::new((*event_bus).clone(), self.config.interactive);

        // Clear any existing handlers from previous runs to avoid duplicates
        debug!("Clearing any existing handlers before registration");
        event_bus.clear_handlers()?;

        // Register all handlers in sequence
        debug!("Registering session manager handlers");
        session_manager.register_handlers()?;

        debug!("Registering event adapter handlers");
        event_adapter.register_handlers()?;

        // Get model sender from session manager for direct communication
        let model_sender = session_manager.get_model_sender();
        debug!("Got model sender from session manager");

        // Store the model sender in the event adapter for direct communication
        event_adapter.set_direct_model_sender(model_sender);
        debug!("Set direct model sender in event adapter");

        // Verify handler registration
        debug!(
            "Verification: UI handlers: {}, Model handlers: {}, API handlers: {}",
            event_bus.ui_handlers(),
            event_bus.model_handlers(),
            event_bus.api_handlers()
        );

        // Start event distribution once for our shared event bus
        event_bus.start_event_distribution()?;

        // Store the components - this requires an unsafe cast since the types don't match
        // In a proper implementation, we'd use better generics or a trait object approach
        let session_manager_arc = Arc::new(session_manager);
        self.session_manager = Some(unsafe {
            std::mem::transmute::<Arc<SessionManager<BedrockClient>>, Arc<SessionManager<L>>>(
                session_manager_arc,
            )
        });
        self.event_adapter = Some(event_adapter);

        Ok(())
    }

    // Run a single message through the event bus
    pub async fn run(&mut self, input: &str) -> Result<String> {
        if self.session_manager.is_none() || self.event_adapter.is_none() {
            return Err(anyhow!("Session not initialized. Call initialize() first."));
        }

        let event_adapter = self.event_adapter.as_ref().unwrap();

        // Send the user message through the event adapter
        event_adapter.send_user_message(input)?;

        // Wait for a response (with a reasonable timeout)
        // In interactive mode, responses are already printed by the event adapter
        let response = event_adapter.wait_for_response(60)?;

        Ok(response)
    }

    // Get the current session for inspection
    pub fn get_session(&self) -> Option<Arc<mcp_runtime::Session>> {
        self.session_manager.as_ref().map(|sm| sm.get_session())
    }

    // Helper method to request cancellation
    pub fn cancel_request(&self) -> Result<()> {
        if let Some(event_adapter) = &self.event_adapter {
            event_adapter.request_cancellation()?;
        }
        Ok(())
    }

    // Helper method to clear conversation
    pub fn clear_conversation(&self) -> Result<()> {
        if let Some(event_adapter) = &self.event_adapter {
            event_adapter.clear_conversation()?;
        }
        Ok(())
    }
}

// Implement ToolProvider for the CliSession
impl<L: LlmClient + 'static> ToolProvider for CliSession<L> {
    fn get_tools(&self) -> Vec<ToolInfo> {
        // Convert ToolMetadata to ToolInfo
        self.tool_manager
            .get_tools()
            .iter()
            .map(|tool| ToolInfo {
                id: tool.id.clone(),
                name: tool.name.clone(),
                description: tool.description.clone(),
                category: "core".to_string(), // Default category
                input_schema: tool.input_schema.clone(),
                output_schema: tool.output_schema.clone(),
            })
            .collect()
    }

    fn get_tool_details(&self, tool_id: &str) -> Option<ToolInfo> {
        // Find the tool by ID and convert to ToolInfo
        self.tool_manager
            .get_tools()
            .iter()
            .find(|t| t.id == tool_id)
            .map(|tool| ToolInfo {
                id: tool.id.clone(),
                name: tool.name.clone(),
                description: tool.description.clone(),
                category: "core".to_string(), // Default category
                input_schema: tool.input_schema.clone(),
                output_schema: tool.output_schema.clone(),
            })
    }
}
