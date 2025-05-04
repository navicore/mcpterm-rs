use crate::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct ShellConfig {
    pub default_timeout_ms: u64,
    pub max_timeout_ms: u64,
    pub allowed_commands: Option<Vec<String>>,
    pub denied_commands: Option<Vec<String>>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 5000, // 5 seconds default
            max_timeout_ms: 30000,    // 30 seconds max
            allowed_commands: None,   // All commands allowed unless in denied list
            denied_commands: Some(vec![
                "rm -rf".to_string(),      // Prevent recursive force deletion
                "sudo".to_string(),        // Prevent sudo
                ":(){:|:&};:".to_string(), // Fork bomb
            ]),
        }
    }
}

#[derive(Default, Debug)]
pub struct ShellTool {
    config: ShellConfig,
}

impl ShellTool {
    pub fn new() -> Self {
        Self {
            config: ShellConfig::default(),
        }
    }

    pub fn with_config(config: ShellConfig) -> Self {
        Self { config }
    }

    // Check if a command is allowed based on configuration
    fn is_command_allowed(&self, command: &str) -> bool {
        // First check denied commands
        if let Some(denied) = &self.config.denied_commands {
            for denied_cmd in denied {
                if command.contains(denied_cmd) {
                    warn!(
                        "Command '{}' contains denied pattern: {}",
                        command, denied_cmd
                    );
                    return false;
                }
            }
        }

        // Then check allowed commands if specified
        if let Some(allowed) = &self.config.allowed_commands {
            // If we have an allowed list, command must be in it
            let is_allowed = allowed
                .iter()
                .any(|allowed_cmd| command.starts_with(allowed_cmd));

            if !is_allowed {
                warn!("Command '{}' is not in the allowed list", command);
                return false;
            }
        }

        // Command is allowed
        true
    }

    // Get the shell to use based on platform
    fn get_shell() -> (&'static str, &'static str) {
        if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        }
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "shell".to_string(),
            name: "Shell Command".to_string(),
            description: "Executes shell commands with configurable timeout".to_string(),
            category: ToolCategory::Shell,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Command timeout in milliseconds",
                        "default": 5000
                    }
                },
                "required": ["command"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "stdout": {
                        "type": "string"
                    },
                    "stderr": {
                        "type": "string"
                    },
                    "exit_code": {
                        "type": "integer"
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let command = params["command"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'command'"))?;

        // Get timeout from parameters or use default
        let timeout_ms = params["timeout"]
            .as_u64()
            .unwrap_or(self.config.default_timeout_ms);

        // Cap timeout at maximum
        let timeout_ms = timeout_ms.min(self.config.max_timeout_ms);

        // Log the command execution
        info!("Executing shell command: {}", command);
        debug!("Command timeout: {} ms", timeout_ms);

        // Check if command is allowed
        if !self.is_command_allowed(command) {
            return Ok(ToolResult {
                tool_id: "shell".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "stdout": "",
                    "stderr": "Command not allowed for security reasons",
                    "exit_code": 1
                }),
                error: Some("Command execution denied for security reasons".to_string()),
            });
        }

        // Set up the command
        let (shell, shell_arg) = Self::get_shell();
        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg)
            .arg(command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Execute with timeout
        let result = timeout(Duration::from_millis(timeout_ms), async {
            match cmd.spawn() {
                Ok(mut child) => {
                    // Capture stdout
                    let stdout = if let Some(stdout) = child.stdout.take() {
                        let mut reader = tokio::io::BufReader::new(stdout);
                        let mut buffer = Vec::new();
                        if let Err(e) = reader.read_to_end(&mut buffer).await {
                            error!("Error reading stdout: {}", e);
                            "".to_string()
                        } else {
                            String::from_utf8_lossy(&buffer).to_string()
                        }
                    } else {
                        "".to_string()
                    };

                    // Capture stderr
                    let stderr = if let Some(stderr) = child.stderr.take() {
                        let mut reader = tokio::io::BufReader::new(stderr);
                        let mut buffer = Vec::new();
                        if let Err(e) = reader.read_to_end(&mut buffer).await {
                            error!("Error reading stderr: {}", e);
                            "".to_string()
                        } else {
                            String::from_utf8_lossy(&buffer).to_string()
                        }
                    } else {
                        "".to_string()
                    };

                    // Get exit code
                    let status = match child.wait().await {
                        Ok(status) => status,
                        Err(e) => {
                            error!("Failed to wait for child process: {}", e);
                            return Err(anyhow!("Failed to wait for child process: {}", e));
                        }
                    };

                    let exit_code = status.code().unwrap_or(-1);

                    Ok((stdout, stderr, exit_code))
                }
                Err(e) => {
                    error!("Failed to spawn command: {}", e);
                    Err(anyhow!("Failed to spawn command: {}", e))
                }
            }
        })
        .await;

        // Process result
        match result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                // Command completed successfully
                debug!("Command completed with exit code: {}", exit_code);
                debug!(
                    "stdout: {} bytes, stderr: {} bytes",
                    stdout.len(),
                    stderr.len()
                );

                // Create a truncated version of stdout/stderr if too long
                let truncated_stdout = if stdout.len() > 10000 {
                    let mut s = stdout[..10000].to_string();
                    s.push_str("\n... [output truncated] ...");
                    s
                } else {
                    stdout
                };

                let truncated_stderr = if stderr.len() > 10000 {
                    let mut s = stderr[..10000].to_string();
                    s.push_str("\n... [output truncated] ...");
                    s
                } else {
                    stderr
                };

                Ok(ToolResult {
                    tool_id: "shell".to_string(),
                    status: if exit_code == 0 {
                        ToolStatus::Success
                    } else {
                        ToolStatus::Failure
                    },
                    output: json!({
                        "stdout": truncated_stdout,
                        "stderr": truncated_stderr,
                        "exit_code": exit_code
                    }),
                    error: if exit_code != 0 {
                        Some(format!(
                            "Command exited with non-zero status: {}",
                            exit_code
                        ))
                    } else {
                        None
                    },
                })
            }
            Ok(Err(e)) => {
                // Command execution failed
                error!("Command execution error: {}", e);
                Ok(ToolResult {
                    tool_id: "shell".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "stdout": "",
                        "stderr": e.to_string(),
                        "exit_code": -1
                    }),
                    error: Some(e.to_string()),
                })
            }
            Err(_) => {
                // Timeout occurred
                warn!("Command timed out after {} ms", timeout_ms);
                Ok(ToolResult {
                    tool_id: "shell".to_string(),
                    status: ToolStatus::Timeout,
                    output: json!({
                        "stdout": "",
                        "stderr": format!("Command timed out after {} ms", timeout_ms),
                        "exit_code": -1
                    }),
                    error: Some(format!("Command timed out after {} ms", timeout_ms)),
                })
            }
        }
    }
}
