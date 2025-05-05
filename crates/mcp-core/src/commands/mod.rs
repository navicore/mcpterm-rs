use serde_json::Value;
use std::fmt;

/// Represents a command that can be executed by the application.
/// This is used to implement local slash commands like /mcp list.
pub trait SlashCommand: Send + Sync {
    /// Get the name of the command (the part after the slash)
    fn name(&self) -> &str;
    
    /// Get the description of this command
    fn description(&self) -> &str;
    
    /// Get help information for this command
    fn help(&self) -> &str;
    
    /// Execute the command with the given arguments and return the result
    fn execute(&self, args: &[&str]) -> CommandResult;
}

/// Represents a command result
pub struct CommandResult {
    /// The status of the command execution
    pub status: CommandStatus,
    
    /// The output content if the command was successful
    pub content: Option<String>,
    
    /// Any error message if the command failed
    pub error: Option<String>,
    
    /// Structured output data if the command produced it
    pub data: Option<Value>,
}

/// The status of a command execution
pub enum CommandStatus {
    /// Command executed successfully
    Success,
    
    /// Command failed to execute
    Error,
    
    /// Command requires more information
    NeedsMoreInfo,
}

impl fmt::Display for CommandStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandStatus::Success => write!(f, "Success"),
            CommandStatus::Error => write!(f, "Error"),
            CommandStatus::NeedsMoreInfo => write!(f, "NeedsMoreInfo"),
        }
    }
}

impl CommandResult {
    /// Create a new successful result with content
    pub fn success(content: &str) -> Self {
        Self {
            status: CommandStatus::Success,
            content: Some(content.to_string()),
            error: None,
            data: None,
        }
    }
    
    /// Create a new successful result with content and data
    pub fn success_with_data(content: &str, data: Value) -> Self {
        Self {
            status: CommandStatus::Success,
            content: Some(content.to_string()),
            error: None,
            data: Some(data),
        }
    }
    
    /// Create a new error result
    pub fn error(error: &str) -> Self {
        Self {
            status: CommandStatus::Error,
            content: None,
            error: Some(error.to_string()),
            data: None,
        }
    }
    
    /// Create a new "needs more info" result
    pub fn needs_more_info(message: &str) -> Self {
        Self {
            status: CommandStatus::NeedsMoreInfo,
            content: Some(message.to_string()),
            error: None,
            data: None,
        }
    }
}

/// Parse an input string to determine if it's a slash command
pub fn parse_slash_command(input: &str) -> Option<(String, Vec<String>)> {
    if !input.starts_with('/') {
        return None;
    }
    
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    
    let command = parts[0].trim_start_matches('/').to_string();
    let args = parts[1..].iter().map(|s| s.to_string()).collect();
    
    Some((command, args))
}

/// Process a slash command using the given command handlers
pub fn process_slash_command(
    input: &str,
    handlers: &[Box<dyn SlashCommand>],
) -> Option<CommandResult> {
    // Try to parse the input as a slash command
    let command_parts = parse_slash_command(input)?;
    let (command_name, args) = command_parts;
    
    // Find the handler for this command
    let handler = handlers.iter().find(|h| h.name() == command_name)?;
    
    // Convert the args for the handler
    let args_refs: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
    
    // Execute the command
    let result = handler.execute(&args_refs);
    
    Some(result)
}

// Re-export sub-modules
pub mod mcp;