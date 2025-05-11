use super::ToolExecutor;
use mcp_tools::{
    create_tool_manager, create_tool_manager_with_config, default_filesystem_config,
    default_shell_config, filesystem::FilesystemConfig, shell::ShellConfig, ToolManager,
};
use std::sync::Arc;
use tracing::debug;

/// Factory for creating tool executors with standard tools
pub struct ToolFactory;

impl ToolFactory {
    /// Create a new ToolExecutor with default tool configuration
    pub fn create_executor() -> ToolExecutor {
        debug!("Creating ToolExecutor with default configuration");
        let tool_manager = create_tool_manager();
        ToolExecutor::new(tool_manager)
    }

    /// Create a new ToolExecutor with custom tool configuration
    pub fn create_executor_with_config(
        shell_config: ShellConfig,
        filesystem_config: FilesystemConfig,
    ) -> ToolExecutor {
        debug!("Creating ToolExecutor with custom configuration");
        let tool_manager = create_tool_manager_with_config(shell_config, filesystem_config);
        ToolExecutor::new(tool_manager)
    }

    /// Create a new ToolExecutor with an existing ToolManager
    pub fn create_executor_with_manager(tool_manager: ToolManager) -> ToolExecutor {
        debug!("Creating ToolExecutor with existing ToolManager (taking ownership)");
        ToolExecutor::new(tool_manager)
    }

    /// Create a new ToolExecutor with a shared ToolManager
    pub fn create_executor_with_shared_manager(tool_manager: Arc<ToolManager>) -> ToolExecutor {
        debug!("Creating ToolExecutor with shared ToolManager");
        ToolExecutor::with_shared_manager(tool_manager)
    }

    /// Create a default shell configuration
    pub fn default_shell_config() -> ShellConfig {
        default_shell_config()
    }

    /// Create a default filesystem configuration
    pub fn default_filesystem_config() -> FilesystemConfig {
        default_filesystem_config()
    }

    /// Create a shared ToolManager with default configuration
    pub fn create_shared_tool_manager() -> Arc<ToolManager> {
        debug!("Creating shared ToolManager with default configuration");
        Arc::new(create_tool_manager())
    }
}
