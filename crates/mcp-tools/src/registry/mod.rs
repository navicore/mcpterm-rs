use crate::analysis::{LanguageAnalyzerTool, ProjectNavigator};
use crate::diff::{DiffTool, PatchTool};
use crate::filesystem::{FilesystemConfig, ListDirectoryTool, ReadFileTool, WriteFileTool};
use crate::search::{FindConfig, FindTool, GrepConfig, GrepTool};
use crate::shell::{ShellConfig, ShellTool};
use crate::testing::TestRunnerTool;
use crate::ToolManager;

/// Default shell configuration with security restrictions
pub fn default_shell_config() -> ShellConfig {
    ShellConfig {
        default_timeout_ms: 30000, // 30 seconds default timeout
        max_timeout_ms: 300000,    // 5 minutes maximum timeout
        allowed_commands: None,    // No specific whitelist
        denied_commands: Some(vec![
            "rm -rf".to_string(),   // Prevent dangerous recursive deletion
            "sudo".to_string(),     // Prevent sudo commands
            "chmod".to_string(),    // Prevent permission changes
            "chown".to_string(),    // Prevent ownership changes
            "mkfs".to_string(),     // Prevent formatting
            "dd".to_string(),       // Prevent raw disk operations
            "shutdown".to_string(), // Prevent shutdown
            "reboot".to_string(),   // Prevent reboot
            "halt".to_string(),     // Prevent halt
        ]),
    }
}

/// Default filesystem configuration with security restrictions
pub fn default_filesystem_config() -> FilesystemConfig {
    FilesystemConfig {
        // Use default denied paths to protect sensitive areas
        denied_paths: Some(vec![
            "/etc/".to_string(),
            "/var/".to_string(),
            "/usr/".to_string(),
            "/bin/".to_string(),
            "/sbin/".to_string(),
            "/.ssh/".to_string(),
            "/.aws/".to_string(),
            "/.config/".to_string(),
            "C:\\Windows\\".to_string(),
            "C:\\Program Files\\".to_string(),
            "C:\\Program Files (x86)\\".to_string(),
        ]),
        allowed_paths: None, // Allow all paths not explicitly denied
        max_file_size: 10 * 1024 * 1024, // 10 MB max file size
    }
}

/// Create a tool manager with default configuration
pub fn create_tool_manager() -> ToolManager {
    create_tool_manager_with_config(
        default_shell_config(),
        default_filesystem_config(),
    )
}

/// Create a tool manager with custom configuration
pub fn create_tool_manager_with_config(
    shell_config: ShellConfig,
    filesystem_config: FilesystemConfig,
) -> ToolManager {
    let mut tool_manager = ToolManager::new();

    // Register shell tool
    let shell_tool = ShellTool::with_config(shell_config);
    tool_manager.register_tool(Box::new(shell_tool));

    // Register filesystem tools
    let read_file_tool = ReadFileTool::with_config(filesystem_config.clone());
    tool_manager.register_tool(Box::new(read_file_tool));

    let write_file_tool = WriteFileTool::with_config(filesystem_config.clone());
    tool_manager.register_tool(Box::new(write_file_tool));

    let list_dir_tool = ListDirectoryTool::with_config(filesystem_config.clone());
    tool_manager.register_tool(Box::new(list_dir_tool));

    // Register search tools
    let grep_config = GrepConfig {
        denied_paths: filesystem_config.denied_paths.clone(),
        allowed_paths: filesystem_config.allowed_paths.clone(),
        ..GrepConfig::default()
    };
    let grep_tool = GrepTool::with_config(grep_config);
    tool_manager.register_tool(Box::new(grep_tool));

    let find_config = FindConfig {
        denied_paths: filesystem_config.denied_paths.clone(),
        allowed_paths: filesystem_config.allowed_paths.clone(),
        ..FindConfig::default()
    };
    let find_tool = FindTool::with_config(find_config);
    tool_manager.register_tool(Box::new(find_tool));

    // Register diff and patch tools
    let diff_tool = DiffTool::new();
    tool_manager.register_tool(Box::new(diff_tool));

    let patch_tool = PatchTool::new();
    tool_manager.register_tool(Box::new(patch_tool));

    // Register project navigator tool
    let project_navigator = ProjectNavigator::new();
    tool_manager.register_tool(Box::new(project_navigator));

    // Register language analyzer tool
    let language_analyzer = LanguageAnalyzerTool::new();
    tool_manager.register_tool(Box::new(language_analyzer));

    // Register test runner tool
    let test_runner = TestRunnerTool::new();
    tool_manager.register_tool(Box::new(test_runner));

    tool_manager
}

/// Register standard tools to an existing tool manager
pub fn register_standard_tools(
    tool_manager: &mut ToolManager,
    shell_config: ShellConfig,
    filesystem_config: FilesystemConfig,
) {
    // Register shell tool
    let shell_tool = ShellTool::with_config(shell_config);
    tool_manager.register_tool(Box::new(shell_tool));

    // Register filesystem tools
    let read_file_tool = ReadFileTool::with_config(filesystem_config.clone());
    tool_manager.register_tool(Box::new(read_file_tool));

    let write_file_tool = WriteFileTool::with_config(filesystem_config.clone());
    tool_manager.register_tool(Box::new(write_file_tool));

    let list_dir_tool = ListDirectoryTool::with_config(filesystem_config.clone());
    tool_manager.register_tool(Box::new(list_dir_tool));

    // Register search tools
    let grep_config = GrepConfig {
        denied_paths: filesystem_config.denied_paths.clone(),
        allowed_paths: filesystem_config.allowed_paths.clone(),
        ..GrepConfig::default()
    };
    let grep_tool = GrepTool::with_config(grep_config);
    tool_manager.register_tool(Box::new(grep_tool));

    let find_config = FindConfig {
        denied_paths: filesystem_config.denied_paths.clone(),
        allowed_paths: filesystem_config.allowed_paths.clone(),
        ..FindConfig::default()
    };
    let find_tool = FindTool::with_config(find_config);
    tool_manager.register_tool(Box::new(find_tool));

    // Register diff and patch tools
    let diff_tool = DiffTool::new();
    tool_manager.register_tool(Box::new(diff_tool));

    let patch_tool = PatchTool::new();
    tool_manager.register_tool(Box::new(patch_tool));

    // Register project navigator tool
    let project_navigator = ProjectNavigator::new();
    tool_manager.register_tool(Box::new(project_navigator));

    // Register language analyzer tool
    let language_analyzer = LanguageAnalyzerTool::new();
    tool_manager.register_tool(Box::new(language_analyzer));

    // Register test runner tool
    let test_runner = TestRunnerTool::new();
    tool_manager.register_tool(Box::new(test_runner));
}