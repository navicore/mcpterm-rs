use anyhow::Result;
use mcp_tools::{
    search::{GrepConfig, GrepTool},
    Tool, ToolStatus,
};
use serde_json::{json, Value};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

// Helper to create a temporary test file with content
fn create_test_file(dir: &PathBuf, filename: &str, content: &str) -> Result<PathBuf> {
    let file_path = dir.join(filename);
    let mut file = File::create(&file_path)?;
    writeln!(file, "{}", content)?;
    Ok(file_path)
}

#[tokio::test]
async fn test_grep_tool_simple_search() -> Result<()> {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    println!("Test is running...");

    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());
    println!("Temp directory: {:?}", temp_dir.path());

    // Create a test file
    let test_content = "This is a test file.\nIt contains some sample text.\nAnd it has multiple lines.\nThis line contains test and file words.";
    let _file_path = create_test_file(&test_dir, "test1.txt", test_content)?;

    // Create a grep tool with a specific configuration for this test:
    // 1. Allow the temp directory
    // 2. Don't include any of the default denied paths
    let config = GrepConfig {
        allowed_paths: Some(vec![temp_dir.path().to_string_lossy().to_string()]),
        denied_paths: None, // Override the default denied paths
        max_matches: 1000,
        max_files: 1000,
        max_file_size: 10 * 1024 * 1024, // 10 MB
        default_context_lines: 2,
    };

    println!(
        "GrepTool config: allowed_paths = {:?}",
        config.allowed_paths
    );
    let grep_tool = GrepTool::with_config(config);

    // Test simple search for "test"
    let params = json!({
        "pattern": "test",
        "path": temp_dir.path().to_string_lossy().to_string(),
        "recursive": true
    });

    let result = grep_tool.execute(params).await?;

    // Print error for debugging
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
    }

    // Verify the result
    assert_eq!(
        result.status,
        ToolStatus::Success,
        "Tool failed with error: {:?}",
        result.error
    );
    assert!(result.error.is_none());

    // Get the matches from the output
    let matches = result.output["matches"].as_array().unwrap();

    // Should have found at least 2 matches ("test" appears twice)
    assert!(matches.len() >= 2);

    // Verify one of the matches
    let first_match = &matches[0];
    assert!(first_match["matched_text"]
        .as_str()
        .unwrap()
        .contains("test"));

    Ok(())
}

#[tokio::test]
async fn test_grep_tool_with_include_pattern() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create multiple files with different extensions
    create_test_file(
        &test_dir,
        "code.rs",
        "fn test_function() {\n    println!(\"This is a test\");\n}",
    )?;
    create_test_file(
        &test_dir,
        "readme.md",
        "# Test Project\nThis is a test readme file.",
    )?;
    create_test_file(&test_dir, "config.json", "{ \"test\": true }")?;

    // Create a grep tool with a specific configuration for this test:
    // 1. Allow the temp directory
    // 2. Don't include any of the default denied paths
    let config = GrepConfig {
        allowed_paths: Some(vec![temp_dir.path().to_string_lossy().to_string()]),
        denied_paths: None, // Override the default denied paths
        max_matches: 1000,
        max_files: 1000,
        max_file_size: 10 * 1024 * 1024, // 10 MB
        default_context_lines: 2,
    };

    println!(
        "GrepTool config: allowed_paths = {:?}",
        config.allowed_paths
    );
    let grep_tool = GrepTool::with_config(config);

    // Test search for "test" only in .rs files
    let params = json!({
        "pattern": "test",
        "path": temp_dir.path().to_string_lossy().to_string(),
        "include": "*.rs",
        "recursive": true
    });

    let result = grep_tool.execute(params).await?;

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the matches from the output
    let matches = result.output["matches"].as_array().unwrap();

    // Should have found matches only in the .rs file
    for m in matches {
        let file_path = m["file"].as_str().unwrap();
        assert!(file_path.ends_with(".rs"));
        assert!(!file_path.ends_with(".md"));
        assert!(!file_path.ends_with(".json"));
    }

    Ok(())
}

#[tokio::test]
async fn test_grep_tool_with_context_lines() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create a test file with multiple lines
    let test_content = "Line 1\nLine 2\nLine 3\nLine test 4\nLine 5\nLine 6\nLine 7";
    let _file_path = create_test_file(&test_dir, "context_test.txt", test_content)?;

    // Create a grep tool with a specific configuration for this test:
    // 1. Allow the temp directory
    // 2. Don't include any of the default denied paths
    let config = GrepConfig {
        allowed_paths: Some(vec![temp_dir.path().to_string_lossy().to_string()]),
        denied_paths: None, // Override the default denied paths
        max_matches: 1000,
        max_files: 1000,
        max_file_size: 10 * 1024 * 1024, // 10 MB
        default_context_lines: 2,
    };

    println!(
        "GrepTool config: allowed_paths = {:?}",
        config.allowed_paths
    );
    let grep_tool = GrepTool::with_config(config);

    // Test search with context lines set to 2
    let params = json!({
        "pattern": "test",
        "path": temp_dir.path().to_string_lossy().to_string(),
        "context_lines": 2,
        "recursive": true
    });

    let result = grep_tool.execute(params).await?;

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the matches from the output
    let matches = result.output["matches"].as_array().unwrap();
    assert!(!matches.is_empty());

    // Verify the context lines
    let first_match = &matches[0];

    // Should have 2 lines before
    let context_before = first_match["context_before"].as_array().unwrap();
    assert_eq!(context_before.len(), 2);
    assert_eq!(context_before[0].as_str().unwrap(), "Line 2");
    assert_eq!(context_before[1].as_str().unwrap(), "Line 3");

    // Should have 2 lines after
    let context_after = first_match["context_after"].as_array().unwrap();
    assert_eq!(context_after.len(), 2);
    assert_eq!(context_after[0].as_str().unwrap(), "Line 5");
    assert_eq!(context_after[1].as_str().unwrap(), "Line 6");

    Ok(())
}

#[tokio::test]
async fn test_grep_tool_case_insensitive() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create a test file with mixed case
    let test_content = "This is a test file.\nTEST is uppercase.\nAnother test lowercase.";
    let _file_path = create_test_file(&test_dir, "case_test.txt", test_content)?;

    // Create a grep tool with a specific configuration for this test:
    // 1. Allow the temp directory
    // 2. Don't include any of the default denied paths
    let config = GrepConfig {
        allowed_paths: Some(vec![temp_dir.path().to_string_lossy().to_string()]),
        denied_paths: None, // Override the default denied paths
        max_matches: 1000,
        max_files: 1000,
        max_file_size: 10 * 1024 * 1024, // 10 MB
        default_context_lines: 2,
    };

    println!(
        "GrepTool config: allowed_paths = {:?}",
        config.allowed_paths
    );
    let grep_tool = GrepTool::with_config(config);

    // Test case-insensitive search (default)
    let params = json!({
        "pattern": "test",
        "path": temp_dir.path().to_string_lossy().to_string(),
        "recursive": true
    });

    let result = grep_tool.execute(params).await?;

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the matches from the output
    let matches = result.output["matches"].as_array().unwrap();

    // Should have found at least 3 matches (case insensitive by default)
    assert!(matches.len() >= 3);

    // Test case-sensitive search
    let params = json!({
        "pattern": "test",
        "path": temp_dir.path().to_string_lossy().to_string(),
        "case_sensitive": true,
        "recursive": true
    });

    let result = grep_tool.execute(params).await?;

    // Get the matches from the output
    let matches = result.output["matches"].as_array().unwrap();

    // Should have found only 2 matches (lowercase "test")
    assert_eq!(matches.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_grep_tool_denied_paths() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create a test file
    let test_content = "This is a test file.";
    let _file_path = create_test_file(&test_dir, "test1.txt", test_content)?;

    // Create a subdirectory called "secrets"
    let secrets_dir = test_dir.join("secrets");
    fs::create_dir(&secrets_dir)?;
    let _secret_file = create_test_file(&secrets_dir, "secret.txt", "This is a secret test file.")?;

    // Create a grep tool with a specific configuration for this test:
    // 1. Allow the temp directory
    // 2. Deny the secrets subdirectory
    // 3. Don't include any of the default denied paths
    let config = GrepConfig {
        allowed_paths: Some(vec![temp_dir.path().to_string_lossy().to_string()]),
        denied_paths: Some(vec![format!("{}/", secrets_dir.to_string_lossy())]), // Add trailing slash to match exactly
        max_matches: 1000,
        max_files: 1000,
        max_file_size: 10 * 1024 * 1024, // 10 MB
        default_context_lines: 2,
    };

    let grep_tool = GrepTool::with_config(config);

    // Test search that includes the denied path
    let params = json!({
        "pattern": "test",
        "path": temp_dir.path().to_string_lossy().to_string(),
        "recursive": true
    });

    let result = grep_tool.execute(params).await?;

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the matches from the output
    let matches = result.output["matches"].as_array().unwrap();

    // Should only find matches in the allowed files, not in secret.txt
    for m in matches {
        let file_path = m["file"].as_str().unwrap();
        assert!(!file_path.contains("secret"));
    }

    Ok(())
}

#[tokio::test]
async fn test_grep_tool_invalid_regex() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;

    // Create a grep tool with a specific configuration for this test:
    // 1. Allow the temp directory
    // 2. Don't include any of the default denied paths
    let config = GrepConfig {
        allowed_paths: Some(vec![temp_dir.path().to_string_lossy().to_string()]),
        denied_paths: None, // Override the default denied paths
        max_matches: 1000,
        max_files: 1000,
        max_file_size: 10 * 1024 * 1024, // 10 MB
        default_context_lines: 2,
    };

    println!(
        "GrepTool config: allowed_paths = {:?}",
        config.allowed_paths
    );
    let grep_tool = GrepTool::with_config(config);

    // Test invalid regex pattern
    let params = json!({
        "pattern": "[invalid regex(",
        "path": temp_dir.path().to_string_lossy().to_string()
    });

    let result = grep_tool.execute(params).await?;

    // Verify the result indicates failure
    assert_eq!(result.status, ToolStatus::Failure);
    assert!(result.error.is_some());

    // Error message should mention invalid regex
    let error_msg = result.error.unwrap();
    assert!(error_msg.contains("Invalid regex"));

    Ok(())
}
