use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

// Export the template module
pub mod template;
pub use template::TemplateEngine;

/// Prompt type indicating the role or purpose of a prompt
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PromptType {
    /// System prompt used to provide instructions to the LLM
    System,
    /// Initial user prompts for specific tasks
    Initial,
    /// Default prompts for specific tools or functions
    Tool(String),
    /// MCP system prompt without tool documentation
    McpSystem,
    /// MCP system prompt template for use with tool documentation
    McpSystemWithTools,
    /// Custom prompts defined by the user
    Custom(String),
}

impl PromptType {
    /// Convert PromptType to a valid filename
    fn to_filename(&self) -> String {
        match self {
            PromptType::System => "system.txt".to_string(),
            PromptType::Initial => "initial.txt".to_string(),
            PromptType::Tool(name) => format!("tool_{}.txt", name),
            PromptType::McpSystem => "mcp_system.txt".to_string(),
            PromptType::McpSystemWithTools => "mcp_system_with_tools.txt".to_string(),
            PromptType::Custom(name) => format!("custom_{}.txt", name),
        }
    }

    /// Create a PromptType from a filename
    fn from_filename(filename: &str) -> Option<Self> {
        let filename = filename.to_lowercase();

        if filename == "system.txt" {
            Some(PromptType::System)
        } else if filename == "initial.txt" {
            Some(PromptType::Initial)
        } else if filename == "mcp_system.txt" {
            Some(PromptType::McpSystem)
        } else if filename == "mcp_system_with_tools.txt" {
            Some(PromptType::McpSystemWithTools)
        } else if filename.starts_with("tool_") && filename.ends_with(".txt") {
            let name = filename
                .strip_prefix("tool_")?
                .strip_suffix(".txt")?
                .to_string();
            Some(PromptType::Tool(name))
        } else if filename.starts_with("custom_") && filename.ends_with(".txt") {
            let name = filename
                .strip_prefix("custom_")?
                .strip_suffix(".txt")?
                .to_string();
            Some(PromptType::Custom(name))
        } else {
            None
        }
    }
}

/// Manager for prompt resources that loads prompts from the config directory
pub struct PromptManager {
    /// Map of prompt type to prompt content
    prompts: HashMap<PromptType, String>,
    /// The base directory where prompts are stored
    base_dir: PathBuf,
}

impl PromptManager {
    /// Create a new prompt manager with default prompts
    pub fn new() -> Self {
        let base_dir = Self::get_default_prompt_dir();

        // Create a new manager with an empty prompt map
        let mut manager = Self {
            prompts: HashMap::new(),
            base_dir,
        };

        // Try to load prompts, but don't fail if we can't - we'll use defaults
        if let Err(e) = manager.load_all_prompts() {
            warn!("Could not load prompts, using defaults: {}", e);
            manager.initialize_default_prompts();
        }

        manager
    }

    /// Create a new prompt manager with a specific base directory
    pub fn with_base_dir<P: AsRef<Path>>(base_dir: P) -> Self {
        let base_dir_path = base_dir.as_ref().to_path_buf();

        // Create a new manager with an empty prompt map
        let mut manager = Self {
            prompts: HashMap::new(),
            base_dir: base_dir_path.clone(),
        };

        // Try to load prompts, but don't fail if we can't - we'll use defaults
        if let Err(e) = manager.load_all_prompts() {
            warn!(
                "Could not load prompts from {}, using defaults: {}",
                base_dir_path.display(),
                e
            );
            manager.initialize_default_prompts();
        }

        manager
    }

    /// Get the default prompt directory
    fn get_default_prompt_dir() -> PathBuf {
        let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push("mcpterm");
        dir.push("prompts");
        dir
    }

    /// Initialize the manager with default prompts
    fn initialize_default_prompts(&mut self) {
        // Add default system prompt
        self.prompts.insert(
            PromptType::System,
            r#"You are an AI assistant that helps users with software tasks.

You can assist with:
- Analyzing code and explaining how it works
- Writing new code based on user requirements
- Debugging and fixing issues in existing code
- Suggesting improvements and optimizations

When helping the user, prefer to search and understand their code before making changes.
"#
            .to_string(),
        );

        // Add default MCP system prompt (previously in McpSchemaManager)
        self.prompts.insert(
            PromptType::McpSystem,
            r#"You are an AI assistant that follows the Model Context Protocol (MCP).
You MUST communicate using valid JSON in the JSON-RPC 2.0 format.

Here are the rules:

1. For regular responses, use:
{
  "jsonrpc": "2.0",
  "result": "Your message here...",
  "id": "<request_id>"
}

2. For tool calls, use:
{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "<tool_name>",
    "parameters": {
      // Tool-specific parameters
    }
  },
  "id": "<request_id>"
}

3. For errors, use:
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Error description"
  },
  "id": "<request_id>"
}

Available tools:

1. "shell": Execute a shell command
   Parameters: {
     "command": "string",           // The shell command to execute
     "timeout": "number"            // Optional: timeout in milliseconds
   }

2. "file_read": Read a file
   Parameters: {
     "path": "string"               // Absolute path to the file
   }

3. "file_write": Write to a file
   Parameters: {
     "path": "string",              // Absolute path to the file
     "content": "string",           // Content to write
     "append": "boolean"            // Optional: append instead of overwrite
   }

4. "directory_list": List files in a directory
   Parameters: {
     "path": "string"               // Absolute path to the directory
   }

5. "grep": Search file contents with regex patterns
   Parameters: {
     "pattern": "string",           // Regex pattern to search for
     "path": "string",              // Directory to search in
     "include": "string",           // Optional: Glob pattern for files to include
     "exclude": "string",           // Optional: Glob pattern for files to exclude
     "context_lines": "number",     // Optional: Number of context lines to include
     "max_matches": "number",       // Optional: Maximum number of matches to return
     "case_sensitive": "boolean",   // Optional: Whether to use case-sensitive matching
     "recursive": "boolean"         // Optional: Whether to search recursively
   }

6. "find": Find files matching name patterns
   Parameters: {
     "pattern": "string",           // Glob pattern to match files
     "base_dir": "string",          // Base directory for search
     "exclude": "string",           // Optional: Glob pattern for files to exclude
     "max_depth": "number",         // Optional: Maximum directory depth
     "modified_after": "string",    // Optional: Only find files modified after (YYYY-MM-DD)
     "modified_before": "string",   // Optional: Only find files modified before (YYYY-MM-DD)
     "sort_by": "string",           // Optional: Sort by "name", "size", or "modified_time"
     "order": "string",             // Optional: "asc" or "desc"
     "include_dirs": "boolean"      // Optional: Include directories in results
   }

Always ensure your responses are syntactically valid JSON.
Never include multiple JSON objects in a single response.
If you require more information or the result of a tool call, make a tool call request and wait for the result.

When working with a codebase, first use the 'find' and 'grep' tools to explore and understand the code
before making changes or executing commands.

IMPORTANT - Task Completion:
After completing a task (such as creating files, running commands, etc.), always:
1. Send a clear message confirming the task is complete
2. Summarize what was done (files created, changes made, etc.)
3. Offer relevant next steps (like building, testing, or running the code)
4. If you created or modified files, explain their purpose or structure

For example, after creating a project, say something like:
"✓ Successfully created the Rust project! The project structure includes:
- hello_world/src/main.rs: Contains the main program with Hello World code
- hello_world/Cargo.toml: Project configuration file

Would you like me to:
- Build and run the project?
- Explain the main.rs file?
- Modify the code to do something more interesting?"
"#.to_string(),
        );

        // Add MCP system prompt template for dynamic tool documentation
        self.prompts.insert(
            PromptType::McpSystemWithTools,
            r#"You are an AI assistant that follows the Model Context Protocol (MCP).
You MUST communicate using valid JSON in the JSON-RPC 2.0 format.

Here are the rules:

1. For regular responses, use:
{
  "jsonrpc": "2.0",
  "result": "Your message here...",
  "id": "<request_id>"
}

2. For tool calls, use:
{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "<tool_name>",
    "parameters": {
      // Tool-specific parameters
    }
  },
  "id": "<request_id>"
}

3. For errors, use:
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Error description"
  },
  "id": "<request_id>"
}

Available tools:

{{tool_documentation}}

Always ensure your responses are syntactically valid JSON.
Never include multiple JSON objects in a single response.
If you require more information or the result of a tool call, make a tool call request and wait for the result.

When working with a codebase, first use the 'find' and 'grep' tools to explore and understand the code
before making changes or executing commands.
"#.to_string(),
        );

        // Add a default initial prompt
        self.prompts.insert(
            PromptType::Initial,
            "I am a helpful AI assistant. How can I help you today?".to_string(),
        );

        // Add a default tool prompt for shell
        self.prompts.insert(
            PromptType::Tool("shell".to_string()),
            r#"When executing shell commands, I should:
1. Be careful with potentially destructive commands
2. Always explain what the command does before running it
3. Format command output for readability
4. Avoid running commands that could expose sensitive information
5. Consider security implications of commands before execution"#
                .to_string(),
        );

        // Add a default tool prompt for patch
        self.prompts.insert(
            PromptType::Tool("patch".to_string()),
            r#"When modifying files, prefer using the patch tool when:
1. Making precise changes to a specific part of a file
2. Applying multiple changes across different parts of a file
3. Working with changes that depend on existing content/context
4. Comparing file versions and applying differences

To use the patch tool effectively:
1. Specify the target_file path
2. Provide a unified diff format with context in patch_content
3. Include a few lines of context before and after the change
4. CRITICAL: All newlines MUST be properly escaped as "\\n" in the patch_content
5. CRITICAL: The JSON must be valid - no raw newlines, tabs, or control characters
6. Use the format: @@ -line,count +line,count @@ followed by context lines (space prefix), removals (- prefix), and additions (+ prefix)

Example valid patch call (note all newlines are escaped as \\n):
{
  "jsonrpc": "2.0",
  "method": "mcp.tool_call",
  "params": {
    "name": "patch",
    "parameters": {
      "target_file": "example.txt",
      "patch_content": "@@ -10,4 +10,5 @@\\n unchanged line\\n unchanged line\\n-line to remove\\n+line to add instead\\n unchanged line"
    }
  },
  "id": "1"
}

NEVER include raw newlines, tabs, or other control characters in JSON. Always escape them properly."#
                .to_string(),
        );
    }

    /// Load all prompts from the base directory
    pub fn load_all_prompts(&mut self) -> Result<()> {
        // Create the directory if it doesn't exist
        if !self.base_dir.exists() {
            debug!("Creating prompt directory at {}", self.base_dir.display());
            fs::create_dir_all(&self.base_dir)?;
        }

        // Clear existing prompts
        self.prompts.clear();

        // Load existing prompts from files first
        debug!("Loading prompts from {}", self.base_dir.display());

        let mut found_prompt_types = Vec::new();

        // Check if directory exists and is readable
        if self.base_dir.exists() {
            // Read all files in the directory
            let entries = fs::read_dir(&self.base_dir)?;

            for entry in entries {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                        if let Some(prompt_type) = PromptType::from_filename(filename) {
                            // Read prompt content
                            let content = fs::read_to_string(&path)?;

                            // Add prompt to the manager
                            self.prompts.insert(prompt_type.clone(), content);
                            found_prompt_types.push(prompt_type.clone());
                            debug!("Loaded prompt: {:?}", prompt_type);
                        }
                    }
                }
            }
        }

        // Initialize with default prompts for any that weren't found in files
        self.initialize_default_prompts();

        // Write default prompts to files, but ONLY if they don't exist already
        for (prompt_type, content) in &self.prompts {
            // Skip if we already loaded this prompt type
            if found_prompt_types.contains(prompt_type) {
                continue;
            }

            // Construct the file path
            let filename = prompt_type.to_filename();
            let file_path = self.base_dir.join(&filename);

            // Only write if the file doesn't exist
            if !file_path.exists() {
                debug!("Creating default prompt file: {}", file_path.display());
                fs::write(&file_path, content)?;
            }
        }

        if found_prompt_types.is_empty() {
            info!("Created default prompts in {}", self.base_dir.display());
        } else {
            info!(
                "Loaded {} prompts from {}",
                found_prompt_types.len(),
                self.base_dir.display()
            );
        }

        Ok(())
    }

    /// Get a prompt by type
    pub fn get_prompt(&self, prompt_type: &PromptType) -> Option<&str> {
        self.prompts.get(prompt_type).map(|s| s.as_str())
    }

    /// Get the system prompt (convenience method)
    pub fn get_system_prompt(&self) -> &str {
        self.get_prompt(&PromptType::System).unwrap_or_default()
    }

    /// Get the MCP system prompt (convenience method)
    pub fn get_mcp_system_prompt(&self) -> &str {
        self.get_prompt(&PromptType::McpSystem).unwrap_or_default()
    }

    /// Get the MCP system prompt with custom tool documentation
    pub fn get_mcp_system_prompt_with_tools(&self, tools_doc: &str) -> String {
        let template = self
            .get_prompt(&PromptType::McpSystemWithTools)
            .unwrap_or_default();
        let engine = TemplateEngine::new().with_var("tool_documentation", tools_doc);
        engine.render(template)
    }

    /// Get a tool-specific prompt (convenience method)
    pub fn get_tool_prompt(&self, tool_name: &str) -> Option<&str> {
        self.get_prompt(&PromptType::Tool(tool_name.to_string()))
    }

    /// Get a prompt with template variables substituted
    pub fn get_rendered_prompt(
        &self,
        prompt_type: &PromptType,
        engine: &TemplateEngine,
    ) -> Option<String> {
        self.get_prompt(prompt_type)
            .map(|template| engine.render(template))
    }

    /// Get the system prompt with template variables substituted (convenience method)
    pub fn get_rendered_system_prompt(&self, engine: &TemplateEngine) -> String {
        let template = self.get_system_prompt();
        engine.render(template)
    }

    /// Get a tool-specific prompt with template variables substituted (convenience method)
    pub fn get_rendered_tool_prompt(
        &self,
        tool_name: &str,
        engine: &TemplateEngine,
    ) -> Option<String> {
        self.get_tool_prompt(tool_name)
            .map(|template| engine.render(template))
    }

    /// Set a prompt with the given type and content
    pub fn set_prompt(&mut self, prompt_type: PromptType, content: String) -> Result<()> {
        // Update the prompt in memory
        self.prompts.insert(prompt_type.clone(), content.clone());

        // Save the prompt to a file
        self.save_prompt(&prompt_type, &content, false)?;

        Ok(())
    }

    /// Set a prompt with the given type and content, optionally not overwriting existing files
    pub fn set_prompt_safe(
        &mut self,
        prompt_type: PromptType,
        content: String,
        no_overwrite: bool,
    ) -> Result<()> {
        // Update the prompt in memory
        self.prompts.insert(prompt_type.clone(), content.clone());

        // Save the prompt to a file
        self.save_prompt(&prompt_type, &content, no_overwrite)?;

        Ok(())
    }

    /// Save a prompt to a file
    fn save_prompt(
        &self,
        prompt_type: &PromptType,
        content: &str,
        no_overwrite: bool,
    ) -> Result<()> {
        // Create the prompt directory if it doesn't exist
        if !self.base_dir.exists() {
            fs::create_dir_all(&self.base_dir)?;
        }

        // Get the filename for this prompt type
        let filename = prompt_type.to_filename();
        let path = self.base_dir.join(filename);

        // Check if file exists and we're in no_overwrite mode
        if no_overwrite && path.exists() {
            debug!("Not overwriting existing prompt file: {}", path.display());
            return Ok(());
        }

        // Write the prompt to the file
        fs::write(&path, content)?;

        debug!("Saved prompt {:?} to {}", prompt_type, path.display());
        Ok(())
    }

    /// Get all available prompt types
    pub fn get_available_prompts(&self) -> Vec<PromptType> {
        self.prompts.keys().cloned().collect()
    }
}

/// Default implementation for PromptManager
impl Default for PromptManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_prompt_type_filename_conversion() {
        // Test PromptType to filename
        assert_eq!(PromptType::System.to_filename(), "system.txt");
        assert_eq!(PromptType::Initial.to_filename(), "initial.txt");
        assert_eq!(
            PromptType::Tool("shell".to_string()).to_filename(),
            "tool_shell.txt"
        );
        assert_eq!(
            PromptType::Custom("my_prompt".to_string()).to_filename(),
            "custom_my_prompt.txt"
        );

        // Test filename to PromptType
        assert_eq!(
            PromptType::from_filename("system.txt"),
            Some(PromptType::System)
        );
        assert_eq!(
            PromptType::from_filename("initial.txt"),
            Some(PromptType::Initial)
        );
        assert_eq!(
            PromptType::from_filename("tool_shell.txt"),
            Some(PromptType::Tool("shell".to_string()))
        );
        assert_eq!(
            PromptType::from_filename("custom_my_prompt.txt"),
            Some(PromptType::Custom("my_prompt".to_string()))
        );

        // Test invalid filenames
        assert_eq!(PromptType::from_filename("invalid.txt"), None);
        assert_eq!(PromptType::from_filename("tool.txt"), None);
        assert_eq!(PromptType::from_filename("custom.txt"), None);
    }

    #[test]
    fn test_prompt_manager_default_prompts() {
        let manager = PromptManager::new();

        // Check that default prompts are available
        assert!(manager.get_prompt(&PromptType::System).is_some());
        assert!(manager.get_prompt(&PromptType::Initial).is_some());
        assert!(manager
            .get_prompt(&PromptType::Tool("shell".to_string()))
            .is_some());
    }

    #[test]
    fn test_prompt_manager_custom_dir() {
        // Create a temporary directory
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create a prompt manager with the temp directory
        let mut manager = PromptManager::with_base_dir(temp_path);

        // Set a custom prompt
        let custom_type = PromptType::Custom("test".to_string());
        let custom_content = "This is a test prompt.".to_string();

        manager
            .set_prompt(custom_type.clone(), custom_content.clone())
            .unwrap();

        // Check that the prompt file was created
        let prompt_path = temp_path.join("custom_test.txt");
        assert!(prompt_path.exists());

        // Check that the file content matches
        let file_content = fs::read_to_string(&prompt_path).unwrap();
        assert_eq!(file_content, custom_content);

        // Create a new manager with the same dir and check that it loads the prompt
        let manager2 = PromptManager::with_base_dir(temp_path);
        assert_eq!(
            manager2.get_prompt(&custom_type),
            Some(custom_content.as_str())
        );
    }

    #[test]
    fn test_prompt_manager_no_overwrite() {
        // Create a temporary directory
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create a prompt manager with the temp directory
        let mut manager = PromptManager::with_base_dir(temp_path);

        // Set a custom prompt
        let custom_type = PromptType::Custom("test".to_string());
        let original_content = "Original content.".to_string();

        manager
            .set_prompt(custom_type.clone(), original_content.clone())
            .unwrap();

        // Verify it was saved
        let prompt_path = temp_path.join("custom_test.txt");
        assert!(prompt_path.exists());
        let file_content = fs::read_to_string(&prompt_path).unwrap();
        assert_eq!(file_content, original_content);

        // Try to update with no_overwrite=true
        let new_content = "New content that should not be saved.".to_string();
        manager
            .set_prompt_safe(custom_type.clone(), new_content.clone(), true)
            .unwrap();

        // Verify the file wasn't changed
        let file_content = fs::read_to_string(&prompt_path).unwrap();
        assert_eq!(file_content, original_content);

        // But the in-memory content was updated
        assert_eq!(manager.get_prompt(&custom_type), Some(new_content.as_str()));

        // Now update with no_overwrite=false
        let final_content = "Final content that should be saved.".to_string();
        manager
            .set_prompt_safe(custom_type.clone(), final_content.clone(), false)
            .unwrap();

        // Verify the file was changed
        let file_content = fs::read_to_string(&prompt_path).unwrap();
        assert_eq!(file_content, final_content);
    }

    #[test]
    fn test_prompt_manager_with_template() {
        // Create a temporary directory
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create a prompt manager with the temp directory
        let mut manager = PromptManager::with_base_dir(temp_path);

        // Set a custom prompt with template variables
        let custom_type = PromptType::Custom("templated".to_string());
        let custom_content = "Hello, {{name}}! Your session started at {{time}}.".to_string();

        manager
            .set_prompt(custom_type.clone(), custom_content.clone())
            .unwrap();

        // Create a template engine with variables
        let engine = TemplateEngine::new()
            .with_var("name", "User")
            .with_var("time", "12:00");

        // Get the rendered prompt
        let rendered = manager.get_rendered_prompt(&custom_type, &engine).unwrap();

        // Check that variables were substituted
        assert_eq!(rendered, "Hello, User! Your session started at 12:00.");

        // Create a new engine with different variables
        let engine2 = TemplateEngine::new()
            .with_var("name", "Alice")
            .with_var("time", "15:30");

        // Get the rendered prompt with new variables
        let rendered2 = manager.get_rendered_prompt(&custom_type, &engine2).unwrap();

        // Check that variables were substituted with new values
        assert_eq!(rendered2, "Hello, Alice! Your session started at 15:30.");
    }
}
