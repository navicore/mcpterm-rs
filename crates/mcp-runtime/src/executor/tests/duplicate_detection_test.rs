#[cfg(test)]
mod tests {
    use crate::executor::{clear_executed_tools, should_execute_tool};
    use serde_json::{json, Value};

    #[test]
    fn test_duplicate_tool_detection() {
        // Clear any existing tool executions
        clear_executed_tools();

        // Test parameters
        let tool_id = "shell";
        let params1 = json!({
            "command": "cargo new hello_world"
        });
        let params2 = json!({
            "command": "cargo new hello_world"
        });
        let params3 = json!({
            "command": "mkdir test_dir"
        });

        // First execution should be allowed
        assert!(should_execute_tool(tool_id, &params1), "First execution should be allowed");

        // Same command again should be blocked
        assert!(!should_execute_tool(tool_id, &params2), "Duplicate execution should be blocked");

        // Different command should be allowed
        assert!(should_execute_tool(tool_id, &params3), "Different command should be allowed");

        // After clearing, the same command should be allowed again
        clear_executed_tools();
        assert!(should_execute_tool(tool_id, &params1), "After clearing, execution should be allowed again");
    }

    #[test]
    fn test_different_tool_ids_with_same_params() {
        // Clear any existing tool executions
        clear_executed_tools();

        // Test parameters
        let tool_id1 = "shell";
        let tool_id2 = "command";
        let params = json!({
            "command": "cargo new hello_world"
        });

        // First execution with tool_id1 should be allowed
        assert!(should_execute_tool(tool_id1, &params), "First execution with tool_id1 should be allowed");

        // Same params with tool_id2 should be allowed (different tool)
        assert!(should_execute_tool(tool_id2, &params), "Same params with different tool_id should be allowed");
    }

    #[test]
    fn test_tool_returns_correct_status() {
        // This tests that our implementation in the execute_tool method works correctly
        // But since we can't easily test the executor directly here, this is just
        // a placeholder for a more comprehensive test
        
        // In a real implementation, we would:
        // 1. Create a mock ToolManager
        // 2. Create a ToolExecutor with the mock
        // 3. Execute the same tool twice
        // 4. Verify the first one executes normally
        // 5. Verify the second one returns the "skipped" status
    }
}