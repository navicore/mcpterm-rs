use mcp_tools::diff::{PatchConfig, PatchTool};
use mcp_tools::{Tool, ToolStatus};
use serde_json::json;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

#[tokio::test]
async fn test_patch_tool_basic() {
    // Create temporary directory
    let dir = tempdir().unwrap();

    // Create a test file to be patched
    let file_path = dir.path().join("file.txt");
    let mut file = File::create(&file_path).unwrap();
    write!(
        file,
        "This is line one.\nThis is line two.\nThis is line three.\n"
    )
    .unwrap();

    // Create a patch in unified format
    let patch_content = "@@ -1,3 +1,3 @@
 This is line one.
-This is line two.
+This is a modified line.
 This is line three.
";

    // Create a patch tool with allowed paths
    let patch_tool = PatchTool::with_config(PatchConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None, // Override the default denied paths
        create_backup: true,
        max_file_size: 10 * 1024 * 1024, // 10 MB
    });

    // Apply the patch
    let result = patch_tool
        .execute(json!({
            "target_file": file_path.to_string_lossy(),
            "patch_content": patch_content,
            "create_backup": true
        }))
        .await
        .unwrap();

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);
    assert_eq!(result.output["success"].as_bool().unwrap(), true);
    assert_eq!(result.output["hunks_applied"].as_i64().unwrap(), 1);
    assert_eq!(result.output["hunks_failed"].as_i64().unwrap(), 0);

    // Verify the file was actually patched
    let patched_content = fs::read_to_string(&file_path).unwrap();
    assert!(patched_content.contains("This is a modified line."));
    assert!(!patched_content.contains("This is line two."));

    // Verify a backup was created
    let backup_path = result.output["backup_created"].as_str().unwrap();
    assert!(PathBuf::from(backup_path).exists());

    // Check the backup content
    let backup_content = fs::read_to_string(backup_path).unwrap();
    assert!(backup_content.contains("This is line two."));
}

#[tokio::test]
async fn test_patch_tool_dry_run() {
    // Create temporary directory
    let dir = tempdir().unwrap();

    // Create a test file to be patched
    let file_path = dir.path().join("file.txt");
    let mut file = File::create(&file_path).unwrap();
    let original_content = "Line 1\nLine 2\nLine 3\n";
    write!(file, "{}", original_content).unwrap();

    // Create a patch in unified format
    let patch_content = "@@ -1,3 +1,3 @@
 Line 1
-Line 2
+Modified Line
 Line 3
";

    // Create a patch tool with allowed paths
    let patch_tool = PatchTool::with_config(PatchConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        create_backup: false,
        max_file_size: 10 * 1024 * 1024,
    });

    // Apply the patch in dry-run mode
    let result = patch_tool
        .execute(json!({
            "target_file": file_path.to_string_lossy(),
            "patch_content": patch_content,
            "dry_run": true
        }))
        .await
        .unwrap();

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);
    assert_eq!(result.output["success"].as_bool().unwrap(), true);
    assert_eq!(result.output["hunks_applied"].as_i64().unwrap(), 1);

    // Verify the file was NOT modified (dry run)
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_content, original_content);

    // Verify no backup was created
    assert!(result.output["backup_created"].is_null());
}

#[tokio::test]
async fn test_patch_tool_conflict() {
    // Create temporary directory
    let dir = tempdir().unwrap();

    // Create a test file
    let file_path = dir.path().join("file.txt");
    let mut file = File::create(&file_path).unwrap();
    write!(file, "Line 1\nThis is completely different\nLine 3\n").unwrap();

    // Create a patch that won't match the file content
    let patch_content = "@@ -1,3 +1,3 @@
 Line 1
-Line 2 that doesn't exist
+Modified Line
 Line 3
";

    // Create a patch tool with allowed paths
    let patch_tool = PatchTool::with_config(PatchConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        create_backup: true,
        max_file_size: 10 * 1024 * 1024,
    });

    // Try to apply the patch (should fail with conflicts)
    let result = patch_tool
        .execute(json!({
            "target_file": file_path.to_string_lossy(),
            "patch_content": patch_content
        }))
        .await
        .unwrap();

    // Verify the conflict is detected
    assert_eq!(result.status, ToolStatus::Failure);
    assert_eq!(result.output["success"].as_bool().unwrap(), false);
    assert_eq!(result.output["hunks_applied"].as_i64().unwrap(), 0);
    assert_eq!(result.output["hunks_failed"].as_i64().unwrap(), 1);

    // Should have conflict information
    let conflicts = result.output["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert!(conflicts[0].as_str().unwrap().contains("failed"));
}

#[tokio::test]
async fn test_patch_tool_multiple_hunks() {
    // Create temporary directory
    let dir = tempdir().unwrap();

    // Create a test file with multiple lines
    let file_path = dir.path().join("file.txt");
    let mut file = File::create(&file_path).unwrap();
    write!(file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\n").unwrap();

    // Create a patch with multiple hunks
    let patch_content = "@@ -1,3 +1,3 @@
 Line 1
-Line 2
+Modified Line 2
 Line 3
@@ -4,3 +4,3 @@
 Line 4
-Line 5
+Modified Line 5
 Line 6
";

    // Create a patch tool with allowed paths
    let patch_tool = PatchTool::with_config(PatchConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        create_backup: true,
        max_file_size: 10 * 1024 * 1024,
    });

    // Apply the patch
    let result = patch_tool
        .execute(json!({
            "target_file": file_path.to_string_lossy(),
            "patch_content": patch_content
        }))
        .await
        .unwrap();

    // Verify both hunks were applied
    assert_eq!(result.status, ToolStatus::Success);
    assert_eq!(result.output["success"].as_bool().unwrap(), true);
    assert_eq!(result.output["hunks_applied"].as_i64().unwrap(), 2);

    // Verify the file content
    let patched_content = fs::read_to_string(&file_path).unwrap();
    println!("Patched content: '{}'", patched_content);

    // Verify that the modifications were applied correctly
    assert!(patched_content.contains("Modified Line 2"));
    assert!(patched_content.contains("Modified Line 5"));

    // Verify that the exact lines "Line 2" and "Line 5" as standalone lines don't exist
    let lines: Vec<&str> = patched_content.lines().collect();
    assert!(!lines.contains(&"Line 2"));
    assert!(!lines.contains(&"Line 5"));
}

#[tokio::test]
async fn test_patch_tool_partial_success() {
    // Create temporary directory
    let dir = tempdir().unwrap();

    // Create a test file
    let file_path = dir.path().join("file.txt");
    let mut file = File::create(&file_path).unwrap();
    write!(
        file,
        "Line 1\nLine 2\nLine 3\nLine 4\nDifferent line 5\nLine 6\n"
    )
    .unwrap();

    // Create a patch with two hunks (one will succeed, one will fail)
    let patch_content = "@@ -1,3 +1,3 @@
 Line 1
-Line 2
+Modified Line 2
 Line 3
@@ -4,3 +4,3 @@
 Line 4
-Line 5 that doesn't match
+Modified Line 5
 Line 6
";

    // Create a patch tool with allowed paths
    let patch_tool = PatchTool::with_config(PatchConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        create_backup: true,
        max_file_size: 10 * 1024 * 1024,
    });

    // Apply the patch
    let result = patch_tool
        .execute(json!({
            "target_file": file_path.to_string_lossy(),
            "patch_content": patch_content
        }))
        .await
        .unwrap();

    // Verify partial success
    assert_eq!(result.status, ToolStatus::Failure);
    assert_eq!(result.output["success"].as_bool().unwrap(), false);
    assert_eq!(result.output["hunks_applied"].as_i64().unwrap(), 1);
    assert_eq!(result.output["hunks_failed"].as_i64().unwrap(), 1);

    // Verify the content (first hunk applied, second failed)
    let patched_content = fs::read_to_string(&file_path).unwrap();
    assert!(patched_content.contains("Modified Line 2"));
    assert!(patched_content.contains("Different line 5")); // Unchanged
}

#[tokio::test]
async fn test_patch_tool_denied_path() {
    // Create a patch tool with default security
    let patch_tool = PatchTool::new();

    // Try to patch a sensitive file
    let result = patch_tool
        .execute(json!({
            "target_file": "/etc/passwd",
            "patch_content": "@@ -1,1 +1,1 @@\n-line\n+modified\n"
        }))
        .await
        .unwrap();

    // Verify the operation is denied
    assert_eq!(result.status, ToolStatus::Failure);
    assert!(result.output["error"]
        .as_str()
        .unwrap()
        .contains("not allowed"));
}

#[tokio::test]
async fn test_patch_tool_nonexistent_file() {
    // Create temporary directory for allowed path
    let dir = tempdir().unwrap();

    // Create a patch tool
    let patch_tool = PatchTool::with_config(PatchConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        create_backup: true,
        max_file_size: 10 * 1024 * 1024,
    });

    // Try to patch a file that doesn't exist
    let nonexistent_file = dir.path().join("nonexistent.txt");

    let result = patch_tool
        .execute(json!({
            "target_file": nonexistent_file.to_string_lossy(),
            "patch_content": "@@ -1,1 +1,1 @@\n-line\n+modified\n"
        }))
        .await
        .unwrap();

    // Verify the operation fails
    assert_eq!(result.status, ToolStatus::Failure);
    assert!(result.output["error"]
        .as_str()
        .unwrap()
        .contains("not exist"));
}
