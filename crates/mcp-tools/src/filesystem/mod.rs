use crate::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilesystemConfig {
    pub allowed_paths: Option<Vec<String>>,
    pub denied_paths: Option<Vec<String>>,
    pub max_file_size: usize, // Maximum allowed file size in bytes
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            allowed_paths: None, // By default, allow all paths except denied
            denied_paths: Some(vec![
                // Sensitive system directories
                "/etc/".to_string(),
                "/var/".to_string(),
                "/usr/".to_string(),
                "/bin/".to_string(),
                "/sbin/".to_string(),
                // Home directory sensitive files
                "/.ssh/".to_string(),
                "/.aws/".to_string(),
                "/.config/".to_string(),
                // Windows system directories
                "C:\\Windows\\".to_string(),
                "C:\\Program Files\\".to_string(),
                "C:\\Program Files (x86)\\".to_string(),
            ]),
            max_file_size: 10 * 1024 * 1024, // 10 MB default
        }
    }
}

/// Base class for filesystem tools that provides common functionality
#[derive(Default, Debug, Clone)]
pub struct FilesystemBaseTool {
    config: FilesystemConfig,
}

impl FilesystemBaseTool {
    pub fn new() -> Self {
        Self {
            config: FilesystemConfig::default(),
        }
    }

    pub fn with_config(config: FilesystemConfig) -> Self {
        Self { config }
    }

    // Check if a path is allowed based on configuration
    fn is_path_allowed(&self, path_str: &str) -> bool {
        let path = PathBuf::from(path_str);
        let path_str = path.to_string_lossy().to_string();

        // First check denied paths
        if let Some(denied) = &self.config.denied_paths {
            for denied_path in denied {
                if path_str.contains(denied_path) {
                    warn!(
                        "Path '{}' contains denied pattern: {}",
                        path_str, denied_path
                    );
                    return false;
                }
            }
        }

        // Then check allowed paths if specified
        if let Some(allowed) = &self.config.allowed_paths {
            // If we have an allowed list, path must be in it
            let is_allowed = allowed
                .iter()
                .any(|allowed_path| path_str.starts_with(allowed_path));

            if !is_allowed {
                warn!("Path '{}' is not in the allowed list", path_str);
                return false;
            }
        }

        // Path is allowed
        true
    }
}

#[derive(Default, Debug, Clone)]
pub struct ReadFileTool {
    base: FilesystemBaseTool,
}

impl ReadFileTool {
    pub fn new() -> Self {
        Self {
            base: FilesystemBaseTool::new(),
        }
    }

    pub fn with_config(config: FilesystemConfig) -> Self {
        Self {
            base: FilesystemBaseTool::with_config(config),
        }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "file_read".to_string(),
            name: "Read File".to_string(),
            description: "Reads the contents of a file at the specified path".to_string(),
            category: ToolCategory::Filesystem,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to read"
                    }
                },
                "required": ["path"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The file content"
                    },
                    "size": {
                        "type": "integer",
                        "description": "The file size in bytes"
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'path'"))?;

        // Check if path is allowed
        if !self.base.is_path_allowed(path) {
            return Ok(ToolResult {
                tool_id: "file_read".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "Access to this path is not allowed for security reasons"
                }),
                error: Some("Access to this path is not allowed for security reasons".to_string()),
            });
        }

        // Check if file exists
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            return Ok(ToolResult {
                tool_id: "file_read".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("File not found: {}", path)
                }),
                error: Some(format!("File not found: {}", path)),
            });
        }

        // Check if it's a file (not a directory)
        if !path_buf.is_file() {
            return Ok(ToolResult {
                tool_id: "file_read".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("Not a file: {}", path)
                }),
                error: Some(format!("Not a file: {}", path)),
            });
        }

        // Check file size
        let metadata = fs::metadata(&path_buf)?;
        let file_size = metadata.len() as usize;

        if file_size > self.base.config.max_file_size {
            return Ok(ToolResult {
                tool_id: "file_read".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("File too large: {} bytes (max: {} bytes)",
                                   file_size, self.base.config.max_file_size)
                }),
                error: Some(format!(
                    "File too large: {} bytes (max: {} bytes)",
                    file_size, self.base.config.max_file_size
                )),
            });
        }

        // Read the file
        info!("Reading file: {}", path);
        match fs::read_to_string(&path_buf) {
            Ok(content) => {
                debug!("Successfully read file: {} ({} bytes)", path, content.len());

                // Truncate if the file content is too large (e.g., binary files)
                let truncated_content = if content.len() > self.base.config.max_file_size {
                    let mut trunc = content[..self.base.config.max_file_size].to_string();
                    trunc.push_str("\n... [content truncated] ...");
                    trunc
                } else {
                    content
                };

                Ok(ToolResult {
                    tool_id: "file_read".to_string(),
                    status: ToolStatus::Success,
                    output: json!({
                        "content": truncated_content,
                        "size": file_size
                    }),
                    error: None,
                })
            }
            Err(e) => {
                error!("Failed to read file {}: {}", path, e);
                Ok(ToolResult {
                    tool_id: "file_read".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("Failed to read file: {}", e)
                    }),
                    error: Some(format!("Failed to read file: {}", e)),
                })
            }
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct WriteFileTool {
    base: FilesystemBaseTool,
}

impl WriteFileTool {
    pub fn new() -> Self {
        Self {
            base: FilesystemBaseTool::new(),
        }
    }

    pub fn with_config(config: FilesystemConfig) -> Self {
        Self {
            base: FilesystemBaseTool::with_config(config),
        }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "file_write".to_string(),
            name: "Write File".to_string(),
            description: "Writes content to a file at the specified path".to_string(),
            category: ToolCategory::Filesystem,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to write the file to"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    },
                    "append": {
                        "type": "boolean",
                        "description": "Whether to append to the file or overwrite it",
                        "default": false
                    }
                },
                "required": ["path", "content"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "success": {
                        "type": "boolean",
                        "description": "Whether the write was successful"
                    },
                    "bytes_written": {
                        "type": "integer",
                        "description": "The number of bytes written"
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'path'"))?;

        let content = params["content"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'content'"))?;

        let append = params["append"].as_bool().unwrap_or(false);

        // Check if content size is allowed
        if content.len() > self.base.config.max_file_size {
            return Ok(ToolResult {
                tool_id: "file_write".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("Content too large: {} bytes (max: {} bytes)",
                                   content.len(), self.base.config.max_file_size)
                }),
                error: Some(format!(
                    "Content too large: {} bytes (max: {} bytes)",
                    content.len(),
                    self.base.config.max_file_size
                )),
            });
        }

        // Check if path is allowed
        if !self.base.is_path_allowed(path) {
            return Ok(ToolResult {
                tool_id: "file_write".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "Access to this path is not allowed for security reasons"
                }),
                error: Some("Access to this path is not allowed for security reasons".to_string()),
            });
        }

        // Create parent directories if they don't exist
        let path_buf = PathBuf::from(path);
        if let Some(parent) = path_buf.parent() {
            if !parent.exists() {
                match fs::create_dir_all(parent) {
                    Ok(_) => {
                        debug!("Created parent directories for: {}", path);
                    }
                    Err(e) => {
                        error!("Failed to create parent directories for {}: {}", path, e);
                        return Ok(ToolResult {
                            tool_id: "file_write".to_string(),
                            status: ToolStatus::Failure,
                            output: json!({
                                "error": format!("Failed to create parent directories: {}", e)
                            }),
                            error: Some(format!("Failed to create parent directories: {}", e)),
                        });
                    }
                }
            }
        }

        // Write the file
        info!("Writing to file: {} (append: {})", path, append);
        let result = if append {
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path_buf)
                .and_then(|mut file| std::io::Write::write_all(&mut file, content.as_bytes()))
        } else {
            fs::write(&path_buf, content)
        };

        match result {
            Ok(_) => {
                debug!(
                    "Successfully wrote to file: {} ({} bytes)",
                    path,
                    content.len()
                );
                Ok(ToolResult {
                    tool_id: "file_write".to_string(),
                    status: ToolStatus::Success,
                    output: json!({
                        "success": true,
                        "bytes_written": content.len()
                    }),
                    error: None,
                })
            }
            Err(e) => {
                error!("Failed to write to file {}: {}", path, e);
                Ok(ToolResult {
                    tool_id: "file_write".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("Failed to write to file: {}", e)
                    }),
                    error: Some(format!("Failed to write to file: {}", e)),
                })
            }
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct ListDirectoryTool {
    base: FilesystemBaseTool,
}

impl ListDirectoryTool {
    pub fn new() -> Self {
        Self {
            base: FilesystemBaseTool::new(),
        }
    }

    pub fn with_config(config: FilesystemConfig) -> Self {
        Self {
            base: FilesystemBaseTool::with_config(config),
        }
    }
}

#[async_trait]
impl Tool for ListDirectoryTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "directory_list".to_string(),
            name: "List Directory".to_string(),
            description: "Lists files and directories at the specified path".to_string(),
            category: ToolCategory::Filesystem,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The directory path to list"
                    }
                },
                "required": ["path"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string"
                                },
                                "path": {
                                    "type": "string"
                                },
                                "type": {
                                    "type": "string",
                                    "enum": ["file", "directory", "symlink", "other"]
                                },
                                "size": {
                                    "type": "integer"
                                }
                            }
                        }
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'path'"))?;

        // Check if path is allowed
        if !self.base.is_path_allowed(path) {
            return Ok(ToolResult {
                tool_id: "directory_list".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "Access to this path is not allowed for security reasons"
                }),
                error: Some("Access to this path is not allowed for security reasons".to_string()),
            });
        }

        // Check if directory exists
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            return Ok(ToolResult {
                tool_id: "directory_list".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("Directory not found: {}", path)
                }),
                error: Some(format!("Directory not found: {}", path)),
            });
        }

        // Check if it's a directory
        if !path_buf.is_dir() {
            return Ok(ToolResult {
                tool_id: "directory_list".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("Not a directory: {}", path)
                }),
                error: Some(format!("Not a directory: {}", path)),
            });
        }

        // Read the directory
        info!("Listing directory: {}", path);
        match fs::read_dir(&path_buf) {
            Ok(entries) => {
                let mut entry_list = Vec::new();
                for entry_result in entries {
                    match entry_result {
                        Ok(entry) => {
                            let entry_path = entry.path();
                            let file_name = entry.file_name().to_string_lossy().to_string();

                            let entry_type = if entry_path.is_file() {
                                "file"
                            } else if entry_path.is_dir() {
                                "directory"
                            } else if entry_path.is_symlink() {
                                "symlink"
                            } else {
                                "other"
                            };

                            let size = if entry_path.is_file() {
                                match fs::metadata(&entry_path) {
                                    Ok(metadata) => metadata.len() as i64,
                                    Err(_) => -1,
                                }
                            } else {
                                -1
                            };

                            entry_list.push(json!({
                                "name": file_name,
                                "path": entry_path.to_string_lossy(),
                                "type": entry_type,
                                "size": size
                            }));
                        }
                        Err(e) => {
                            warn!("Failed to read directory entry: {}", e);
                            // Skip this entry
                        }
                    }
                }

                debug!(
                    "Successfully listed directory: {} ({} entries)",
                    path,
                    entry_list.len()
                );

                Ok(ToolResult {
                    tool_id: "directory_list".to_string(),
                    status: ToolStatus::Success,
                    output: json!({
                        "entries": entry_list
                    }),
                    error: None,
                })
            }
            Err(e) => {
                error!("Failed to list directory {}: {}", path, e);
                Ok(ToolResult {
                    tool_id: "directory_list".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("Failed to list directory: {}", e)
                    }),
                    error: Some(format!("Failed to list directory: {}", e)),
                })
            }
        }
    }
}
