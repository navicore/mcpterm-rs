use anyhow::{anyhow, Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

use super::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use crate::mcp::protocol::validation;
use crate::mcp::resources::ResourceManager;

/// Maximum number of results to return
const MAX_RESULTS: usize = 1000;

/// Maximum file size to search for content (in bytes)
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

/// Search tool
pub struct SearchTool {
    /// Base directory for search operations
    base_dir: PathBuf,

    /// Files/directories to exclude from search
    exclude_patterns: GlobSet,
}

/// Search type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchType {
    /// Search for files matching a pattern
    File,

    /// Search for content within files
    Content,
}

/// Search parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    /// Type of search
    #[serde(default = "default_search_type")]
    pub search_type: SearchType,

    /// Base directory (relative to tool base directory)
    #[serde(default)]
    pub directory: Option<String>,

    /// Pattern to search for
    pub pattern: String,

    /// Include only files matching this glob pattern (file search only)
    #[serde(default)]
    pub include: Option<String>,

    /// Exclude files matching these glob patterns
    #[serde(default)]
    pub exclude: Option<Vec<String>>,

    /// Maximum number of results to return
    #[serde(default = "default_max_results")]
    pub max_results: usize,

    /// Whether to ignore case in pattern matching
    #[serde(default)]
    pub ignore_case: bool,

    /// Whether to recurse into subdirectories
    #[serde(default = "default_recursive")]
    pub recursive: bool,
}

/// Default search type function
fn default_search_type() -> SearchType {
    SearchType::File
}

/// Default max results function
fn default_max_results() -> usize {
    100
}

/// Default recursive function
fn default_recursive() -> bool {
    true
}

/// File search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResult {
    /// File path (relative to search directory)
    pub path: String,

    /// File size in bytes
    pub size: u64,

    /// Whether the path is a directory
    pub is_dir: bool,
}

/// Content match result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMatch {
    /// File path (relative to search directory)
    pub path: String,

    /// Line number of the match (1-based)
    pub line_number: usize,

    /// Line content containing the match
    pub line: String,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Type of search that was performed
    pub search_type: SearchType,

    /// Pattern that was searched for
    pub pattern: String,

    /// Directory that was searched
    pub directory: String,

    /// Number of matches found
    pub total_found: usize,

    /// Whether the result set was truncated due to max_results
    pub truncated: bool,

    /// File matches (for file search)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_matches: Option<Vec<FileSearchResult>>,

    /// Content matches (for content search)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_matches: Option<Vec<ContentMatch>>,
}

impl SearchTool {
    /// Create a new search tool
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        // Define default exclude patterns
        let default_excludes = vec![
            ".git/**",
            "node_modules/**",
            "target/**",
            "build/**",
            "dist/**",
            ".DS_Store",
            "*.pyc",
            "*.pyo",
            "*.so",
            "*.o",
            "*.a",
            "*.class",
            "*.exe",
            "*.dll",
            "*.obj",
            "*.jar",
            "*.zip",
            "*.tar",
            "*.tar.gz",
            "*.tgz",
            "*.7z",
            "*.rar",
            "*.swp",
            "*.swo",
            "*.swn",
            "*.bak",
            "*.tmp",
            "*.temp",
        ];

        // Build exclude glob set
        let mut builder = GlobSetBuilder::new();
        for pattern in default_excludes {
            let glob = Glob::new(pattern)
                .context(format!("Failed to create glob from pattern: {}", pattern))?;
            builder.add(glob);
        }
        let exclude_patterns = builder
            .build()
            .context("Failed to build glob set for exclude patterns")?;

        Ok(Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            exclude_patterns,
        })
    }

    /// Check if a path should be excluded from search
    fn is_excluded(&self, entry: &DirEntry, user_exclude: &Option<GlobSet>) -> bool {
        let path = entry.path();

        // Check against default exclude patterns
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| self.exclude_patterns.is_match(n))
            .unwrap_or(false)
        {
            return true;
        }

        // Check against user exclude patterns
        if let Some(exclude) = user_exclude {
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| exclude.is_match(n))
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    /// Perform file search
    fn search_files(
        &self,
        base_dir: &Path,
        params: &SearchParams,
        user_include: &Option<GlobSet>,
        user_exclude: &Option<GlobSet>,
    ) -> Result<SearchResult> {
        let mut results = Vec::new();
        let mut total_found = 0;
        let mut truncated = false;

        // Create WalkDir iterator with appropriate recursion depth
        let walk = if params.recursive {
            WalkDir::new(base_dir)
        } else {
            WalkDir::new(base_dir).max_depth(1)
        };

        // Process each entry
        for entry in walk {
            // Break if we've reached the maximum number of results
            if results.len() >= params.max_results {
                truncated = true;
                break;
            }

            let entry = entry.context("Failed to read directory entry")?;

            // Skip excluded entries
            if self.is_excluded(&entry, user_exclude) {
                // If it's a directory, we've already skipped it
                // WalkDir doesn't support filter_entry here as it's already been yielded
                // We'll handle directory skipping at the WalkDir creation
                continue;
            }

            // Get the entry's path
            let path = entry.path();
            let metadata = fs::metadata(path).context("Failed to get file metadata")?;

            // Get the path relative to the base directory
            let rel_path = path
                .strip_prefix(base_dir)
                .context("Failed to get relative path")?
                .to_string_lossy()
                .into_owned();

            // Check if the path matches the pattern
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            let matches = if params.ignore_case {
                filename
                    .to_lowercase()
                    .contains(&params.pattern.to_lowercase())
            } else {
                filename.contains(&params.pattern)
            };

            // Check if the path matches the include pattern (if provided)
            let include_matches = user_include.as_ref().map_or(true, |include| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| include.is_match(n))
                    .unwrap_or(false)
            });

            // Only include if both pattern and include pattern match
            if matches && include_matches {
                total_found += 1;
                results.push(FileSearchResult {
                    path: rel_path,
                    size: metadata.len(),
                    is_dir: metadata.is_dir(),
                });
            }
        }

        Ok(SearchResult {
            search_type: SearchType::File,
            pattern: params.pattern.clone(),
            directory: base_dir.to_string_lossy().into_owned(),
            total_found,
            truncated,
            file_matches: Some(results),
            content_matches: None,
        })
    }

    /// Perform content search
    fn search_content(
        &self,
        base_dir: &Path,
        params: &SearchParams,
        user_include: &Option<GlobSet>,
        user_exclude: &Option<GlobSet>,
    ) -> Result<SearchResult> {
        let mut results = Vec::new();
        let mut total_found = 0;
        let mut truncated = false;

        // Create WalkDir iterator with appropriate recursion depth
        let walk = if params.recursive {
            WalkDir::new(base_dir)
        } else {
            WalkDir::new(base_dir).max_depth(1)
        };

        // Process each entry
        for entry in walk {
            // Break if we've reached the maximum number of results
            if results.len() >= params.max_results {
                truncated = true;
                break;
            }

            let entry = entry.context("Failed to read directory entry")?;

            // Skip excluded entries
            if self.is_excluded(&entry, user_exclude) {
                // If it's a directory, we've already skipped it
                // WalkDir doesn't support filter_entry here as it's already been yielded
                // We'll handle directory skipping at the WalkDir creation
                continue;
            }

            // Skip directories
            if entry.file_type().is_dir() {
                continue;
            }

            // Get the entry's path
            let path = entry.path();

            // Check if the path matches the include pattern (if provided)
            let include_matches = user_include.as_ref().map_or(true, |include| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| include.is_match(n))
                    .unwrap_or(false)
            });

            // Skip if the file doesn't match the include pattern
            if !include_matches {
                continue;
            }

            // Get file metadata
            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue, // Skip if we can't get metadata
            };

            // Skip if the file is too large
            if metadata.len() > MAX_FILE_SIZE {
                continue;
            }

            // Get the path relative to the base directory
            let rel_path = path
                .strip_prefix(base_dir)
                .context("Failed to get relative path")?
                .to_string_lossy()
                .into_owned();

            // Open the file
            let file = match File::open(path) {
                Ok(f) => f,
                Err(_) => continue, // Skip if we can't open the file
            };

            // Search for the pattern in the file
            let reader = BufReader::new(file);
            for (line_index, line_result) in reader.lines().enumerate() {
                // Break if we've reached the maximum number of results
                if results.len() >= params.max_results {
                    truncated = true;
                    break;
                }

                // Skip if we can't read the line
                let line = match line_result {
                    Ok(l) => l,
                    Err(_) => continue,
                };

                // Check if the line contains the pattern
                let matches = if params.ignore_case {
                    line.to_lowercase().contains(&params.pattern.to_lowercase())
                } else {
                    line.contains(&params.pattern)
                };

                if matches {
                    total_found += 1;
                    results.push(ContentMatch {
                        path: rel_path.clone(),
                        line_number: line_index + 1, // Convert to 1-based line number
                        line,
                    });
                }
            }
        }

        Ok(SearchResult {
            search_type: SearchType::Content,
            pattern: params.pattern.clone(),
            directory: base_dir.to_string_lossy().into_owned(),
            total_found,
            truncated,
            file_matches: None,
            content_matches: Some(results),
        })
    }
}

impl Tool for SearchTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "search".to_string(),
            name: "File Search".to_string(),
            description: "Search for files or content within files".to_string(),
            category: ToolCategory::Search,
            input_schema: json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "search_type": {
                        "type": "string",
                        "enum": ["file", "content"],
                        "description": "Type of search (file or content)",
                        "default": "file"
                    },
                    "directory": {
                        "type": ["string", "null"],
                        "description": "Base directory (relative to tool base directory)"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Pattern to search for"
                    },
                    "include": {
                        "type": ["string", "null"],
                        "description": "Include only files matching this glob pattern (file search only)"
                    },
                    "exclude": {
                        "type": ["array", "null"],
                        "items": {
                            "type": "string"
                        },
                        "description": "Exclude files matching these glob patterns"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return",
                        "default": 100
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "description": "Whether to ignore case in pattern matching",
                        "default": false
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to recurse into subdirectories",
                        "default": true
                    }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "search_type": {
                        "type": "string",
                        "enum": ["file", "content"],
                        "description": "Type of search that was performed"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Pattern that was searched for"
                    },
                    "directory": {
                        "type": "string",
                        "description": "Directory that was searched"
                    },
                    "total_found": {
                        "type": "integer",
                        "description": "Number of matches found"
                    },
                    "truncated": {
                        "type": "boolean",
                        "description": "Whether the result set was truncated due to max_results"
                    },
                    "file_matches": {
                        "type": ["array", "null"],
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "File path (relative to search directory)"
                                },
                                "size": {
                                    "type": "integer",
                                    "description": "File size in bytes"
                                },
                                "is_dir": {
                                    "type": "boolean",
                                    "description": "Whether the path is a directory"
                                }
                            }
                        },
                        "description": "File matches (for file search)"
                    },
                    "content_matches": {
                        "type": ["array", "null"],
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "File path (relative to search directory)"
                                },
                                "line_number": {
                                    "type": "integer",
                                    "description": "Line number of the match (1-based)"
                                },
                                "line": {
                                    "type": "string",
                                    "description": "Line content containing the match"
                                }
                            }
                        },
                        "description": "Content matches (for content search)"
                    }
                }
            }),
            metadata: None,
        }
    }

    fn execute(&self, params: Value, _resource_manager: &ResourceManager) -> Result<ToolResult> {
        // Parse parameters
        let mut params: SearchParams =
            serde_json::from_value(params).context("Failed to parse search parameters")?;

        // Validate parameters
        validation::validate_path_component(&params.pattern).context("Invalid search pattern")?;

        if let Some(ref dir) = params.directory {
            validation::validate_path_component(dir).context("Invalid directory path")?;
        }

        // Limit max_results to prevent excessive resource usage
        params.max_results = params.max_results.min(MAX_RESULTS);

        // Determine search directory
        let search_dir = if let Some(ref dir) = params.directory {
            self.base_dir.join(dir)
        } else {
            self.base_dir.clone()
        };

        // Ensure search directory exists
        if !search_dir.exists() {
            return Err(anyhow!(
                "Search directory does not exist: {}",
                search_dir.display()
            ));
        }

        // Build user include glob set
        let user_include = if let Some(ref include) = params.include {
            let glob = Glob::new(include).context(format!(
                "Failed to create glob from include pattern: {}",
                include
            ))?;
            let mut builder = GlobSetBuilder::new();
            builder.add(glob);
            Some(
                builder
                    .build()
                    .context("Failed to build glob set for include pattern")?,
            )
        } else {
            None
        };

        // Build user exclude glob set
        let user_exclude = if let Some(ref exclude) = params.exclude {
            let mut builder = GlobSetBuilder::new();
            for pattern in exclude {
                let glob = Glob::new(pattern).context(format!(
                    "Failed to create glob from exclude pattern: {}",
                    pattern
                ))?;
                builder.add(glob);
            }
            Some(
                builder
                    .build()
                    .context("Failed to build glob set for exclude patterns")?,
            )
        } else {
            None
        };

        // Perform search based on search type
        let result = match params.search_type {
            SearchType::File => {
                self.search_files(&search_dir, &params, &user_include, &user_exclude)?
            }
            SearchType::Content => {
                self.search_content(&search_dir, &params, &user_include, &user_exclude)?
            }
        };

        // Create tool result
        let tool_result = ToolResult {
            tool_id: "search".to_string(),
            status: ToolStatus::Success,
            output: serde_json::to_value(result).context("Failed to serialize search result")?,
            error: None,
        };

        Ok(tool_result)
    }
}
