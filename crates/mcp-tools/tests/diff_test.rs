use mcp_tools::diff::{DiffTool, DiffConfig};
use mcp_tools::{Tool, ToolStatus};
use serde_json::json;
use tempfile::tempdir;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[tokio::test]
async fn test_diff_tool_with_strings() {
    // Create a basic diff tool
    let diff_tool = DiffTool::new();
    
    // Test comparing two simple strings
    let old_content = "line 1\nline 2\nline 3\n";
    let new_content = "line 1\nmodified line\nline 3\n";
    
    let result = diff_tool.execute(json!({
        "old_content": old_content,
        "new_content": new_content,
        "output_format": "unified"
    })).await.unwrap();
    
    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);
    assert!(result.output["diff"].as_str().unwrap().contains("-line 2"));
    assert!(result.output["diff"].as_str().unwrap().contains("+modified line"));
    assert_eq!(result.output["stats"]["inserted"].as_i64().unwrap(), 1);
    assert_eq!(result.output["stats"]["deleted"].as_i64().unwrap(), 1);
}

#[tokio::test]
async fn test_diff_tool_with_files() {
    // Create temporary directory
    let dir = tempdir().unwrap();
    
    // Create two test files
    let file1_path = dir.path().join("old.txt");
    let file2_path = dir.path().join("new.txt");
    
    let mut file1 = File::create(&file1_path).unwrap();
    write!(file1, "This is line one.\nThis is line two.\nThis is line three.\n").unwrap();
    
    let mut file2 = File::create(&file2_path).unwrap();
    write!(file2, "This is line one.\nThis is a modified line.\nThis is line three.\nThis is line four.\n").unwrap();
    
    // Create a diff tool with allowed paths
    let diff_tool = DiffTool::with_config(DiffConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None, // Override the default denied paths
        ..DiffConfig::default()
    });
    
    // Test comparing files
    let result = diff_tool.execute(json!({
        "old_file": file1_path.to_string_lossy(),
        "new_file": file2_path.to_string_lossy(),
        "output_format": "inline"
    })).await.unwrap();
    
    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);
    
    // Should be array format for inline
    let diff_lines = result.output["diff"].as_array().unwrap();
    
    // Check for specific changes
    let found_deletion = diff_lines.iter().any(|line| {
        line["change_type"].as_str() == Some("delete") &&
        line["content"].as_str().unwrap().contains("line two")
    });
    
    let found_insertion_modified = diff_lines.iter().any(|line| {
        line["change_type"].as_str() == Some("insert") &&
        line["content"].as_str().unwrap().contains("modified line")
    });
    
    let found_insertion_four = diff_lines.iter().any(|line| {
        line["change_type"].as_str() == Some("insert") &&
        line["content"].as_str().unwrap().contains("line four")
    });
    
    assert!(found_deletion);
    assert!(found_insertion_modified);
    assert!(found_insertion_four);
    
    // Check stats
    assert_eq!(result.output["stats"]["inserted"].as_i64().unwrap(), 2);
    assert_eq!(result.output["stats"]["deleted"].as_i64().unwrap(), 1);
    
    // Files compared should include our two files
    let files_compared = result.output["files_compared"].as_array().unwrap();
    assert_eq!(files_compared.len(), 2);
}

#[tokio::test]
async fn test_diff_tool_whitespace_handling() {
    // Create a diff tool
    let diff_tool = DiffTool::new();
    
    // Test comparing strings with different whitespace - more significant differences
    let old_content = "function hello() {\n    console.log('hello');\n}";
    let new_content = "function  hello(  )  {\nconsole.log(  'hello'  );\n  }";
    
    // First compare with whitespace sensitivity (should show differences)
    let result_sensitive = diff_tool.execute(json!({
        "old_content": old_content,
        "new_content": new_content,
        "ignore_whitespace": false
    })).await.unwrap();
    
    // Then compare with whitespace insensitivity
    let result_insensitive = diff_tool.execute(json!({
        "old_content": old_content,
        "new_content": new_content,
        "ignore_whitespace": true
    })).await.unwrap();
    
    // Sensitive diff should show changes
    assert!(result_sensitive.output["stats"]["unchanged"].as_i64().unwrap() < 3);
    
    // Insensitive diff should show no changes or fewer changes
    assert!(result_insensitive.output["stats"]["unchanged"].as_i64().unwrap() > 
            result_sensitive.output["stats"]["unchanged"].as_i64().unwrap());
}

#[tokio::test]
async fn test_diff_tool_denied_path() {
    // Create a diff tool with default denied paths
    let diff_tool = DiffTool::new();
    
    // Try to access /etc/passwd (should be denied by default security policy)
    let result = diff_tool.execute(json!({
        "old_file": "/etc/passwd",
        "new_content": "test"
    })).await.unwrap();
    
    // Verify access is denied
    assert_eq!(result.status, ToolStatus::Failure);
    assert!(result.output["error"].as_str().unwrap().contains("not allowed"));
}

#[tokio::test]
async fn test_diff_tool_changes_format() {
    // Create a diff tool
    let diff_tool = DiffTool::new();
    
    // Test with changes-only format
    // Make sure content is different enough to detect
    let old_content = "line 1\nline 2\nline 3\nline 4\n";
    let new_content = "line 1\nmodified line\nline 3\nadded line\n";
    
    let result = diff_tool.execute(json!({
        "old_content": old_content,
        "new_content": new_content,
        "output_format": "changes"
    })).await.unwrap();
    
    // Verify result
    assert_eq!(result.status, ToolStatus::Success);
    
    // Should be array format for changes
    let diff_lines = result.output["diff"].as_array().unwrap();
    
    // Should only include changed lines (not unchanged ones)
    for line in diff_lines {
        let change_type = line["change_type"].as_str().unwrap();
        assert!(change_type == "insert" || change_type == "delete", 
                "Changes format should only include inserts and deletes");
    }
    
    // Print out the actual lines for debugging
    println!("Diff lines count: {}", diff_lines.len());
    for (i, line) in diff_lines.iter().enumerate() {
        println!("Line {}: type={}, content={}", 
            i,
            line["change_type"].as_str().unwrap_or("unknown"), 
            line["content"].as_str().unwrap_or("unknown")
        );
    }
    
    // Exact match for line 2
    let has_deleted_line2 = diff_lines.iter().any(|line| {
        let line_type = line["change_type"].as_str();
        let content = line["content"].as_str();
        line_type == Some("delete") && 
        content.map_or(false, |c| c.trim() == "line 2")
    });
    
    let has_deleted_line4 = diff_lines.iter().any(|line| {
        let line_type = line["change_type"].as_str();
        let content = line["content"].as_str();
        line_type == Some("delete") &&
        content.map_or(false, |c| c.trim() == "line 4")
    });
    
    let has_added_modified = diff_lines.iter().any(|line| {
        let line_type = line["change_type"].as_str();
        let content = line["content"].as_str();
        line_type == Some("insert") && 
        content.map_or(false, |c| c.trim() == "modified line")
    });
    
    let has_added_line = diff_lines.iter().any(|line| {
        let line_type = line["change_type"].as_str();
        let content = line["content"].as_str();
        line_type == Some("insert") && 
        content.map_or(false, |c| c.trim() == "added line")
    });
    
    assert!(has_deleted_line2);
    assert!(has_deleted_line4);
    assert!(has_added_modified);
    assert!(has_added_line);
}