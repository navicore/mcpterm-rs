use crate::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Represents the diff output format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffFormat {
    /// Unified diff format (e.g., git diff)
    Unified,
    /// Side-by-side/inline diff format
    Inline,
    /// Just the changed lines
    Changes,
}

impl From<&str> for DiffFormat {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "unified" => DiffFormat::Unified,
            "inline" | "side-by-side" => DiffFormat::Inline,
            "changes" => DiffFormat::Changes,
            _ => DiffFormat::Unified, // Default
        }
    }
}

/// Configuration for the DiffTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffConfig {
    /// Default number of context lines
    pub default_context_lines: usize,
    /// Maximum file size to compare in bytes
    pub max_file_size: usize,
    /// Paths that are not allowed to be accessed
    pub denied_paths: Option<Vec<String>>,
    /// Paths that are allowed to be accessed (if specified, overrides denied_paths)
    pub allowed_paths: Option<Vec<String>>,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            default_context_lines: 3,
            max_file_size: 10 * 1024 * 1024, // 10 MB
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
            allowed_paths: None,
        }
    }
}

/// Represents a changed line in the diff
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiffLine {
    /// Original line number (0 for insertions)
    old_line_num: usize,
    /// New line number (0 for deletions)
    new_line_num: usize,
    /// Type of change
    change_type: String,
    /// The line content
    content: String,
}

/// Statistics about the differences
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiffStats {
    /// Number of inserted lines
    inserted: usize,
    /// Number of deleted lines
    deleted: usize,
    /// Number of modified lines (counted as both insert and delete)
    modified: usize,
    /// Number of unchanged lines
    unchanged: usize,
}

/// The DiffTool for comparing files or strings
#[derive(Debug, Clone)]
pub struct DiffTool {
    config: DiffConfig,
}

impl DiffTool {
    pub fn new() -> Self {
        Self {
            config: DiffConfig::default(),
        }
    }

    pub fn with_config(config: DiffConfig) -> Self {
        Self { config }
    }

    // Check if a path is allowed based on configuration
    fn is_path_allowed(&self, path_str: &str) -> bool {
        let path = PathBuf::from(path_str);
        let path_str = path.to_string_lossy().to_string();

        info!("Checking if path is allowed: '{}'", path_str);

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
            info!("Checking against allowed paths: {:?}", allowed);
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

    // Read file contents safely with size check
    fn read_file(&self, path: &Path) -> Result<String> {
        // Check file size
        let metadata = fs::metadata(path)?;
        if metadata.len() as usize > self.config.max_file_size {
            return Err(anyhow!(
                "File is too large to diff (max size: {} bytes)",
                self.config.max_file_size
            ));
        }

        // Read file content
        Ok(fs::read_to_string(path)?)
    }

    // Generate a unified diff format (similar to git diff)
    fn format_unified_diff<'a>(
        &self,
        diff: &'a TextDiff<'a, 'a, 'a, str>,
        context_lines: usize,
    ) -> (String, DiffStats) {
        let mut output = String::new();
        let mut stats = DiffStats {
            inserted: 0,
            deleted: 0,
            modified: 0,
            unchanged: 0,
        };

        let mut current_hunk_header = String::new();
        let mut current_hunk_lines = Vec::new();
        let mut old_line_num: usize = 0;
        let mut new_line_num: usize = 0;
        let mut hunk_old_start: usize = 0;
        let mut hunk_new_start: usize = 0;
        let mut hunk_old_count: usize = 0;
        let mut hunk_new_count: usize = 0;
        let mut in_hunk = false;
        let mut context_buffer = Vec::new();

        // Process each change
        for change in diff.iter_all_changes() {
            let tag = change.tag();
            let value = change.value();

            match tag {
                ChangeTag::Delete => {
                    old_line_num += 1;
                    stats.deleted += 1;

                    // Start a new hunk if needed
                    if !in_hunk {
                        hunk_old_start = old_line_num.saturating_sub(context_lines);
                        hunk_new_start = new_line_num.saturating_sub(context_lines);

                        // Add context lines from buffer
                        for (i, line) in context_buffer.iter().enumerate() {
                            let ctx_old_num = old_line_num - context_buffer.len() + i;
                            let ctx_new_num = new_line_num - context_buffer.len() + i;
                            if ctx_old_num >= hunk_old_start {
                                current_hunk_lines.push(format!(" {}", line));
                                hunk_old_count += 1;
                                hunk_new_count += 1;
                            }
                        }

                        in_hunk = true;
                        context_buffer.clear();
                    }

                    current_hunk_lines.push(format!("-{}", value));
                    hunk_old_count += 1;
                }
                ChangeTag::Insert => {
                    new_line_num += 1;
                    stats.inserted += 1;

                    // Start a new hunk if needed
                    if !in_hunk {
                        hunk_old_start = old_line_num.saturating_sub(context_lines);
                        hunk_new_start = new_line_num.saturating_sub(context_lines);

                        // Add context lines from buffer
                        for (i, line) in context_buffer.iter().enumerate() {
                            let ctx_old_num = old_line_num - context_buffer.len() + i;
                            let ctx_new_num = new_line_num - context_buffer.len() + i;
                            if ctx_new_num >= hunk_new_start {
                                current_hunk_lines.push(format!(" {}", line));
                                hunk_old_count += 1;
                                hunk_new_count += 1;
                            }
                        }

                        in_hunk = true;
                        context_buffer.clear();
                    }

                    current_hunk_lines.push(format!("+{}", value));
                    hunk_new_count += 1;
                }
                ChangeTag::Equal => {
                    old_line_num += 1;
                    new_line_num += 1;
                    stats.unchanged += 1;

                    // Add to context buffer
                    context_buffer.push(value.to_string());
                    if context_buffer.len() > context_lines * 2 {
                        context_buffer.remove(0);
                    }

                    if in_hunk {
                        // Add to current hunk if we're in one
                        current_hunk_lines.push(format!(" {}", value));
                        hunk_old_count += 1;
                        hunk_new_count += 1;

                        // Check if we've had enough unchanged lines to end the hunk
                        let unchanged_streak = current_hunk_lines
                            .iter()
                            .rev()
                            .take(context_lines)
                            .filter(|l| l.starts_with(' '))
                            .count();

                        if unchanged_streak >= context_lines {
                            // Finish the hunk
                            current_hunk_header = format!(
                                "@@ -{},{} +{},{} @@",
                                hunk_old_start, hunk_old_count, hunk_new_start, hunk_new_count
                            );

                            output.push_str(&current_hunk_header);
                            output.push('\n');
                            for line in &current_hunk_lines {
                                output.push_str(line);
                                output.push('\n');
                            }

                            // Reset hunk tracking
                            current_hunk_lines.clear();
                            in_hunk = false;
                            hunk_old_count = 0;
                            hunk_new_count = 0;
                        }
                    }
                }
            }
        }

        // Handle any remaining hunk
        if in_hunk && !current_hunk_lines.is_empty() {
            current_hunk_header = format!(
                "@@ -{},{} +{},{} @@",
                hunk_old_start, hunk_old_count, hunk_new_start, hunk_new_count
            );

            output.push_str(&current_hunk_header);
            output.push('\n');
            for line in &current_hunk_lines {
                output.push_str(line);
                output.push('\n');
            }
        }

        // Compute modified count (lines that are both deleted and inserted)
        // This is a simplistic approach; for a perfect count we'd need to do more analysis
        stats.modified = std::cmp::min(stats.deleted, stats.inserted);

        (output, stats)
    }

    // Generate inline/side-by-side diff format
    fn format_inline_diff<'a>(
        &self,
        diff: &'a TextDiff<'a, 'a, 'a, str>,
    ) -> (Vec<DiffLine>, DiffStats) {
        let mut lines = Vec::new();
        let mut stats = DiffStats {
            inserted: 0,
            deleted: 0,
            modified: 0,
            unchanged: 0,
        };

        let mut old_line_num: usize = 0;
        let mut new_line_num: usize = 0;

        for change in diff.iter_all_changes() {
            let tag = change.tag();
            let value = change.value();

            match tag {
                ChangeTag::Delete => {
                    old_line_num += 1;
                    stats.deleted += 1;
                    lines.push(DiffLine {
                        old_line_num,
                        new_line_num: 0, // No new line for deletion
                        change_type: "delete".to_string(),
                        content: value.to_string(),
                    });
                }
                ChangeTag::Insert => {
                    new_line_num += 1;
                    stats.inserted += 1;
                    lines.push(DiffLine {
                        old_line_num: 0, // No old line for insertion
                        new_line_num,
                        change_type: "insert".to_string(),
                        content: value.to_string(),
                    });
                }
                ChangeTag::Equal => {
                    old_line_num += 1;
                    new_line_num += 1;
                    stats.unchanged += 1;
                    lines.push(DiffLine {
                        old_line_num,
                        new_line_num,
                        change_type: "equal".to_string(),
                        content: value.to_string(),
                    });
                }
            }
        }

        // Compute modified count
        stats.modified = std::cmp::min(stats.deleted, stats.inserted);

        (lines, stats)
    }

    // Generate changes-only diff format (just the lines that changed)
    fn format_changes_diff<'a>(
        &self,
        diff: &'a TextDiff<'a, 'a, 'a, str>,
    ) -> (Vec<DiffLine>, DiffStats) {
        let mut lines = Vec::new();
        let mut stats = DiffStats {
            inserted: 0,
            deleted: 0,
            modified: 0,
            unchanged: 0,
        };

        let mut old_line_num: usize = 0;
        let mut new_line_num: usize = 0;

        for change in diff.iter_all_changes() {
            let tag = change.tag();
            let value = change.value();

            match tag {
                ChangeTag::Delete => {
                    old_line_num += 1;
                    stats.deleted += 1;
                    lines.push(DiffLine {
                        old_line_num,
                        new_line_num: 0,
                        change_type: "delete".to_string(),
                        content: value.to_string(),
                    });
                }
                ChangeTag::Insert => {
                    new_line_num += 1;
                    stats.inserted += 1;
                    lines.push(DiffLine {
                        old_line_num: 0,
                        new_line_num,
                        change_type: "insert".to_string(),
                        content: value.to_string(),
                    });
                }
                ChangeTag::Equal => {
                    old_line_num += 1;
                    new_line_num += 1;
                    stats.unchanged += 1;
                    // Skip equal lines in changes-only format
                }
            }
        }

        // Compute modified count
        stats.modified = std::cmp::min(stats.deleted, stats.inserted);

        (lines, stats)
    }
}

#[async_trait]
impl Tool for DiffTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "diff".to_string(),
            name: "Diff".to_string(),
            description: "Compare two files or text strings and show differences".to_string(),
            category: ToolCategory::Utility,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "old_content": {
                        "type": "string",
                        "description": "Original content string to compare"
                    },
                    "new_content": {
                        "type": "string",
                        "description": "New content string to compare"
                    },
                    "old_file": {
                        "type": "string",
                        "description": "Path to original file (alternative to old_content)"
                    },
                    "new_file": {
                        "type": "string",
                        "description": "Path to new file (alternative to new_content)"
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Number of context lines around changes",
                        "default": 3
                    },
                    "ignore_whitespace": {
                        "type": "boolean",
                        "description": "Whether to ignore whitespace in comparison",
                        "default": false
                    },
                    "output_format": {
                        "type": "string",
                        "description": "Output format (unified, inline, changes)",
                        "enum": ["unified", "inline", "changes"],
                        "default": "unified"
                    }
                },
                "oneOf": [
                    {
                        "required": ["old_content", "new_content"]
                    },
                    {
                        "required": ["old_file", "new_file"]
                    },
                    {
                        "required": ["old_content", "new_file"]
                    },
                    {
                        "required": ["old_file", "new_content"]
                    }
                ]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "diff": {
                        "oneOf": [
                            {
                                "type": "string",
                                "description": "Unified diff output (when format is 'unified')"
                            },
                            {
                                "type": "array",
                                "description": "Line-by-line diff (when format is 'inline' or 'changes')",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "old_line_num": {"type": "integer"},
                                        "new_line_num": {"type": "integer"},
                                        "change_type": {"type": "string"},
                                        "content": {"type": "string"}
                                    }
                                }
                            }
                        ]
                    },
                    "stats": {
                        "type": "object",
                        "properties": {
                            "inserted": {"type": "integer"},
                            "deleted": {"type": "integer"},
                            "modified": {"type": "integer"},
                            "unchanged": {"type": "integer"}
                        }
                    },
                    "files_compared": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let old_content = params["old_content"].as_str();
        let new_content = params["new_content"].as_str();
        let old_file = params["old_file"].as_str();
        let new_file = params["new_file"].as_str();

        // Ensure we have either content or file paths
        if old_content.is_none() && old_file.is_none() {
            return Ok(ToolResult {
                tool_id: "diff".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "You must provide either 'old_content' or 'old_file'"
                }),
                error: Some("You must provide either 'old_content' or 'old_file'".to_string()),
            });
        }

        if new_content.is_none() && new_file.is_none() {
            return Ok(ToolResult {
                tool_id: "diff".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "You must provide either 'new_content' or 'new_file'"
                }),
                error: Some("You must provide either 'new_content' or 'new_file'".to_string()),
            });
        }

        // Get additional parameters
        let context_lines = params["context_lines"]
            .as_u64()
            .unwrap_or(self.config.default_context_lines as u64)
            as usize;

        let ignore_whitespace = params["ignore_whitespace"].as_bool().unwrap_or(false);

        let output_format = params["output_format"]
            .as_str()
            .map(DiffFormat::from)
            .unwrap_or(DiffFormat::Unified);

        // Prepare the content strings and track file paths for output
        let mut old_content_str = String::new();
        let mut new_content_str = String::new();
        let mut files_compared = Vec::new();

        // Get old content
        if let Some(content) = old_content {
            old_content_str = content.to_string();
        } else if let Some(path) = old_file {
            // Check if path is allowed
            if !self.is_path_allowed(path) {
                return Ok(ToolResult {
                    tool_id: "diff".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": "Access to this path is not allowed for security reasons"
                    }),
                    error: Some(
                        "Access to this path is not allowed for security reasons".to_string(),
                    ),
                });
            }

            let file_path = PathBuf::from(path);

            // Validate path exists
            if !file_path.exists() {
                return Ok(ToolResult {
                    tool_id: "diff".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("File does not exist: {}", path)
                    }),
                    error: Some(format!("File does not exist: {}", path)),
                });
            }

            // Read file
            match self.read_file(&file_path) {
                Ok(content) => {
                    old_content_str = content;
                    files_compared.push(path.to_string());
                }
                Err(e) => {
                    return Ok(ToolResult {
                        tool_id: "diff".to_string(),
                        status: ToolStatus::Failure,
                        output: json!({
                            "error": format!("Failed to read file '{}': {}", path, e)
                        }),
                        error: Some(format!("Failed to read file '{}': {}", path, e)),
                    });
                }
            }
        }

        // Get new content
        if let Some(content) = new_content {
            new_content_str = content.to_string();
        } else if let Some(path) = new_file {
            // Check if path is allowed
            if !self.is_path_allowed(path) {
                return Ok(ToolResult {
                    tool_id: "diff".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": "Access to this path is not allowed for security reasons"
                    }),
                    error: Some(
                        "Access to this path is not allowed for security reasons".to_string(),
                    ),
                });
            }

            let file_path = PathBuf::from(path);

            // Validate path exists
            if !file_path.exists() {
                return Ok(ToolResult {
                    tool_id: "diff".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("File does not exist: {}", path)
                    }),
                    error: Some(format!("File does not exist: {}", path)),
                });
            }

            // Read file
            match self.read_file(&file_path) {
                Ok(content) => {
                    new_content_str = content;
                    files_compared.push(path.to_string());
                }
                Err(e) => {
                    return Ok(ToolResult {
                        tool_id: "diff".to_string(),
                        status: ToolStatus::Failure,
                        output: json!({
                            "error": format!("Failed to read file '{}': {}", path, e)
                        }),
                        error: Some(format!("Failed to read file '{}': {}", path, e)),
                    });
                }
            }
        }

        // Handle whitespace normalization if requested
        if ignore_whitespace {
            old_content_str = normalize_whitespace(&old_content_str);
            new_content_str = normalize_whitespace(&new_content_str);
        }

        // Create the diff
        let diff = TextDiff::from_lines(&old_content_str, &new_content_str);

        // Format according to requested output format
        let (diff_output, stats) = match output_format {
            DiffFormat::Unified => {
                let (text, stats) = self.format_unified_diff(&diff, context_lines);
                (json!(text), stats)
            }
            DiffFormat::Inline => {
                let (lines, stats) = self.format_inline_diff(&diff);
                (json!(lines), stats)
            }
            DiffFormat::Changes => {
                let (lines, stats) = self.format_changes_diff(&diff);

                // Debug output to help diagnose test failures
                info!("Changes-only diff format produced {} lines", lines.len());
                for line in &lines {
                    info!("Line: type={}, content={}", line.change_type, line.content);
                }

                (json!(lines), stats)
            }
        };

        // Return results
        Ok(ToolResult {
            tool_id: "diff".to_string(),
            status: ToolStatus::Success,
            output: json!({
                "diff": diff_output,
                "stats": {
                    "inserted": stats.inserted,
                    "deleted": stats.deleted,
                    "modified": stats.modified,
                    "unchanged": stats.unchanged
                },
                "files_compared": files_compared
            }),
            error: None,
        })
    }
}

// Normalize whitespace for whitespace-insensitive diff
fn normalize_whitespace(text: &str) -> String {
    // First, replace all whitespace sequences with a single space
    let mut result = String::with_capacity(text.len());
    let mut last_was_space = false;

    for c in text.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(c);
            last_was_space = false;
        }
    }

    // Then, remove whitespace around braces and parentheses
    let result = result
        .replace(" {", "{")
        .replace("{ ", "{")
        .replace(" }", "}")
        .replace("} ", "}")
        .replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(") ", ")");

    result
}
