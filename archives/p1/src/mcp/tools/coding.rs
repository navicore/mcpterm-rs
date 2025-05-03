use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use crate::mcp::protocol::validation;
use crate::mcp::resources::ResourceManager;

/// Coding tool for code manipulation
pub struct CodingTool {
    /// Base directory for file operations
    base_dir: PathBuf,
}

/// Edit operation type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditOperation {
    /// Replace text in a file
    Replace,

    /// Insert text at a specific position
    Insert,

    /// Delete text from a file
    Delete,
}

/// Code edit parameters for the replace operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceParams {
    /// File path (relative to base directory)
    pub file_path: String,

    /// Text to replace
    pub old_text: String,

    /// New text to replace with
    pub new_text: String,

    /// Expected number of replacements (for safety)
    #[serde(default = "default_expected_replacements")]
    pub expected_replacements: usize,
}

/// Code edit parameters for the insert operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertParams {
    /// File path (relative to base directory)
    pub file_path: String,

    /// Position to insert at (line number, 1-based)
    pub line: usize,

    /// Column to insert at (0-based, where 0 means beginning of line)
    #[serde(default)]
    pub column: usize,

    /// Text to insert
    pub text: String,
}

/// Code edit parameters for the delete operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteParams {
    /// File path (relative to base directory)
    pub file_path: String,

    /// Start line (1-based)
    pub start_line: usize,

    /// End line (1-based, inclusive)
    pub end_line: usize,
}

/// Code edit parameters union type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EditParams {
    /// Replace operation
    Replace(ReplaceParams),

    /// Insert operation
    Insert(InsertParams),

    /// Delete operation
    Delete(DeleteParams),
}

/// Coding tool parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingParams {
    /// Operation to perform
    pub operation: EditOperation,

    /// Parameters for the operation
    #[serde(flatten)]
    pub params: EditParams,
}

/// Code edit result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingResult {
    /// Operation that was performed
    pub operation: EditOperation,

    /// File path that was edited
    pub file_path: String,

    /// Number of lines changed
    pub lines_changed: usize,

    /// Number of characters changed
    pub chars_changed: i64,
}

/// Default value for expected replacements
fn default_expected_replacements() -> usize {
    1
}

impl CodingTool {
    /// Create a new coding tool
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        Ok(Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        })
    }

    /// Validate that a file path is within the base directory
    fn validate_file_path(&self, file_path: &str) -> Result<PathBuf> {
        // Validate path component
        validation::validate_path_component(file_path)?;

        // Join with base directory
        let full_path = self.base_dir.join(file_path);

        // Check that the path is within the base directory
        validation::validate_path_within_base(&self.base_dir, &full_path)?;

        Ok(full_path)
    }

    /// Perform a replace operation
    fn replace(&self, params: ReplaceParams) -> Result<CodingResult> {
        // Validate file path
        let file_path = self.validate_file_path(&params.file_path)?;

        // Ensure the file exists
        if !file_path.exists() {
            return Err(anyhow!("File does not exist: {}", file_path.display()));
        }

        // Read file content
        let content = fs::read_to_string(&file_path)
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Perform replacement
        let new_content = content.replace(&params.old_text, &params.new_text);

        // Count replacements
        let replacements = content.matches(&params.old_text).count();

        // Check expected replacements
        if replacements != params.expected_replacements {
            return Err(anyhow!(
                "Expected {} replacements, but found {}",
                params.expected_replacements,
                replacements
            ));
        }

        // Calculate changes
        let lines_changed = params.old_text.lines().count().max(1);
        let chars_changed = params.new_text.len() as i64 - params.old_text.len() as i64;

        // Write file
        fs::write(&file_path, new_content)
            .context(format!("Failed to write file: {}", file_path.display()))?;

        Ok(CodingResult {
            operation: EditOperation::Replace,
            file_path: params.file_path,
            lines_changed,
            chars_changed,
        })
    }

    /// Perform an insert operation
    fn insert(&self, params: InsertParams) -> Result<CodingResult> {
        // Validate file path
        let file_path = self.validate_file_path(&params.file_path)?;

        // Ensure the file exists
        if !file_path.exists() {
            return Err(anyhow!("File does not exist: {}", file_path.display()));
        }

        // Read file content
        let content = fs::read_to_string(&file_path)
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Split content into lines
        let mut lines: Vec<String> = content.lines().map(String::from).collect();

        // Validate line number
        if params.line == 0 || params.line > lines.len() + 1 {
            return Err(anyhow!(
                "Invalid line number: {}, file has {} lines",
                params.line,
                lines.len()
            ));
        }

        // Handle insertion at end of file
        if params.line > lines.len() {
            lines.push(params.text.clone());
        } else {
            // Get the line to insert at (0-based index)
            let line_index = params.line - 1;
            let line = &lines[line_index];

            // Validate column
            if params.column > line.len() {
                return Err(anyhow!(
                    "Invalid column number: {}, line has {} columns",
                    params.column,
                    line.len()
                ));
            }

            // Split the line at the column
            let (before, after) = line.split_at(params.column);

            // Insert the text
            lines[line_index] = format!("{}{}{}", before, params.text, after);
        }

        // Join lines and write file
        let new_content = lines.join("\n");
        fs::write(&file_path, new_content)
            .context(format!("Failed to write file: {}", file_path.display()))?;

        Ok(CodingResult {
            operation: EditOperation::Insert,
            file_path: params.file_path,
            lines_changed: 1,
            chars_changed: params.text.len() as i64,
        })
    }

    /// Perform a delete operation
    fn delete(&self, params: DeleteParams) -> Result<CodingResult> {
        // Validate file path
        let file_path = self.validate_file_path(&params.file_path)?;

        // Ensure the file exists
        if !file_path.exists() {
            return Err(anyhow!("File does not exist: {}", file_path.display()));
        }

        // Read file content
        let content = fs::read_to_string(&file_path)
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Split content into lines
        let lines: Vec<String> = content.lines().map(String::from).collect();

        // Validate line numbers
        if params.start_line == 0 || params.start_line > lines.len() {
            return Err(anyhow!(
                "Invalid start line: {}, file has {} lines",
                params.start_line,
                lines.len()
            ));
        }

        if params.end_line == 0 || params.end_line > lines.len() {
            return Err(anyhow!(
                "Invalid end line: {}, file has {} lines",
                params.end_line,
                lines.len()
            ));
        }

        if params.start_line > params.end_line {
            return Err(anyhow!(
                "Start line ({}) must not be greater than end line ({})",
                params.start_line,
                params.end_line
            ));
        }

        // Calculate the number of lines and characters to be deleted
        let start_idx = params.start_line - 1;
        let end_idx = params.end_line;
        let lines_changed = params.end_line - params.start_line + 1;
        let chars_changed = lines[start_idx..end_idx]
            .iter()
            .map(|line| line.len() + 1) // +1 for newline
            .sum::<usize>() as i64;

        // Create new content with the lines removed
        let mut new_lines = lines[0..start_idx].to_vec();
        new_lines.extend_from_slice(&lines[end_idx..]);

        // Join lines and write file
        let new_content = new_lines.join("\n");
        fs::write(&file_path, new_content)
            .context(format!("Failed to write file: {}", file_path.display()))?;

        Ok(CodingResult {
            operation: EditOperation::Delete,
            file_path: params.file_path,
            lines_changed,
            chars_changed: -chars_changed,
        })
    }
}

impl Tool for CodingTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "coding".to_string(),
            name: "Code Manipulation".to_string(),
            description: "Manipulate code files with replace, insert, and delete operations"
                .to_string(),
            category: ToolCategory::Coding,
            input_schema: json!({
                "type": "object",
                "required": ["operation"],
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["replace", "insert", "delete"],
                        "description": "The operation to perform"
                    },
                    // Replace parameters
                    "file_path": {
                        "type": "string",
                        "description": "File path (relative to base directory)"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Text to replace (for replace operation)"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "New text to replace with (for replace operation)"
                    },
                    "expected_replacements": {
                        "type": "integer",
                        "description": "Expected number of replacements (for safety)",
                        "default": 1
                    },
                    // Insert parameters
                    "line": {
                        "type": "integer",
                        "description": "Line number to insert at (1-based) (for insert operation)"
                    },
                    "column": {
                        "type": "integer",
                        "description": "Column to insert at (0-based) (for insert operation)",
                        "default": 0
                    },
                    "text": {
                        "type": "string",
                        "description": "Text to insert (for insert operation)"
                    },
                    // Delete parameters
                    "start_line": {
                        "type": "integer",
                        "description": "Start line (1-based) (for delete operation)"
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "End line (1-based, inclusive) (for delete operation)"
                    }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["replace", "insert", "delete"],
                        "description": "The operation that was performed"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "File path that was edited"
                    },
                    "lines_changed": {
                        "type": "integer",
                        "description": "Number of lines changed"
                    },
                    "chars_changed": {
                        "type": "integer",
                        "description": "Number of characters changed (positive for additions, negative for deletions)"
                    }
                }
            }),
            metadata: None,
        }
    }

    fn execute(&self, params: Value, _resource_manager: &ResourceManager) -> Result<ToolResult> {
        // Parse parameters
        let params: CodingParams =
            serde_json::from_value(params).context("Failed to parse coding parameters")?;

        // Perform operation based on type
        let result = match params.operation {
            EditOperation::Replace => {
                if let EditParams::Replace(replace_params) = params.params {
                    self.replace(replace_params)?
                } else {
                    return Err(anyhow!("Invalid parameters for replace operation"));
                }
            }
            EditOperation::Insert => {
                if let EditParams::Insert(insert_params) = params.params {
                    self.insert(insert_params)?
                } else {
                    return Err(anyhow!("Invalid parameters for insert operation"));
                }
            }
            EditOperation::Delete => {
                if let EditParams::Delete(delete_params) = params.params {
                    self.delete(delete_params)?
                } else {
                    return Err(anyhow!("Invalid parameters for delete operation"));
                }
            }
        };

        // Create tool result
        let tool_result = ToolResult {
            tool_id: "coding".to_string(),
            status: ToolStatus::Success,
            output: serde_json::to_value(result).context("Failed to serialize coding result")?,
            error: None,
        };

        Ok(tool_result)
    }
}
