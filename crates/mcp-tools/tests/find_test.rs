use anyhow::Result;
use mcp_tools::{
    search::{FindConfig, FindTool},
    Tool, ToolStatus,
};
use serde_json::json;
use std::io::Write;
use std::path::PathBuf;
use std::{
    fs::{self, File},
    path::Path,
};
use tempfile::tempdir;

// Helper to create a temporary test file with content
fn create_test_file(dir: &Path, filename: &str, content: &str) -> Result<PathBuf> {
    let file_path = dir.join(filename);
    let mut file = File::create(&file_path)?;
    writeln!(file, "{}", content)?;
    Ok(file_path)
}

#[tokio::test]
async fn test_find_tool_basic_search() -> Result<()> {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    // Print the temporary directory for debugging
    println!("Test is running...");

    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());
    println!("Temp directory: {:?}", temp_dir.path());

    // Create multiple test files of different types
    create_test_file(&test_dir, "file1.txt", "Test content")?;
    create_test_file(&test_dir, "file2.md", "# Markdown content")?;
    create_test_file(&test_dir, "script.js", "console.log('Hello');")?;

    // Create the find tool with custom config that allows the temp directory
    let temp_path_str = temp_dir.path().to_string_lossy().to_string();
    println!("Setting allowed path to: {}", temp_path_str);

    // Create a config that allows the temp directory and has NO denied paths
    let config = FindConfig {
        allowed_paths: Some(vec![temp_path_str]),
        denied_paths: None, // Override the default denied paths
        max_files: 1000,
        default_max_depth: 10,
    };

    // Print the configuration for debugging
    println!(
        "Find tool config: allowed_paths = {:?}",
        config.allowed_paths
    );

    let find_tool = FindTool::with_config(config);

    // Test basic search for all files
    let params = json!({
        "pattern": "*.*",
        "base_dir": temp_dir.path().to_string_lossy().to_string()
    });

    let result = find_tool.execute(params).await?;

    // Print detailed error information
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
        println!("Result output: {:?}", result.output);
    }

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);
    assert!(result.error.is_none());

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Should have found 3 files
    assert_eq!(files.len(), 3);

    // Verify file names
    let file_names: Vec<&str> = files.iter().map(|f| f["name"].as_str().unwrap()).collect();

    assert!(file_names.contains(&"file1.txt"));
    assert!(file_names.contains(&"file2.md"));
    assert!(file_names.contains(&"script.js"));

    Ok(())
}

#[tokio::test]
async fn test_find_tool_with_glob_pattern() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create multiple test files
    create_test_file(&test_dir, "file1.txt", "Test content")?;
    create_test_file(&test_dir, "file2.txt", "More test content")?;
    create_test_file(&test_dir, "image.png", "Binary content")?;
    create_test_file(&test_dir, "document.pdf", "PDF content")?;

    // Create the find tool with custom config that allows the temp directory
    let temp_path_str = temp_dir.path().to_string_lossy().to_string();

    // Create a config that allows the temp directory and has NO denied paths
    let config = FindConfig {
        allowed_paths: Some(vec![temp_path_str]),
        denied_paths: None, // Override the default denied paths
        max_files: 1000,
        default_max_depth: 10,
    };
    let find_tool = FindTool::with_config(config);

    // Test search for only .txt files
    let params = json!({
        "pattern": "*.txt",
        "base_dir": temp_dir.path().to_string_lossy().to_string()
    });

    let result = find_tool.execute(params).await?;

    // Print detailed error information
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
        println!("Result output: {:?}", result.output);
    }

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Should have found 2 files
    assert_eq!(files.len(), 2);

    // All files should be .txt files
    for file in files {
        let name = file["name"].as_str().unwrap();
        assert!(name.ends_with(".txt"));
    }

    Ok(())
}

#[tokio::test]
async fn test_find_tool_with_exclude_pattern() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create multiple test files
    create_test_file(&test_dir, "file1.txt", "Test content")?;
    create_test_file(&test_dir, "file2.txt", "More test content")?;
    create_test_file(&test_dir, "temp.txt", "Temporary content")?;
    create_test_file(&test_dir, "doc.pdf", "PDF content")?;

    // Create the find tool with custom config that allows the temp directory
    let temp_path_str = temp_dir.path().to_string_lossy().to_string();

    // Create a config that allows the temp directory and has NO denied paths
    let config = FindConfig {
        allowed_paths: Some(vec![temp_path_str]),
        denied_paths: None, // Override the default denied paths
        max_files: 1000,
        default_max_depth: 10,
    };
    let find_tool = FindTool::with_config(config);

    // Test search for all files except temp.txt
    let params = json!({
        "pattern": "*.*",
        "base_dir": temp_dir.path().to_string_lossy().to_string(),
        "exclude": "*temp*"
    });

    let result = find_tool.execute(params).await?;

    // Print detailed error information
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
        println!("Result output: {:?}", result.output);
    }

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Should have found 3 files (excluding temp.txt)
    assert_eq!(files.len(), 3);

    // None of the files should contain "temp" in their name
    for file in files {
        let name = file["name"].as_str().unwrap();
        assert!(!name.contains("temp"));
    }

    Ok(())
}

#[tokio::test]
async fn test_find_tool_sorting() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create files of different sizes
    let _small_file = create_test_file(&test_dir, "small.txt", "Small")?;
    let _medium_file = create_test_file(&test_dir, "medium.txt", "Medium content that is longer")?;
    let _large_file = create_test_file(
        &test_dir,
        "large.txt",
        "Large content that is even longer than the medium content",
    )?;

    // Create the find tool with custom config that allows the temp directory
    let temp_path_str = temp_dir.path().to_string_lossy().to_string();

    // Create a config that allows the temp directory and has NO denied paths
    let config = FindConfig {
        allowed_paths: Some(vec![temp_path_str]),
        denied_paths: None, // Override the default denied paths
        max_files: 1000,
        default_max_depth: 10,
    };
    let find_tool = FindTool::with_config(config);

    // Test sorting by size in ascending order
    let params = json!({
        "pattern": "*.txt",
        "base_dir": temp_dir.path().to_string_lossy().to_string(),
        "sort_by": "size",
        "order": "asc"
    });

    let result = find_tool.execute(params).await?;

    // Print detailed error information
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
        println!("Result output: {:?}", result.output);
    }

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Should have found 3 files
    assert_eq!(files.len(), 3);

    // Files should be sorted by size (ascending)
    let sizes: Vec<u64> = files.iter().map(|f| f["size"].as_u64().unwrap()).collect();

    // Check that sizes are in ascending order
    assert!(sizes[0] <= sizes[1] && sizes[1] <= sizes[2]);

    // Test sorting by name in descending order
    let params = json!({
        "pattern": "*.txt",
        "base_dir": temp_dir.path().to_string_lossy().to_string(),
        "sort_by": "name",
        "order": "desc"
    });

    let result = find_tool.execute(params).await?;

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Files should be sorted by name (descending)
    let names: Vec<&str> = files.iter().map(|f| f["name"].as_str().unwrap()).collect();

    // Check that names are in descending order
    assert!(names[0] >= names[1] && names[1] >= names[2]);

    Ok(())
}

#[tokio::test]
async fn test_find_tool_recursive_search() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create a nested directory structure
    let subdir1 = test_dir.join("subdir1");
    fs::create_dir(&subdir1)?;

    let subdir2 = subdir1.join("subdir2");
    fs::create_dir(&subdir2)?;

    // Create files in different directories
    create_test_file(&test_dir, "root.txt", "Root content")?;
    create_test_file(&subdir1, "level1.txt", "Level 1 content")?;
    create_test_file(&subdir2, "level2.txt", "Level 2 content")?;

    // Create the find tool with custom config that allows the temp directory
    let temp_path_str = temp_dir.path().to_string_lossy().to_string();

    // Create a config that allows the temp directory and has NO denied paths
    let config = FindConfig {
        allowed_paths: Some(vec![temp_path_str]),
        denied_paths: None, // Override the default denied paths
        max_files: 1000,
        default_max_depth: 10,
    };
    let find_tool = FindTool::with_config(config);

    // Test recursive search
    let params = json!({
        "pattern": "**/*.txt",
        "base_dir": temp_dir.path().to_string_lossy().to_string()
    });

    let result = find_tool.execute(params).await?;

    // Print detailed error information
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
        println!("Result output: {:?}", result.output);
    }

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Should have found 3 files across all directories
    assert_eq!(files.len(), 3);

    // Verify directories in paths
    let paths: Vec<&str> = files.iter().map(|f| f["path"].as_str().unwrap()).collect();

    // Check that we have files from all subdirectories
    assert!(paths.iter().any(|p| p.contains("subdir1")));
    assert!(paths.iter().any(|p| p.contains("subdir2")));

    Ok(())
}

#[tokio::test]
async fn test_find_tool_denied_paths() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create a "secrets" directory
    let secrets_dir = test_dir.join("secrets");
    fs::create_dir(&secrets_dir)?;

    // Create files in different directories
    create_test_file(&test_dir, "public.txt", "Public content")?;
    create_test_file(&secrets_dir, "private.txt", "Private content")?;

    // Print the paths for debugging
    println!("Temp directory: {}", temp_dir.path().display());
    println!("Secrets directory: {}", secrets_dir.display());

    // Create the find tool with a specific configuration for this test:
    // 1. Allow the temp directory
    // 2. Deny the secrets subdirectory
    // 3. Don't include any of the default denied paths
    let config = FindConfig {
        allowed_paths: Some(vec![temp_dir.path().to_string_lossy().to_string()]),
        denied_paths: Some(vec![format!("{}/", secrets_dir.to_string_lossy())]), // Add trailing slash to make it match exactly
        max_files: 1000,
        default_max_depth: 10,
    };

    println!("Denied paths: {:?}", config.denied_paths);

    let find_tool = FindTool::with_config(config);

    // Test search in the entire directory tree
    let params = json!({
        "pattern": "**/*.txt",
        "base_dir": temp_dir.path().to_string_lossy().to_string()
    });

    let result = find_tool.execute(params).await?;

    // Print detailed error information
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
        println!("Result output: {:?}", result.output);
    }

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Print all found files for debugging
    println!("Found {} files:", files.len());
    for (i, file) in files.iter().enumerate() {
        println!("File {}: path={}", i, file["path"].as_str().unwrap());
    }

    // Make sure at least one file with public.txt is found
    // and no file with "private.txt" is found
    let has_public = files.iter().any(|f| {
        let path = f["path"].as_str().unwrap();
        path.contains("public.txt")
    });
    let has_private = files.iter().any(|f| {
        let path = f["path"].as_str().unwrap();
        path.contains("private.txt")
    });

    assert!(has_public, "Should contain a file with 'public.txt'");
    assert!(
        !has_private,
        "Should not contain any file with 'private.txt'"
    );

    Ok(())
}

#[tokio::test]
async fn test_find_tool_include_directories() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create a subdirectory
    let subdir = test_dir.join("subdir");
    fs::create_dir(&subdir)?;

    // Create a file
    create_test_file(&test_dir, "file.txt", "File content")?;

    // Create the find tool with custom config that allows the temp directory
    let temp_path_str = temp_dir.path().to_string_lossy().to_string();

    // Create a config that allows the temp directory and has NO denied paths
    let config = FindConfig {
        allowed_paths: Some(vec![temp_path_str]),
        denied_paths: None, // Override the default denied paths
        max_files: 1000,
        default_max_depth: 10,
    };
    let find_tool = FindTool::with_config(config);

    // Test search with directories included
    let params = json!({
        "pattern": "*",
        "base_dir": temp_dir.path().to_string_lossy().to_string(),
        "include_dirs": true
    });

    let result = find_tool.execute(params).await?;

    // Print detailed error information
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
        println!("Result output: {:?}", result.output);
    }

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Print the files that were found for debugging
    println!("Found {} files:", files.len());
    for (i, file) in files.iter().enumerate() {
        println!(
            "File {}: name={}, is_dir={}",
            i,
            file["name"].as_str().unwrap(),
            file["is_dir"].as_bool().unwrap()
        );
    }

    // Verify that at least one of the entries is a directory named "subdir"
    let has_subdir = files
        .iter()
        .any(|f| f["is_dir"].as_bool().unwrap() && f["name"].as_str().unwrap() == "subdir");

    assert!(has_subdir, "Should have found a directory named 'subdir'");

    Ok(())
}

#[tokio::test]
async fn test_find_tool_exact_filename_search() -> Result<()> {
    // Create a temporary directory for our test files
    let temp_dir = tempdir()?;
    let test_dir = PathBuf::from(temp_dir.path());

    // Create a nested directory structure
    let subdir1 = test_dir.join("project");
    fs::create_dir(&subdir1)?;

    let subdir2 = subdir1.join("src");
    fs::create_dir(&subdir2)?;

    // Create files in different directories
    create_test_file(&test_dir, "readme.md", "Root readme")?;
    create_test_file(&subdir1, "config.json", "Project config")?;
    create_test_file(&subdir2, "main.go", "package main\n\nfunc main() {}")?;

    // Create the find tool with custom config that allows the temp directory
    let temp_path_str = temp_dir.path().to_string_lossy().to_string();

    // Create a config that allows the temp directory and has NO denied paths
    let config = FindConfig {
        allowed_paths: Some(vec![temp_path_str]),
        denied_paths: None, // Override the default denied paths
        max_files: 1000,
        default_max_depth: 10,
    };
    let find_tool = FindTool::with_config(config);

    // Test search with exact filename - this should find the file in the nested directory
    let params = json!({
        "pattern": "main.go",
        "base_dir": temp_dir.path().to_string_lossy().to_string()
    });

    let result = find_tool.execute(params).await?;

    // Print detailed error information
    if result.status != ToolStatus::Success {
        println!("Test failed with error: {:?}", result.error);
        println!("Result output: {:?}", result.output);
    }

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Get the files from the output
    let files = result.output["files"].as_array().unwrap();

    // Should have found 1 file, the main.go file deep in the directory structure
    assert_eq!(files.len(), 1, "Should find exactly one file matching 'main.go'");

    // The file found should be the main.go file
    let found_path = files[0]["path"].as_str().unwrap();
    assert!(found_path.contains("main.go"), "The found file should be main.go");
    assert!(found_path.contains("src"), "The file should be in the src directory");

    Ok(())
}
