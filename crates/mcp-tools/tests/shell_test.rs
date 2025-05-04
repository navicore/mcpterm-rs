#[cfg(test)]
mod tests {
    use mcp_tools::shell::ShellTool;
    use mcp_tools::{Tool, ToolStatus};
    use serde_json::json;

    #[tokio::test]
    async fn test_shell_tool_execution() {
        let tool = ShellTool::new();
        let result = tool
            .execute(json!({
                "command": "echo 'test'",
                "timeout": 1000
            }))
            .await
            .unwrap();

        assert_eq!(result.status, ToolStatus::Success);
        assert_eq!(result.tool_id, "shell");
        assert!(result.error.is_none());
    }
}
