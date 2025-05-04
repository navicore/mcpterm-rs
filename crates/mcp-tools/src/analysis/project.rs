use crate::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Project type or language/framework identification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Java,
    CSharp,
    Cpp,
    Unknown,
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectType::Rust => write!(f, "Rust"),
            ProjectType::Node => write!(f, "Node.js"),
            ProjectType::Python => write!(f, "Python"),
            ProjectType::Go => write!(f, "Go"),
            ProjectType::Java => write!(f, "Java"),
            ProjectType::CSharp => write!(f, "C#"),
            ProjectType::Cpp => write!(f, "C++"),
            ProjectType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Information about a file or directory in the project structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// Path of the file or directory relative to the project root
    pub path: String,
    /// Whether this is a directory or a file
    pub is_dir: bool,
    /// Size of the file in bytes (0 for directories)
    pub size: u64,
    /// Children files/directories if this is a directory
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FileInfo>,
    /// Detected file type or purpose (e.g. "build-file", "config", "source", "test")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
}

/// Information about a project entry point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPoint {
    /// The path to the entry point file
    pub path: String,
    /// The type of entry point (e.g. "main", "test", "api")
    pub entry_type: String,
    /// Description of the entry point
    pub description: String,
}

/// Information about a project dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Name of the dependency
    pub name: String,
    /// Version of the dependency
    pub version: Option<String>,
    /// Whether this is a development dependency
    pub is_dev: bool,
}

/// Configuration for the ProjectNavigator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Paths that are not allowed to be analyzed for security reasons
    pub denied_paths: Option<Vec<String>>,
    /// Paths that are allowed to be analyzed (overrides denied_paths if specified)
    pub allowed_paths: Option<Vec<String>>,
    /// Maximum directory depth to analyze
    pub max_depth: usize,
    /// Maximum file size to read and analyze
    pub max_file_size: usize,
    /// List of directories to skip (e.g. "node_modules", "target")
    pub skip_dirs: Vec<String>,
    /// List of file extensions to skip (e.g. ".pyc", ".o")
    pub skip_extensions: Vec<String>,
}

impl Default for ProjectConfig {
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
            max_depth: 10, // Reasonable depth limit to avoid excessive recursion
            max_file_size: 10 * 1024 * 1024, // 10 MB
            skip_dirs: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "__pycache__".to_string(),
                "venv".to_string(),
                "env".to_string(),
                "bin".to_string(),
                "obj".to_string(),
                "build".to_string(),
                "dist".to_string(),
            ],
            skip_extensions: vec![
                ".pyc".to_string(),
                ".pyo".to_string(),
                ".class".to_string(),
                ".o".to_string(),
                ".obj".to_string(),
                ".exe".to_string(),
                ".dll".to_string(),
                ".so".to_string(),
                ".a".to_string(),
                ".lib".to_string(),
            ],
        }
    }
}

/// ProjectNavigator tool for analyzing project structure
#[derive(Debug, Default, Clone)]
pub struct ProjectNavigator {
    config: ProjectConfig,
}

impl ProjectNavigator {
    pub fn new() -> Self {
        Self {
            config: ProjectConfig::default(),
        }
    }

    pub fn with_config(config: ProjectConfig) -> Self {
        Self { config }
    }

    // Check if a path is allowed based on configuration
    fn is_path_allowed(&self, path_str: &str) -> bool {
        let path = PathBuf::from(path_str);
        let path_str = path.to_string_lossy().to_string();

        info!("Checking if path is allowed for navigation: '{}'", path_str);

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

    // Detect the type of project based on files in the root directory
    fn detect_project_type(&self, project_dir: &Path) -> ProjectType {
        // Check for Rust project (Cargo.toml)
        if project_dir.join("Cargo.toml").exists() {
            return ProjectType::Rust;
        }

        // Check for Node.js project (package.json)
        if project_dir.join("package.json").exists() {
            return ProjectType::Node;
        }

        // Check for Python project (pyproject.toml, setup.py, requirements.txt)
        if project_dir.join("pyproject.toml").exists()
            || project_dir.join("setup.py").exists()
            || project_dir.join("requirements.txt").exists()
        {
            return ProjectType::Python;
        }

        // Check for Go project (go.mod)
        if project_dir.join("go.mod").exists() {
            return ProjectType::Go;
        }

        // Check for Java project (pom.xml, build.gradle)
        if project_dir.join("pom.xml").exists() || project_dir.join("build.gradle").exists() {
            return ProjectType::Java;
        }

        // Check for C# project (.csproj, .sln)
        if let Ok(entries) = fs::read_dir(project_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "csproj" || ext == "sln" {
                        return ProjectType::CSharp;
                    }
                }
            }
        }

        // Check for C++ project (CMakeLists.txt, Makefile with C++ patterns)
        if project_dir.join("CMakeLists.txt").exists() {
            // Check if it's a C++ project by looking for C++ files
            if self.has_file_with_extension(project_dir, &[".cpp", ".cc", ".cxx", ".hpp", ".hh"]) {
                return ProjectType::Cpp;
            }
        }

        ProjectType::Unknown
    }

    // Helper to check if directory has files with specific extensions
    fn has_file_with_extension(&self, dir: &Path, extensions: &[&str]) -> bool {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = format!(".{}", ext.to_string_lossy());
                        if extensions.iter().any(|e| *e == ext_str) {
                            return true;
                        }
                    }
                } else if path.is_dir() {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    if !self.config.skip_dirs.iter().any(|d| d == &file_name)
                        && self.has_file_with_extension(&path, extensions)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    // Parse dependencies based on project type
    fn parse_dependencies(
        &self,
        project_dir: &Path,
        project_type: &ProjectType,
    ) -> Vec<Dependency> {
        let mut dependencies = Vec::new();

        match project_type {
            ProjectType::Rust => self.parse_rust_dependencies(project_dir, &mut dependencies),
            ProjectType::Node => self.parse_node_dependencies(project_dir, &mut dependencies),
            ProjectType::Python => self.parse_python_dependencies(project_dir, &mut dependencies),
            _ => {
                // Other project types not implemented yet
                debug!("Dependency parsing not implemented for {:?}", project_type);
            }
        }

        dependencies
    }

    // Parse Rust dependencies from Cargo.toml
    fn parse_rust_dependencies(&self, project_dir: &Path, dependencies: &mut Vec<Dependency>) {
        let cargo_path = project_dir.join("Cargo.toml");
        if !cargo_path.exists() {
            return;
        }

        // Basic parsing of Cargo.toml using simple string matching
        // This is not as robust as using a TOML parser but works for basic cases
        if let Ok(content) = fs::read_to_string(&cargo_path) {
            // Simple state machine to track the section we're in
            let mut in_dependencies = false;
            let mut in_dev_dependencies = false;

            for line in content.lines() {
                let trimmed = line.trim();

                // Look for section headers
                if trimmed == "[dependencies]" {
                    in_dependencies = true;
                    in_dev_dependencies = false;
                    continue;
                } else if trimmed == "[dev-dependencies]" {
                    in_dependencies = false;
                    in_dev_dependencies = true;
                    continue;
                } else if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    // Any other section - reset flags
                    in_dependencies = false;
                    in_dev_dependencies = false;
                    continue;
                }

                // If we're in a dependency section, parse dependencies
                if (in_dependencies || in_dev_dependencies)
                    && !trimmed.is_empty()
                    && !trimmed.starts_with('#')
                {
                    // Simple form: name = "version"
                    if let Some(eq_pos) = trimmed.find('=') {
                        let name = trimmed[..eq_pos].trim().to_string();
                        let version_part = trimmed[eq_pos + 1..].trim();

                        // Handle basic version formats: "0.1.0" or { version = "0.1.0", features = [...] }
                        let version = if version_part.starts_with('"')
                            && version_part.ends_with('"')
                        {
                            // Simple string version
                            Some(version_part.trim_matches('"').to_string())
                        } else if version_part.starts_with('{') {
                            // Complex dependency spec
                            if let Some(ver_start) = version_part.find("version") {
                                if let Some(ver_eq) = version_part[ver_start..].find('=') {
                                    let ver_str = &version_part[ver_start + ver_eq + 1..];
                                    if let Some(quote_start) = ver_str.find('"') {
                                        ver_str[quote_start + 1..].find('"').map(|quote_end| {
                                            ver_str[quote_start + 1..quote_start + 1 + quote_end]
                                                .to_string()
                                        })
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        dependencies.push(Dependency {
                            name,
                            version,
                            is_dev: in_dev_dependencies,
                        });
                    }
                }
            }
        }
    }

    // Parse Node.js dependencies from package.json
    fn parse_node_dependencies(&self, project_dir: &Path, dependencies: &mut Vec<Dependency>) {
        let package_path = project_dir.join("package.json");
        if !package_path.exists() {
            return;
        }

        if let Ok(content) = fs::read_to_string(&package_path) {
            // Try to parse the JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                // Parse regular dependencies
                if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
                    for (name, version) in deps {
                        if let Some(ver_str) = version.as_str() {
                            dependencies.push(Dependency {
                                name: name.clone(),
                                version: Some(ver_str.to_string()),
                                is_dev: false,
                            });
                        }
                    }
                }

                // Parse dev dependencies
                if let Some(dev_deps) = json.get("devDependencies").and_then(|d| d.as_object()) {
                    for (name, version) in dev_deps {
                        if let Some(ver_str) = version.as_str() {
                            dependencies.push(Dependency {
                                name: name.clone(),
                                version: Some(ver_str.to_string()),
                                is_dev: true,
                            });
                        }
                    }
                }
            }
        }
    }

    // Parse Python dependencies from requirements.txt or setup.py
    fn parse_python_dependencies(&self, project_dir: &Path, dependencies: &mut Vec<Dependency>) {
        // Try requirements.txt first
        let req_path = project_dir.join("requirements.txt");
        if req_path.exists() {
            if let Ok(content) = fs::read_to_string(&req_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        // Parse requirements like: package==1.0.0 or package>=1.0.0
                        let parts: Vec<&str> = trimmed
                            .split(&['=', '>', '<', '~', '!'][..])
                            .map(|s| s.trim())
                            .collect();

                        if !parts.is_empty() {
                            let name = parts[0].to_string();
                            let version = if parts.len() > 1 && !parts[1].is_empty() {
                                Some(parts[1].to_string())
                            } else {
                                None
                            };

                            dependencies.push(Dependency {
                                name,
                                version,
                                is_dev: false, // No distinction in requirements.txt
                            });
                        }
                    }
                }
            }
        }

        // Look for dev requirements
        let dev_req_path = project_dir.join("dev-requirements.txt");
        if dev_req_path.exists() {
            if let Ok(content) = fs::read_to_string(&dev_req_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        let parts: Vec<&str> = trimmed
                            .split(&['=', '>', '<', '~', '!'][..])
                            .map(|s| s.trim())
                            .collect();

                        if !parts.is_empty() {
                            let name = parts[0].to_string();
                            let version = if parts.len() > 1 && !parts[1].is_empty() {
                                Some(parts[1].to_string())
                            } else {
                                None
                            };

                            dependencies.push(Dependency {
                                name,
                                version,
                                is_dev: true,
                            });
                        }
                    }
                }
            }
        }

        // TODO: Add setup.py parsing for more complex Python projects
    }

    // Detect entry points in the project based on project type
    fn detect_entry_points(
        &self,
        project_dir: &Path,
        project_type: &ProjectType,
    ) -> Vec<EntryPoint> {
        let mut entry_points = Vec::new();

        match project_type {
            ProjectType::Rust => self.detect_rust_entry_points(project_dir, &mut entry_points),
            ProjectType::Node => self.detect_node_entry_points(project_dir, &mut entry_points),
            ProjectType::Python => self.detect_python_entry_points(project_dir, &mut entry_points),
            _ => {
                // Other project types not implemented yet
                debug!(
                    "Entry point detection not implemented for {:?}",
                    project_type
                );
            }
        }

        entry_points
    }

    // Detect Rust entry points (main.rs, lib.rs and bin/*.rs)
    fn detect_rust_entry_points(&self, project_dir: &Path, entry_points: &mut Vec<EntryPoint>) {
        // Check for src/main.rs
        let main_path = project_dir.join("src/main.rs");
        if main_path.exists() {
            entry_points.push(EntryPoint {
                path: "src/main.rs".to_string(),
                entry_type: "main".to_string(),
                description: "Main executable entry point".to_string(),
            });
        }

        // Check for src/lib.rs
        let lib_path = project_dir.join("src/lib.rs");
        if lib_path.exists() {
            entry_points.push(EntryPoint {
                path: "src/lib.rs".to_string(),
                entry_type: "library".to_string(),
                description: "Library crate entry point".to_string(),
            });
        }

        // Check for bin directory
        let bin_path = project_dir.join("src/bin");
        if bin_path.exists() && bin_path.is_dir() {
            if let Ok(entries) = fs::read_dir(&bin_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && {
                        let this = path.extension();
                        if let Some(t) = this {
                            fn fun_name(ext: &std::ffi::OsStr) -> bool {
                                ext == "rs"
                            }
                            (fun_name)(t)
                        } else {
                            false
                        }
                    } {
                        let relative_path =
                            format!("src/bin/{}", path.file_name().unwrap().to_string_lossy());

                        entry_points.push(EntryPoint {
                            path: relative_path.clone(),
                            entry_type: "binary".to_string(),
                            description: format!(
                                "Binary executable: {}",
                                path.file_stem().unwrap().to_string_lossy()
                            ),
                        });
                    }
                }
            }
        }

        // Check for examples directory
        let examples_path = project_dir.join("examples");
        if examples_path.exists() && examples_path.is_dir() {
            if let Ok(entries) = fs::read_dir(&examples_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or_else(|| false, |ext| ext == "rs") {
                        let relative_path =
                            format!("examples/{}", path.file_name().unwrap().to_string_lossy());

                        entry_points.push(EntryPoint {
                            path: relative_path.clone(),
                            entry_type: "example".to_string(),
                            description: format!(
                                "Example program: {}",
                                path.file_stem().unwrap().to_string_lossy()
                            ),
                        });
                    }
                }
            }
        }
    }

    // Detect Node.js entry points
    fn detect_node_entry_points(&self, project_dir: &Path, entry_points: &mut Vec<EntryPoint>) {
        // Check for package.json to find "main" entry point
        let package_path = project_dir.join("package.json");
        if package_path.exists() {
            if let Ok(content) = fs::read_to_string(&package_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    // Look for main field
                    if let Some(main) = json.get("main").and_then(|m| m.as_str()) {
                        entry_points.push(EntryPoint {
                            path: main.to_string(),
                            entry_type: "main".to_string(),
                            description: "Main entry point from package.json".to_string(),
                        });
                    }

                    // Look for bin field
                    if let Some(bin) = json.get("bin") {
                        if let Some(bin_str) = bin.as_str() {
                            entry_points.push(EntryPoint {
                                path: bin_str.to_string(),
                                entry_type: "binary".to_string(),
                                description: "Binary executable".to_string(),
                            });
                        } else if let Some(bin_obj) = bin.as_object() {
                            for (name, path_value) in bin_obj {
                                if let Some(path_str) = path_value.as_str() {
                                    entry_points.push(EntryPoint {
                                        path: path_str.to_string(),
                                        entry_type: "binary".to_string(),
                                        description: format!("Binary executable: {}", name),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check for index.js in the root or src directory
        let index_path = project_dir.join("index.js");
        if index_path.exists() {
            entry_points.push(EntryPoint {
                path: "index.js".to_string(),
                entry_type: "main".to_string(),
                description: "Root index.js file".to_string(),
            });
        }

        let src_index_path = project_dir.join("src/index.js");
        if src_index_path.exists() {
            entry_points.push(EntryPoint {
                path: "src/index.js".to_string(),
                entry_type: "main".to_string(),
                description: "Source directory index.js file".to_string(),
            });
        }
    }

    // Detect Python entry points
    fn detect_python_entry_points(&self, project_dir: &Path, entry_points: &mut Vec<EntryPoint>) {
        // Look for setup.py
        let setup_path = project_dir.join("setup.py");
        if setup_path.exists() {
            entry_points.push(EntryPoint {
                path: "setup.py".to_string(),
                entry_type: "setup".to_string(),
                description: "Python package setup script".to_string(),
            });
        }

        // Look for __main__.py files
        if let Ok(entries) = walk_dir(project_dir, self.config.max_depth) {
            entries.into_iter().for_each(|entry| {
                if let Ok(entry) = entry {
                    let path = entry.path();

                    if path.is_file() && path.file_name().is_some_and(|name| name == "__main__.py")
                    {
                        // Get relative path
                        if let Ok(relative) = path.strip_prefix(project_dir) {
                            let relative_path = relative.to_string_lossy().to_string();

                            entry_points.push(EntryPoint {
                                path: relative_path.clone(),
                                entry_type: "main".to_string(),
                                description: format!(
                                    "Python module entry point: {}",
                                    relative_path
                                ),
                            });
                        }
                    }
                }
            });
        }

        // Look for app.py, main.py, run.py in root or src
        for name in &["app.py", "main.py", "run.py"] {
            let file_path = project_dir.join(name);
            if file_path.exists() {
                entry_points.push(EntryPoint {
                    path: name.to_string(),
                    entry_type: "main".to_string(),
                    description: format!("Python main script: {}", name),
                });
            }

            let src_file_path = project_dir.join(format!("src/{}", name));
            if src_file_path.exists() {
                entry_points.push(EntryPoint {
                    path: format!("src/{}", name),
                    entry_type: "main".to_string(),
                    description: format!("Python main script in src: {}", name),
                });
            }
        }
    }

    // Get common project directories based on the project type
    fn get_project_directories(
        &self,
        project_dir: &Path,
        project_type: &ProjectType,
    ) -> HashMap<String, String> {
        let mut directories = HashMap::new();

        // Common directories across project types
        self.add_directory_if_exists(
            project_dir,
            "src",
            "Source code directory",
            &mut directories,
        );
        self.add_directory_if_exists(project_dir, "test", "Test directory", &mut directories);
        self.add_directory_if_exists(project_dir, "tests", "Tests directory", &mut directories);
        self.add_directory_if_exists(project_dir, "docs", "Documentation", &mut directories);

        // Project-specific directories
        match project_type {
            ProjectType::Rust => {
                self.add_directory_if_exists(
                    project_dir,
                    "benches",
                    "Benchmarks",
                    &mut directories,
                );
                self.add_directory_if_exists(
                    project_dir,
                    "examples",
                    "Example code",
                    &mut directories,
                );
                self.add_directory_if_exists(
                    project_dir,
                    "target",
                    "Build artifacts",
                    &mut directories,
                );
            }
            ProjectType::Node => {
                self.add_directory_if_exists(
                    project_dir,
                    "node_modules",
                    "Dependencies",
                    &mut directories,
                );
                self.add_directory_if_exists(
                    project_dir,
                    "dist",
                    "Distribution files",
                    &mut directories,
                );
                self.add_directory_if_exists(
                    project_dir,
                    "public",
                    "Public assets",
                    &mut directories,
                );
            }
            ProjectType::Python => {
                self.add_directory_if_exists(
                    project_dir,
                    "venv",
                    "Virtual environment",
                    &mut directories,
                );
                self.add_directory_if_exists(
                    project_dir,
                    "env",
                    "Virtual environment",
                    &mut directories,
                );
                self.add_directory_if_exists(
                    project_dir,
                    "__pycache__",
                    "Python cache",
                    &mut directories,
                );
            }
            _ => {}
        }

        directories
    }

    // Helper to add directory to map if it exists
    fn add_directory_if_exists(
        &self,
        base_dir: &Path,
        rel_path: &str,
        description: &str,
        map: &mut HashMap<String, String>,
    ) {
        let dir_path = base_dir.join(rel_path);
        if dir_path.exists() && dir_path.is_dir() {
            map.insert(rel_path.to_string(), description.to_string());
        }
    }

    // Analyze project structure and build FileInfo tree
    fn analyze_structure(&self, project_dir: &Path, include_hidden: bool) -> Result<FileInfo> {
        // Get initial info for the root directory
        let root_dir_name = project_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());

        let mut root_info = FileInfo {
            path: root_dir_name,
            is_dir: true,
            size: 0,
            children: Vec::new(),
            file_type: Some("project-root".to_string()),
        };

        // Build the structure recursively
        self.scan_directory(
            project_dir,
            project_dir,
            &mut root_info.children,
            0,
            include_hidden,
        )?;

        Ok(root_info)
    }

    // Recursively scan a directory to build the file structure
    fn scan_directory(
        &self,
        base_dir: &Path,
        current_dir: &Path,
        children: &mut Vec<FileInfo>,
        depth: usize,
        include_hidden: bool,
    ) -> Result<()> {
        // Check depth limit
        if depth >= self.config.max_depth {
            return Ok(());
        }

        // Read directory entries
        let entries = fs::read_dir(current_dir)?;

        for entry_result in entries {
            let entry = entry_result?;
            let path = entry.path();

            // Get file/directory name
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip hidden files if not including them
            if !include_hidden && name_str.starts_with('.') {
                continue;
            }

            // Skip directories in the skip list
            if path.is_dir()
                && self
                    .config
                    .skip_dirs
                    .iter()
                    .any(|dir| dir == name_str.as_ref())
            {
                continue;
            }

            // Skip files with extensions in the skip list
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = format!(".{}", ext.to_string_lossy());
                    if self
                        .config
                        .skip_extensions
                        .iter()
                        .any(|skip_ext| skip_ext == &ext_str)
                    {
                        continue;
                    }
                }
            }

            // Get relative path from project root
            let relative_path = path
                .strip_prefix(base_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            // Create FileInfo
            if path.is_dir() {
                let mut dir_info = FileInfo {
                    path: relative_path,
                    is_dir: true,
                    size: 0,
                    children: Vec::new(),
                    file_type: self.detect_directory_type(&path),
                };

                // Recursively scan subdirectory
                self.scan_directory(
                    base_dir,
                    &path,
                    &mut dir_info.children,
                    depth + 1,
                    include_hidden,
                )?;

                children.push(dir_info);
            } else if path.is_file() {
                // Get file metadata
                if let Ok(metadata) = fs::metadata(&path) {
                    let size = metadata.len();

                    // Skip files larger than max_file_size
                    if size <= self.config.max_file_size as u64 {
                        children.push(FileInfo {
                            path: relative_path,
                            is_dir: false,
                            size,
                            children: Vec::new(),
                            file_type: self.detect_file_type(&path),
                        });
                    }
                }
            }
        }

        // Sort children by path
        children.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(())
    }

    // Detect what type of directory this is
    fn detect_directory_type(&self, path: &Path) -> Option<String> {
        let dir_name = path.file_name()?.to_string_lossy();

        match dir_name.as_ref() {
            "src" => Some("source".to_string()),
            "test" | "tests" => Some("test".to_string()),
            "docs" | "documentation" => Some("documentation".to_string()),
            "examples" => Some("examples".to_string()),
            "bin" => Some("binaries".to_string()),
            "lib" => Some("libraries".to_string()),
            "node_modules" | "target" | "build" | "dist" => Some("build-output".to_string()),
            "public" | "static" | "assets" => Some("assets".to_string()),
            "config" | "conf" => Some("configuration".to_string()),
            "scripts" => Some("scripts".to_string()),
            _ => None,
        }
    }

    // Detect what type of file this is
    fn detect_file_type(&self, path: &Path) -> Option<String> {
        let file_name = path.file_name()?.to_string_lossy();

        // Check common config files
        if [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "setup.py",
            "requirements.txt",
            "go.mod",
            "pom.xml",
            "build.gradle",
            ".gitignore",
            ".dockerignore",
            "Dockerfile",
            "docker-compose.yml",
            "tsconfig.json",
            "webpack.config.js",
            "babel.config.js",
        ]
        .contains(&file_name.as_ref())
        {
            return Some("config".to_string());
        }

        // Check by extension
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();

            match ext_str.as_ref() {
                // Source code files
                "rs" | "js" | "ts" | "jsx" | "tsx" | "py" | "go" | "java" | "c" | "cpp" | "h"
                | "hpp" | "cs" | "php" | "rb" | "swift" | "kt" => {
                    // Check if it's a test file
                    if file_name.contains("test") || file_name.contains("spec") {
                        return Some("test-source".to_string());
                    }
                    Some("source".to_string())
                }

                // Documentation files
                "md" | "txt" | "rst" | "adoc" => Some("documentation".to_string()),

                // Config files
                "toml" | "json" | "yaml" | "yml" | "ini" | "conf" | "config" => {
                    Some("config".to_string())
                }

                // Lock files
                "lock" => Some("lock".to_string()),

                // Template files
                "html" | "hbs" | "ejs" | "tpl" | "template" => Some("template".to_string()),

                // Asset files
                "css" | "scss" | "sass" | "less" | "svg" | "png" | "jpg" | "jpeg" | "gif"
                | "ico" | "woff" | "woff2" | "ttf" | "eot" => Some("asset".to_string()),

                _ => None,
            }
        } else {
            // Files without extensions - check for special names
            match file_name.as_ref() {
                "README" | "LICENSE" | "CONTRIBUTING" | "CHANGELOG" => {
                    Some("documentation".to_string())
                }
                "Makefile" | "makefile" => Some("build".to_string()),
                _ => None,
            }
        }
    }
}

// Helper function to walk a directory recursively
fn walk_dir(dir: &Path, max_depth: usize) -> Result<Vec<walkdir::Result<walkdir::DirEntry>>> {
    use walkdir::WalkDir;

    // Set up the walker with max depth
    let walker = WalkDir::new(dir).max_depth(max_depth).into_iter();

    // Collect entries
    let entries: Vec<_> = walker.collect();

    Ok(entries)
}

#[async_trait]
impl Tool for ProjectNavigator {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "project".to_string(),
            name: "Project Navigator".to_string(),
            description: "Analyzes project structure and dependencies".to_string(),
            category: ToolCategory::Utility,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_dir": {
                        "type": "string",
                        "description": "Path to the project root directory"
                    },
                    "include_hidden": {
                        "type": "boolean",
                        "description": "Whether to include hidden files and directories",
                        "default": false
                    },
                    "analyze_dependencies": {
                        "type": "boolean",
                        "description": "Whether to analyze project dependencies",
                        "default": true
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum directory depth to analyze",
                        "default": 10
                    }
                },
                "required": ["project_dir"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "project_type": {
                        "type": "string",
                        "description": "Detected project type"
                    },
                    "structure": {
                        "type": "object",
                        "description": "Directory and file hierarchy"
                    },
                    "directories": {
                        "type": "object",
                        "description": "Important directories in the project"
                    },
                    "entry_points": {
                        "type": "array",
                        "description": "Detected entry points to the application",
                        "items": {
                            "type": "object"
                        }
                    },
                    "dependencies": {
                        "type": "array",
                        "description": "Project dependencies",
                        "items": {
                            "type": "object"
                        }
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let project_dir = params["project_dir"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing required parameter: 'project_dir'"))?;

        let include_hidden = params["include_hidden"].as_bool().unwrap_or(false);

        let analyze_dependencies = params["analyze_dependencies"].as_bool().unwrap_or(true);

        let max_depth = params["max_depth"]
            .as_u64()
            .unwrap_or(self.config.max_depth as u64) as usize;

        // Check if the project directory path is allowed
        if !self.is_path_allowed(project_dir) {
            return Ok(ToolResult {
                tool_id: "project".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": "Access to this directory is not allowed for security reasons"
                }),
                error: Some(
                    "Access to this directory is not allowed for security reasons".to_string(),
                ),
            });
        }

        // Check if the directory exists
        let dir_path = PathBuf::from(project_dir);
        if !dir_path.exists() || !dir_path.is_dir() {
            return Ok(ToolResult {
                tool_id: "project".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": format!("Directory does not exist or is not a directory: {}", project_dir)
                }),
                error: Some(format!(
                    "Directory does not exist or is not a directory: {}",
                    project_dir
                )),
            });
        }

        // Override max_depth from config if specified in params
        let mut config = self.config.clone();
        config.max_depth = max_depth;

        // Create the navigator with the updated config
        let navigator = ProjectNavigator::with_config(config);

        // Detect project type
        let project_type = navigator.detect_project_type(&dir_path);

        // Analyze project structure
        let structure = match navigator.analyze_structure(&dir_path, include_hidden) {
            Ok(structure) => structure,
            Err(e) => {
                return Ok(ToolResult {
                    tool_id: "project".to_string(),
                    status: ToolStatus::Failure,
                    output: json!({
                        "error": format!("Failed to analyze project structure: {}", e)
                    }),
                    error: Some(format!("Failed to analyze project structure: {}", e)),
                });
            }
        };

        // Get common project directories
        let directories = navigator.get_project_directories(&dir_path, &project_type);

        // Detect entry points
        let entry_points = navigator.detect_entry_points(&dir_path, &project_type);

        // Parse dependencies if requested
        let dependencies = if analyze_dependencies {
            navigator.parse_dependencies(&dir_path, &project_type)
        } else {
            Vec::new()
        };

        // Create the result
        Ok(ToolResult {
            tool_id: "project".to_string(),
            status: ToolStatus::Success,
            output: json!({
                "project_type": project_type.to_string(),
                "structure": structure,
                "directories": directories,
                "entry_points": entry_points,
                "dependencies": dependencies
            }),
            error: None,
        })
    }
}
