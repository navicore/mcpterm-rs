use crate::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use walkdir::{DirEntry, WalkDir};

/// Configuration for the FindTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindConfig {
    /// Maximum number of files to return
    pub max_files: usize,
    /// Default depth for recursive search
    pub default_max_depth: usize,
    /// Paths that are not allowed to be searched
    pub denied_paths: Option<Vec<String>>,
    /// Paths that are allowed to be searched (if specified, overrides denied_paths)
    pub allowed_paths: Option<Vec<String>>,
}

impl Default for FindConfig {
    fn default() -> Self {
        Self {
            max_files: 1000,
            default_max_depth: 10,
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

/// Represents a file entry found
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    path: String,
    name: String,
    size: u64,
    is_dir: bool,
    modified_time: String,
}

/// Sort options for results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum SortBy {
    Name,
    Size,
    ModifiedTime,
}

impl From<&str> for SortBy {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "name" => SortBy::Name,
            "size" => SortBy::Size,
            "modified_time" | "mtime" => SortBy::ModifiedTime,
            _ => SortBy::Name, // Default
        }
    }
}

/// Sort order for results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum SortOrder {
    Ascending,
    Descending,
}

impl From<&str> for SortOrder {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "asc" | "ascending" => SortOrder::Ascending,
            "desc" | "descending" => SortOrder::Descending,
            _ => SortOrder::Ascending, // Default
        }
    }
}

/// The FindTool for searching files by name patterns
#[derive(Debug, Clone, Default)]
pub struct FindTool {
    config: FindConfig,
}

impl FindTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: FindConfig) -> Self {
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

    // Convert a file entry to our structured output
    fn convert_entry(&self, entry: &DirEntry) -> Result<FileEntry> {
        let path = entry.path();
        let metadata = fs::metadata(path)?;

        let modified = metadata.modified()?;
        let datetime: DateTime<Utc> = modified.into();
        let modified_time = datetime.to_rfc3339();

        Ok(FileEntry {
            path: path.to_string_lossy().to_string(),
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            size: metadata.len(),
            is_dir: metadata.is_dir(),
            modified_time,
        })
    }

    // Sort results based on the specified criteria
    fn sort_entries(
        &self,
        mut entries: Vec<FileEntry>,
        sort_by: SortBy,
        order: SortOrder,
    ) -> Vec<FileEntry> {
        match sort_by {
            SortBy::Name => {
                entries.sort_by(|a, b| {
                    if order == SortOrder::Ascending {
                        a.name.cmp(&b.name)
                    } else {
                        b.name.cmp(&a.name)
                    }
                });
            }
            SortBy::Size => {
                entries.sort_by(|a, b| {
                    if order == SortOrder::Ascending {
                        a.size.cmp(&b.size)
                    } else {
                        b.size.cmp(&a.size)
                    }
                });
            }
            SortBy::ModifiedTime => {
                entries.sort_by(|a, b| {
                    let parse_time = |time_str: &str| -> DateTime<Utc> {
                        DateTime::parse_from_rfc3339(time_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now())
                    };

                    let a_time = parse_time(&a.modified_time);
                    let b_time = parse_time(&b.modified_time);

                    if order == SortOrder::Ascending {
                        a_time.cmp(&b_time)
                    } else {
                        b_time.cmp(&a_time)
                    }
                });
            }
        }

        entries
    }

    // Parse date string to DateTime
    fn parse_date(&self, date_str: &str) -> Option<DateTime<Utc>> {
        // Try common formats
        let formats = [
            "%Y-%m-%d",          // 2023-01-01
            "%Y-%m-%dT%H:%M:%S", // 2023-01-01T12:30:45
            "%Y-%m-%d %H:%M:%S", // 2023-01-01 12:30:45
        ];

        for fmt in &formats {
            // For date-only format, append time component if needed
            let parse_str = if fmt == &"%Y-%m-%d" {
                format!("{}T00:00:00", date_str)
            } else {
                date_str.to_string()
            };

            if let Ok(dt) = NaiveDateTime::parse_from_str(&parse_str, fmt) {
                return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
            }
        }

        None
    }
}

#[async_trait]
impl Tool for FindTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "find".to_string(),
            name: "Find".to_string(),
            description: "Find files matching name patterns and criteria".to_string(),
            category: ToolCategory::Search,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern to match files (e.g. '**/*.rs')"
                    },
                    "base_dir": {
                        "type": "string",
                        "description": "Base directory for search (defaults to current directory)"
                    },
                    "exclude": {
                        "type": "string",
                        "description": "Glob pattern for files to exclude"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum directory depth to search"
                    },
                    "modified_after": {
                        "type": "string",
                        "description": "Only find files modified after this date (YYYY-MM-DD)"
                    },
                    "modified_before": {
                        "type": "string",
                        "description": "Only find files modified before this date (YYYY-MM-DD)"
                    },
                    "sort_by": {
                        "type": "string",
                        "description": "Sort results by (name, size, modified_time)",
                        "enum": ["name", "size", "modified_time"]
                    },
                    "order": {
                        "type": "string",
                        "description": "Sort order (asc, desc)",
                        "enum": ["asc", "desc"]
                    },
                    "include_dirs": {
                        "type": "boolean",
                        "description": "Whether to include directories in results",
                        "default": false
                    }
                },
                "required": ["pattern"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string"
                                },
                                "name": {
                                    "type": "string"
                                },
                                "size": {
                                    "type": "integer"
                                },
                                "is_dir": {
                                    "type": "boolean"
                                },
                                "modified_time": {
                                    "type": "string"
                                }
                            }
                        }
                    },
                    "total_files": {
                        "type": "integer"
                    },
                    "searched_dirs": {
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

        let base_dir = params["base_dir"].as_str().unwrap_or(".").to_string();

        let exclude_pattern = params["exclude"].as_str();

        let max_depth = params["max_depth"]
            .as_u64()
            .unwrap_or(self.config.default_max_depth as u64) as usize;

        let modified_after = params["modified_after"]
            .as_str()
            .and_then(|s| self.parse_date(s));

        let modified_before = params["modified_before"]
            .as_str()
            .and_then(|s| self.parse_date(s));

        let sort_by = params["sort_by"]
            .as_str()
            .map(SortBy::from)
            .unwrap_or(SortBy::Name);

        let order = params["order"]
            .as_str()
            .map(SortOrder::from)
            .unwrap_or(SortOrder::Ascending);

        let include_dirs = params["include_dirs"].as_bool().unwrap_or(false);

        // Check if path is allowed
        if !self.is_path_allowed(&base_dir) {
            return Ok(ToolResult {
                tool_id: "find".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "Access to this path is not allowed for security reasons"
                }),
                error: Some("Access to this path is not allowed for security reasons".to_string()),
            });
        }

        // Validate path exists
        let base_path = PathBuf::from(&base_dir);
        if !base_path.exists() {
            return Ok(ToolResult {
                tool_id: "find".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("Base directory does not exist: {}", base_dir)
                }),
                error: Some(format!("Base directory does not exist: {}", base_dir)),
            });
        }

        // Build include/exclude glob sets
        let include_glob = {
            let mut builder = GlobSetBuilder::new();
            // Modify pattern to be recursive if it doesn't start with ** and doesn't contain a path separator
            let modified_pattern = if !pattern.starts_with("**") && !pattern.contains('/') && !pattern.contains('\\') {
                format!("**/{}", pattern)
            } else {
                pattern.to_string()
            };

            info!("Using pattern: {}", modified_pattern);

            match Glob::new(&modified_pattern) {
                Ok(glob) => {
                    builder.add(glob);
                    builder.build().unwrap_or_else(|_| GlobSet::empty())
                }
                Err(e) => {
                    return Ok(ToolResult {
                        tool_id: "find".to_string(),
                        status: ToolStatus::Failure,
                        output: json!({
                            "error": format!("Invalid glob pattern: {}", e)
                        }),
                        error: Some(format!("Invalid glob pattern: {}", e)),
                    });
                }
            }
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
        info!(
            "Finding files matching pattern '{}' in: {}",
            pattern, base_dir
        );
        let mut entries = Vec::new();
        let mut searched_dirs = 0;

        // Walk the directory tree
        let walker = WalkDir::new(&base_path).max_depth(max_depth);

        for entry in walker.into_iter().filter_map(Result::ok) {
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();
            let is_dir = entry.file_type().is_dir();

            // Check if the path is allowed (to enforce denied_paths)
            if !self.is_path_allowed(&path_str) {
                info!("Skipping denied path: {}", path_str);
                continue;
            }

            // Count directories
            if is_dir {
                searched_dirs += 1;
            }

            // Skip directories if not included
            if is_dir && !include_dirs {
                continue;
            }

            // Get relative path for glob matching
            let rel_path = if let Ok(rel) = path.strip_prefix(&base_path) {
                rel.to_string_lossy()
            } else {
                path.file_name().unwrap_or_default().to_string_lossy()
            };

            // Apply include/exclude filters
            if !include_glob.is_match(rel_path.as_ref())
                && !include_glob.is_match(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .as_ref(),
                )
            {
                continue;
            }

            if let Some(exclude) = &exclude_glob {
                if exclude.is_match(rel_path.as_ref()) {
                    continue;
                }
            }

            // Apply time filters
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    let modified_time: DateTime<Utc> = modified.into();

                    if let Some(after) = modified_after {
                        if modified_time < after {
                            continue;
                        }
                    }

                    if let Some(before) = modified_before {
                        if modified_time > before {
                            continue;
                        }
                    }
                }
            }

            // Convert to FileEntry
            match self.convert_entry(&entry) {
                Ok(file_entry) => {
                    entries.push(file_entry);

                    // Stop if we've reached the maximum files
                    if entries.len() >= self.config.max_files {
                        break;
                    }
                }
                Err(e) => {
                    debug!("Error processing file {}: {}", path.to_string_lossy(), e);
                    // Continue with next file
                }
            }
        }

        // Sort the results
        let sorted_entries = self.sort_entries(entries, sort_by, order);

        // Log search results
        debug!(
            "Found {} files in {} directories",
            sorted_entries.len(),
            searched_dirs
        );

        // Return results
        Ok(ToolResult {
            tool_id: "find".to_string(),
            status: ToolStatus::Success,
            output: json!({
                "files": sorted_entries,
                "total_files": sorted_entries.len(),
                "searched_dirs": searched_dirs
            }),
            error: None,
        })
    }
}
