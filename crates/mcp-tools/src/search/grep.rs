use crate::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

/// Configuration for the GrepTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepConfig {
    /// Maximum number of matches to return
    pub max_matches: usize,
    /// Maximum number of files to scan
    pub max_files: usize,
    /// Maximum file size to scan in bytes
    pub max_file_size: usize,
    /// Default context lines
    pub default_context_lines: usize,
    /// Paths that are not allowed to be searched
    pub denied_paths: Option<Vec<String>>,
    /// Paths that are allowed to be searched (if specified, overrides denied_paths)
    pub allowed_paths: Option<Vec<String>>,
}

impl Default for GrepConfig {
    fn default() -> Self {
        Self {
            max_matches: 1000,
            max_files: 1000,
            max_file_size: 10 * 1024 * 1024, // 10 MB
            default_context_lines: 2,
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

/// Represents a match found in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrepMatch {
    file: String,
    line: usize,
    column: usize,
    matched_text: String,
    context_before: Vec<String>,
    context_after: Vec<String>,
}

/// The GrepTool for searching file contents with regex patterns
#[derive(Debug, Clone)]
pub struct GrepTool {
    config: GrepConfig,
}

impl GrepTool {
    pub fn new() -> Self {
        Self {
            config: GrepConfig::default(),
        }
    }

    pub fn with_config(config: GrepConfig) -> Self {
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

    // Extract lines before and after a match for context
    fn extract_context_lines(
        &self,
        content: &str,
        match_line_idx: usize,
        context_lines: usize,
    ) -> (Vec<String>, Vec<String>) {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Extract context before the match
        let start_idx = match_line_idx.saturating_sub(context_lines);
        let before_lines: Vec<String> = (start_idx..match_line_idx)
            .map(|i| lines[i].to_string())
            .collect();

        // Extract context after the match
        let end_idx = std::cmp::min(match_line_idx + context_lines + 1, total_lines);
        let after_lines: Vec<String> = ((match_line_idx + 1)..end_idx)
            .map(|i| lines[i].to_string())
            .collect();

        (before_lines, after_lines)
    }

    // Search a single file for matches
    fn search_file(
        &self,
        file_path: &Path,
        regex: &Regex,
        context_lines: usize,
        max_matches: usize,
        current_matches: &mut usize,
    ) -> Result<Vec<GrepMatch>> {
        let mut matches = Vec::new();

        // Check file size
        let metadata = fs::metadata(file_path)?;
        if metadata.len() as usize > self.config.max_file_size {
            return Ok(matches); // Skip files that are too large
        }

        // Read file content
        let content = fs::read_to_string(file_path)?;
        let lines: Vec<&str> = content.lines().collect();

        // Search for matches
        for (line_idx, line) in lines.iter().enumerate() {
            if *current_matches >= max_matches {
                break;
            }

            for cap in regex.find_iter(line) {
                if *current_matches >= max_matches {
                    break;
                }

                let (context_before, context_after) =
                    self.extract_context_lines(&content, line_idx, context_lines);

                matches.push(GrepMatch {
                    file: file_path.to_string_lossy().to_string(),
                    line: line_idx + 1, // 1-based line numbers for user-friendliness
                    column: cap.start() + 1, // 1-based column numbers
                    matched_text: cap.as_str().to_string(),
                    context_before,
                    context_after,
                });

                *current_matches += 1;
            }
        }

        Ok(matches)
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "grep".to_string(),
            name: "Grep".to_string(),
            description: "Search for patterns in file contents using regular expressions"
                .to_string(),
            category: ToolCategory::Search,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regular expression pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (defaults to current directory)"
                    },
                    "include": {
                        "type": "string",
                        "description": "Glob pattern for files to include (e.g. '*.rs')"
                    },
                    "exclude": {
                        "type": "string",
                        "description": "Glob pattern for files to exclude"
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Number of context lines before and after matches",
                        "default": 2
                    },
                    "max_matches": {
                        "type": "integer",
                        "description": "Maximum number of matches to return"
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "Whether to perform case-sensitive matching",
                        "default": false
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to search recursively in subdirectories",
                        "default": true
                    }
                },
                "required": ["pattern"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "matches": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file": {
                                    "type": "string"
                                },
                                "line": {
                                    "type": "integer"
                                },
                                "column": {
                                    "type": "integer"
                                },
                                "matched_text": {
                                    "type": "string"
                                },
                                "context_before": {
                                    "type": "array",
                                    "items": {
                                        "type": "string"
                                    }
                                },
                                "context_after": {
                                    "type": "array",
                                    "items": {
                                        "type": "string"
                                    }
                                }
                            }
                        }
                    },
                    "total_matches": {
                        "type": "integer"
                    },
                    "searched_files": {
                        "type": "integer"
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let pattern = params["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'pattern'"))?;

        let path = params["path"].as_str().unwrap_or(".").to_string();

        let include_pattern = params["include"].as_str();
        let exclude_pattern = params["exclude"].as_str();

        let context_lines = params["context_lines"]
            .as_u64()
            .unwrap_or(self.config.default_context_lines as u64)
            as usize;

        let max_matches = params["max_matches"]
            .as_u64()
            .unwrap_or(self.config.max_matches as u64) as usize;

        let case_sensitive = params["case_sensitive"].as_bool().unwrap_or(false);

        let recursive = params["recursive"].as_bool().unwrap_or(true);

        // Check if path is allowed
        if !self.is_path_allowed(&path) {
            return Ok(ToolResult {
                tool_id: "grep".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "Access to this path is not allowed for security reasons"
                }),
                error: Some("Access to this path is not allowed for security reasons".to_string()),
            });
        }

        // Validate path exists
        let path_obj = PathBuf::from(&path);
        if !path_obj.exists() {
            return Ok(ToolResult {
                tool_id: "grep".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("Path does not exist: {}", path)
                }),
                error: Some(format!("Path does not exist: {}", path)),
            });
        }

        // Compile regex
        let regex = match RegexBuilder::new(pattern)
            .case_insensitive(!case_sensitive)
            .build()
        {
            Ok(re) => re,
            Err(e) => {
                return Ok(ToolResult {
                    tool_id: "grep".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("Invalid regex pattern: {}", e)
                    }),
                    error: Some(format!("Invalid regex pattern: {}", e)),
                });
            }
        };

        // Build include/exclude glob sets
        let include_glob = if let Some(pattern) = include_pattern {
            let mut builder = GlobSetBuilder::new();
            match Glob::new(pattern) {
                Ok(glob) => {
                    builder.add(glob);
                    Some(builder.build().unwrap_or_else(|_| GlobSet::empty()))
                }
                Err(_) => None,
            }
        } else {
            None
        };

        let exclude_glob = if let Some(pattern) = exclude_pattern {
            let mut builder = GlobSetBuilder::new();
            match Glob::new(pattern) {
                Ok(glob) => {
                    builder.add(glob);
                    Some(builder.build().unwrap_or_else(|_| GlobSet::empty()))
                }
                Err(_) => None,
            }
        } else {
            None
        };

        // Perform the search
        info!("Searching for pattern '{}' in path: {}", pattern, path);
        let mut all_matches = Vec::new();
        let mut current_matches: usize = 0;
        let mut searched_files: usize = 0;

        // Use WalkDir to handle recursive search
        let walker = if recursive {
            WalkDir::new(path_obj)
        } else {
            WalkDir::new(path_obj).max_depth(1)
        };

        for entry in walker
            .into_iter()
            .filter_map(Result::ok)
            .take(self.config.max_files)
        {
            // Skip directories
            if entry.file_type().is_dir() {
                continue;
            }

            let file_path = entry.path();
            let file_path_str = file_path.to_string_lossy().to_string();
            let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();

            // Check if the path is allowed (to enforce denied_paths)
            if !self.is_path_allowed(&file_path_str) {
                info!("Skipping denied path: {}", file_path_str);
                continue;
            }

            // Apply include/exclude filters
            if let Some(include) = &include_glob {
                if !include.is_match(file_name.as_ref()) {
                    continue;
                }
            }

            if let Some(exclude) = &exclude_glob {
                if exclude.is_match(file_name.as_ref()) {
                    continue;
                }
            }

            // Search the file
            match self.search_file(
                file_path,
                &regex,
                context_lines,
                max_matches,
                &mut current_matches,
            ) {
                Ok(file_matches) => {
                    all_matches.extend(file_matches);
                    searched_files += 1;

                    // Stop if we've reached the maximum matches
                    if current_matches >= max_matches {
                        break;
                    }
                }
                Err(e) => {
                    debug!(
                        "Error searching file {}: {}",
                        file_path.to_string_lossy(),
                        e
                    );
                    // Continue with next file
                }
            }
        }

        // Log search results
        debug!(
            "Found {} matches in {} files",
            all_matches.len(),
            searched_files
        );

        // Return results
        Ok(ToolResult {
            tool_id: "grep".to_string(),
            status: ToolStatus::Success,
            output: json!({
                "matches": all_matches,
                "total_matches": all_matches.len(),
                "searched_files": searched_files
            }),
            error: None,
        })
    }
}
