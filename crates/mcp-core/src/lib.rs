pub mod commands;
pub mod config;
pub mod context;
pub mod jsonrpc;
pub mod logging;
pub mod prompts;
pub mod protocol;

pub use commands::mcp::{McpCommand, ToolInfo, ToolProvider};
pub use commands::{
    parse_slash_command, process_slash_command, CommandResult, CommandStatus, SlashCommand,
};
pub use config::Config;
pub use context::ConversationContext;
pub use jsonrpc::extract_jsonrpc_objects;
pub use logging::tracing::{get_log_level, init_tracing};
pub use logging::{api_log, debug_log, init_debug_log, set_verbose_logging, ui_log};
pub use prompts::{PromptManager, PromptType};
pub use protocol::validation::{create_correction_prompt, validate_llm_response, ValidationResult};
pub use protocol::{create_error_response, create_response, Error, Request, Response};
