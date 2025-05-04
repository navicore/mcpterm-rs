use crate::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Configuration for the PatchTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchConfig {
    /// Paths that are not allowed to be patched
    pub denied_paths: Option<Vec<String>>,
    /// Paths that are allowed to be patched (if specified, overrides denied_paths)
    pub allowed_paths: Option<Vec<String>>,
    /// Maximum file size to patch in bytes
    pub max_file_size: usize,
    /// Whether to create backups by default
    pub create_backup: bool,
}

impl Default for PatchConfig {
    fn default() -> Self {
        Self {
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
            max_file_size: 10 * 1024 * 1024, // 10 MB
            create_backup: true,
        }
    }
}

/// Represents a hunk in a unified diff
#[derive(Debug, Clone)]
struct DiffHunk {
    /// Starting line in the original file
    old_start: usize,
    /// Number of lines in the original file
    old_count: usize,
    /// Starting line in the new file
    new_start: usize,
    /// Number of lines in the new file
    new_count: usize,
    /// The actual lines in the hunk (prefixed with ' ', '-', or '+')
    lines: Vec<String>,
}

/// Result of applying a patch
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PatchResult {
    /// File that was patched
    target_file: String,
    /// Whether the patch was successfully applied
    success: bool,
    /// Path to the backup file if created
    backup_created: Option<String>,
    /// Number of hunks that were successfully applied
    hunks_applied: usize,
    /// Number of hunks that failed to apply
    hunks_failed: usize,
    /// List of conflicting hunks
    conflicts: Vec<String>,
}

/// The PatchTool for applying patches to files
#[derive(Debug, Clone)]
pub struct PatchTool {
    config: PatchConfig,
}

impl PatchTool {
    pub fn new() -> Self {
        Self {
            config: PatchConfig::default(),
        }
    }

    pub fn with_config(config: PatchConfig) -> Self {
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

    // Create a backup of the file
    fn create_backup(&self, file_path: &Path) -> Result<PathBuf> {
        let backup_path = PathBuf::from(format!("{}.bak", file_path.to_string_lossy()));
        fs::copy(file_path, &backup_path)?;
        Ok(backup_path)
    }

    // Parse a unified diff format into hunks
    fn parse_diff(&self, patch_content: &str) -> Result<Vec<DiffHunk>> {
        let mut hunks = Vec::new();
        let mut current_hunk: Option<DiffHunk> = None;

        // Split the diff by lines
        let lines: Vec<&str> = patch_content.lines().collect();

        for line in lines {
            // Check for hunk header like "@@ -1,5 +1,5 @@"
            if line.starts_with("@@") && line.contains("@@") {
                // If we're already processing a hunk, add it to the list
                if let Some(hunk) = current_hunk.take() {
                    hunks.push(hunk);
                }

                // Parse the hunk header
                let header = line.trim_matches('@').trim();
                let parts: Vec<&str> = header.split(' ').collect();

                if parts.len() < 2 {
                    return Err(anyhow!("Invalid hunk header format: {}", line));
                }

                let old_range = parts[0].trim_start_matches('-');
                let new_range = parts[1].trim_start_matches('+');

                let (old_start, old_count) = Self::parse_range(old_range)?;
                let (new_start, new_count) = Self::parse_range(new_range)?;

                current_hunk = Some(DiffHunk {
                    old_start,
                    old_count,
                    new_start,
                    new_count,
                    lines: Vec::new(),
                });
            } else if let Some(ref mut hunk) = current_hunk {
                // Add the line to the current hunk
                // Check if it's a valid hunk line (must start with ' ', '-', or '+')
                if line.starts_with(' ') || line.starts_with('-') || line.starts_with('+') {
                    hunk.lines.push(line.to_string());
                } else if !line.is_empty() {
                    // Only error on non-empty lines that don't match the pattern
                    // Ignore empty lines for better compatibility
                    warn!("Ignoring invalid line in patch: {}", line);
                }
            }
        }

        // Add the last hunk if any
        if let Some(hunk) = current_hunk {
            hunks.push(hunk);
        }

        // Ensure we found at least one hunk
        if hunks.is_empty() {
            return Err(anyhow!("No valid hunks found in the patch"));
        }

        Ok(hunks)
    }

    // Parse a range like "1,5" into (start, count)
    fn parse_range(range: &str) -> Result<(usize, usize)> {
        let parts: Vec<&str> = range.split(',').collect();

        if parts.len() != 2 {
            return Err(anyhow!("Invalid range format: {}", range));
        }

        let start = parts[0].parse::<usize>()?;
        let count = parts[1].parse::<usize>()?;

        Ok((start, count))
    }

    // Apply a patch to a file
    fn apply_patch(
        &self,
        file_path: &Path,
        hunks: &[DiffHunk],
        dry_run: bool,
    ) -> Result<PatchResult> {
        // Read the original file
        let original_content = fs::read_to_string(file_path)?;
        let original_lines: Vec<&str> = original_content.lines().collect();

        // Create a result with file path
        let file_str = file_path.to_string_lossy().to_string();
        let mut result = PatchResult {
            target_file: file_str.clone(),
            success: true,
            backup_created: None,
            hunks_applied: 0,
            hunks_failed: 0,
            conflicts: Vec::new(),
        };

        // Apply hunks to build the new content
        let mut new_lines = Vec::new();
        let mut current_line_idx = 0;

        for (hunk_idx, hunk) in hunks.iter().enumerate() {
            // Add the lines before the hunk
            let old_start_idx = hunk.old_start - 1; // Convert to 0-indexed

            // Validate that we haven't overshot
            if old_start_idx > original_lines.len() {
                // This hunk tries to change lines beyond the file's end
                result.success = false;
                result.hunks_failed += 1;
                result.conflicts.push(format!(
                    "Hunk #{} failed: attempts to modify lines beyond the end of the file",
                    hunk_idx + 1
                ));
                continue;
            }

            // Add lines before the hunk
            while current_line_idx < old_start_idx {
                if current_line_idx < original_lines.len() {
                    new_lines.push(original_lines[current_line_idx].to_string());
                }
                current_line_idx += 1;
            }

            // Apply the hunk - first ensure the context matches
            let mut can_apply = true;
            let mut context_idx = 0;

            // Verify context - all ' ' lines should match the original
            for hunk_line in &hunk.lines {
                if hunk_line.starts_with(' ') {
                    let context_line = &hunk_line[1..]; // Remove the ' ' prefix

                    if context_idx + current_line_idx >= original_lines.len()
                        || original_lines[context_idx + current_line_idx] != context_line
                    {
                        can_apply = false;
                        break;
                    }

                    context_idx += 1;
                } else if hunk_line.starts_with('-') {
                    // Check that deletion lines match
                    let deletion_line = &hunk_line[1..]; // Remove the '-' prefix

                    if context_idx + current_line_idx >= original_lines.len()
                        || original_lines[context_idx + current_line_idx] != deletion_line
                    {
                        can_apply = false;
                        break;
                    }

                    context_idx += 1;
                }
            }

            if can_apply {
                // Apply the hunk
                let mut line_offset = 0;
                for hunk_line in &hunk.lines {
                    if hunk_line.starts_with(' ') {
                        // Unchanged line
                        let content = &hunk_line[1..]; // Remove the ' ' prefix
                        new_lines.push(content.to_string());
                        line_offset += 1;
                    } else if hunk_line.starts_with('+') {
                        // Added line
                        let content = &hunk_line[1..]; // Remove the '+' prefix
                        new_lines.push(content.to_string());
                    } else if hunk_line.starts_with('-') {
                        // Removed line - skip it
                        line_offset += 1;
                    }
                }
                // Update the current_line_idx after processing the entire hunk
                current_line_idx += line_offset;

                result.hunks_applied += 1;
            } else {
                // Can't apply this hunk - add the original lines and mark as failed
                let hunk_lines = hunk.old_count.min(original_lines.len() - current_line_idx);
                for i in 0..hunk_lines {
                    if current_line_idx + i < original_lines.len() {
                        new_lines.push(original_lines[current_line_idx + i].to_string());
                    }
                }

                current_line_idx += hunk_lines;
                result.hunks_failed += 1;
                result.conflicts.push(format!(
                    "Hunk #{} failed: the file content doesn't match the patch context",
                    hunk_idx + 1
                ));
                result.success = false;
            }
        }

        // Add any remaining lines
        while current_line_idx < original_lines.len() {
            new_lines.push(original_lines[current_line_idx].to_string());
            current_line_idx += 1;
        }

        // If this is a dry run, just return the result
        if dry_run {
            return Ok(result);
        }

        // Create backup if needed and it's not a dry run
        if self.config.create_backup {
            let backup_path = self.create_backup(file_path)?;
            result.backup_created = Some(backup_path.to_string_lossy().to_string());
        }

        // Write the new content to the file if not a dry run
        let new_content = new_lines.join("\n");
        let mut file = fs::File::create(file_path)?;
        file.write_all(new_content.as_bytes())?;

        // Add a newline at the end if the original had one
        if original_content.ends_with('\n') && !new_content.ends_with('\n') {
            file.write_all(b"\n")?;
        }

        Ok(result)
    }
}

#[async_trait]
impl Tool for PatchTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "patch".to_string(),
            name: "Patch".to_string(),
            description: "Apply patches to files using unified diff format".to_string(),
            category: ToolCategory::Utility,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target_file": {
                        "type": "string",
                        "description": "Path to the file to patch"
                    },
                    "patch_content": {
                        "type": "string",
                        "description": "Patch content in unified diff format"
                    },
                    "create_backup": {
                        "type": "boolean",
                        "description": "Whether to create a backup of the original file",
                        "default": true
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "Whether to simulate the patch application without changing files",
                        "default": false
                    },
                    "ignore_whitespace": {
                        "type": "boolean",
                        "description": "Whether to ignore whitespace when applying the patch",
                        "default": false
                    }
                },
                "required": ["target_file", "patch_content"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "success": {
                        "type": "boolean",
                        "description": "Whether the patch was successfully applied"
                    },
                    "target_file": {
                        "type": "string",
                        "description": "Path to the file that was patched"
                    },
                    "backup_created": {
                        "type": "string",
                        "description": "Path to the backup file if created"
                    },
                    "hunks_applied": {
                        "type": "integer",
                        "description": "Number of hunks that were successfully applied"
                    },
                    "hunks_failed": {
                        "type": "integer",
                        "description": "Number of hunks that failed to apply"
                    },
                    "conflicts": {
                        "type": "array",
                        "description": "List of conflicting hunks",
                        "items": {
                            "type": "string"
                        }
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let target_file = params["target_file"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'target_file'"))?;

        let patch_content = params["patch_content"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'patch_content'"))?;

        let create_backup = params["create_backup"]
            .as_bool()
            .unwrap_or(self.config.create_backup);

        let dry_run = params["dry_run"].as_bool().unwrap_or(false);

        // Check if the target file path is allowed
        if !self.is_path_allowed(target_file) {
            return Ok(ToolResult {
                tool_id: "patch".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "Access to this path is not allowed for security reasons"
                }),
                error: Some("Access to this path is not allowed for security reasons".to_string()),
            });
        }

        // Check if the target file exists
        let file_path = PathBuf::from(target_file);
        if !file_path.exists() {
            return Ok(ToolResult {
                tool_id: "patch".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("Target file does not exist: {}", target_file)
                }),
                error: Some(format!("Target file does not exist: {}", target_file)),
            });
        }

        // Check file size
        let metadata = fs::metadata(&file_path)?;
        if metadata.len() as usize > self.config.max_file_size {
            return Ok(ToolResult {
                tool_id: "patch".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("File is too large to patch (max size: {} bytes)", self.config.max_file_size)
                }),
                error: Some(format!(
                    "File is too large to patch (max size: {} bytes)",
                    self.config.max_file_size
                )),
            });
        }

        // Parse the diff
        let hunks = match self.parse_diff(patch_content) {
            Ok(hunks) => hunks,
            Err(e) => {
                return Ok(ToolResult {
                    tool_id: "patch".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("Failed to parse patch: {}", e)
                    }),
                    error: Some(format!("Failed to parse patch: {}", e)),
                });
            }
        };

        // Apply the patch with the current backup setting
        let mut local_config = self.config.clone();
        local_config.create_backup = create_backup;

        let patch_result = match self.apply_patch(&file_path, &hunks, dry_run) {
            Ok(result) => result,
            Err(e) => {
                return Ok(ToolResult {
                    tool_id: "patch".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("Failed to apply patch: {}", e)
                    }),
                    error: Some(format!("Failed to apply patch: {}", e)),
                });
            }
        };

        // Return the result
        let status = if patch_result.success {
            ToolStatus::Success
        } else {
            ToolStatus::Failure
        };

        Ok(ToolResult {
            tool_id: "patch".to_string(),
            status,
            output: json!({
                "success": patch_result.success,
                "target_file": patch_result.target_file,
                "backup_created": patch_result.backup_created,
                "hunks_applied": patch_result.hunks_applied,
                "hunks_failed": patch_result.hunks_failed,
                "conflicts": patch_result.conflicts
            }),
            error: if patch_result.success {
                None
            } else {
                Some(format!(
                    "Patch application had {} conflicts",
                    patch_result.conflicts.len()
                ))
            },
        })
    }
}
