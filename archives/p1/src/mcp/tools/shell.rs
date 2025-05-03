use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

use super::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use crate::mcp::protocol::validation;
use crate::mcp::resources::ResourceManager;

/// Maximum command execution time in seconds
const MAX_COMMAND_TIMEOUT: u64 = 30;

/// Shell command execution tool
pub struct ShellTool {
    /// Base directory for file operations
    base_dir: PathBuf,

    /// Allowed commands (for security)
    allowed_commands: Vec<String>,
}

/// Shell command execution parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellParams {
    /// Command to execute
    pub command: String,

    /// Working directory (relative to base directory)
    #[serde(default)]
    pub working_dir: Option<String>,

    /// Timeout in seconds (default: 30 seconds)
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Whether to capture stderr (default: true)
    #[serde(default = "default_capture_stderr")]
    pub capture_stderr: bool,
}

/// Default timeout function
fn default_timeout() -> u64 {
    MAX_COMMAND_TIMEOUT
}

/// Default capture stderr function
fn default_capture_stderr() -> bool {
    true
}

/// Shell command execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellResult {
    /// Command that was executed
    pub command: String,

    /// Command exit code
    pub exit_code: i32,

    /// Command stdout
    pub stdout: String,

    /// Command stderr (if capture_stderr was true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,

    /// Whether the command timed out
    pub timed_out: bool,
}

impl ShellTool {
    /// Create a new shell tool
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        // Define allowed commands
        // This is a simple allow-list approach for security
        let allowed_commands = vec![
            // File system operations
            "ls".to_string(),
            "find".to_string(),
            "grep".to_string(),
            "cat".to_string(),
            "head".to_string(),
            "tail".to_string(),
            "wc".to_string(),
            "diff".to_string(),
            "file".to_string(),
            // Development tools
            "git".to_string(),
            "npm".to_string(),
            "cargo".to_string(),
            "rustc".to_string(),
            "python".to_string(),
            "python3".to_string(),
            "pip".to_string(),
            "pip3".to_string(),
            "node".to_string(),
            "yarn".to_string(),
            "javac".to_string(),
            "java".to_string(),
            "go".to_string(),
            "gcc".to_string(),
            "g++".to_string(),
            "make".to_string(),
            "cmake".to_string(),
            // Shell utilities
            "echo".to_string(),
            "pwd".to_string(),
            "which".to_string(),
            "whoami".to_string(),
            "uname".to_string(),
            "date".to_string(),
            "tee".to_string(),
            "sort".to_string(),
            "uniq".to_string(),
            "cut".to_string(),
            "sed".to_string(),
            "awk".to_string(),
            "tr".to_string(),
            "ps".to_string(),
            "curl".to_string(),
            "wget".to_string(),
        ];

        Ok(Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            allowed_commands,
        })
    }

    /// Check if a command is allowed
    fn is_command_allowed(&self, command: &str) -> bool {
        // Get the base command (before any arguments)
        let base_command = command.split_whitespace().next().unwrap_or("");

        // Check if the base command is in the allowed list
        self.allowed_commands.iter().any(|c| c == base_command)
    }
}

impl Tool for ShellTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "shell".to_string(),
            name: "Shell Command Execution".to_string(),
            description: "Execute shell commands in a controlled environment".to_string(),
            category: ToolCategory::Shell,
            input_schema: json!({
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute"
                    },
                    "working_dir": {
                        "type": ["string", "null"],
                        "description": "Working directory (relative to base directory)"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 30 seconds)",
                        "default": 30
                    },
                    "capture_stderr": {
                        "type": "boolean",
                        "description": "Whether to capture stderr (default: true)",
                        "default": true
                    }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command that was executed"
                    },
                    "exit_code": {
                        "type": "integer",
                        "description": "Command exit code"
                    },
                    "stdout": {
                        "type": "string",
                        "description": "Command stdout"
                    },
                    "stderr": {
                        "type": ["string", "null"],
                        "description": "Command stderr (if capture_stderr was true)"
                    },
                    "timed_out": {
                        "type": "boolean",
                        "description": "Whether the command timed out"
                    }
                }
            }),
            metadata: Some(json!({
                "allowed_commands": self.allowed_commands
            })),
        }
    }

    fn execute(&self, params: Value, _resource_manager: &ResourceManager) -> Result<ToolResult> {
        // Parse parameters
        let params: ShellParams =
            serde_json::from_value(params).context("Failed to parse shell parameters")?;

        // Validate parameters
        if let Err(_) = validation::validate_shell_command(&params.command) {
            return Err(anyhow!("Invalid shell command"));
        }

        // Check if command is allowed
        if !self.is_command_allowed(&params.command) {
            return Err(anyhow!("Command not allowed: {}", params.command));
        }

        // Determine working directory
        let working_dir = if let Some(dir) = params.working_dir {
            self.base_dir.join(dir)
        } else {
            self.base_dir.clone()
        };

        // Ensure working directory exists
        if !working_dir.exists() {
            return Err(anyhow!(
                "Working directory does not exist: {}",
                working_dir.display()
            ));
        }

        // Limit timeout to maximum allowed
        let timeout = Duration::from_secs(params.timeout.min(MAX_COMMAND_TIMEOUT));

        // Execute command
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(&params.command)
            .current_dir(&working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(if params.capture_stderr {
                Stdio::piped()
            } else {
                Stdio::null()
            });

        // Spawn process
        let mut child = command.spawn().context("Failed to spawn command process")?;

        // Wait for process with timeout
        let status_result = child
            .wait_timeout(timeout)
            .context("Failed to wait for command process")?;

        // Check if process timed out
        let (exit_code, timed_out) = match status_result {
            // Process completed within timeout
            Some(status) => (status.code().unwrap_or(-1), false),
            // Process timed out
            None => {
                // Kill the process
                let _ = child.kill();
                // Wait for the process to be fully terminated
                let _ = child.wait();
                // Return timeout status
                (-1, true)
            }
        };

        // Capture stdout
        let stdout = if let Some(stdout) = child.stdout.take() {
            let mut buffer = String::new();
            std::io::Read::read_to_string(&mut std::io::BufReader::new(stdout), &mut buffer)
                .context("Failed to read command stdout")?;
            buffer
        } else {
            String::new()
        };

        // Capture stderr if requested
        let stderr = if params.capture_stderr {
            if let Some(stderr) = child.stderr.take() {
                let mut buffer = String::new();
                std::io::Read::read_to_string(&mut std::io::BufReader::new(stderr), &mut buffer)
                    .context("Failed to read command stderr")?;
                Some(buffer)
            } else {
                None
            }
        } else {
            None
        };

        // Create shell result
        let shell_result = ShellResult {
            command: params.command,
            exit_code,
            stdout,
            stderr,
            timed_out,
        };

        // Create tool result
        let tool_result = ToolResult {
            tool_id: "shell".to_string(),
            status: if exit_code == 0 && !timed_out {
                ToolStatus::Success
            } else {
                ToolStatus::Error
            },
            output: serde_json::to_value(shell_result)
                .context("Failed to serialize shell result")?,
            error: if timed_out {
                Some("Command timed out".to_string())
            } else if exit_code != 0 {
                Some(format!(
                    "Command exited with non-zero status: {}",
                    exit_code
                ))
            } else {
                None
            },
        };

        Ok(tool_result)
    }
}
