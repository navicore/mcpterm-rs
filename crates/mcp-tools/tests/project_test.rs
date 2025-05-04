use mcp_tools::analysis::{ProjectConfig, ProjectNavigator, ProjectType};
use mcp_tools::{Tool, ToolStatus};
use serde_json::json;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

// Helper to create a test file
fn create_test_file(base_dir: &Path, rel_path: &str, content: &str) -> std::io::Result<()> {
    let path = base_dir.join(rel_path);

    // Ensure the parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = File::create(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

// Helper to create a test Rust project
fn create_rust_project(dir: &Path) -> std::io::Result<()> {
    // Create Cargo.toml
    create_test_file(
        dir,
        "Cargo.toml",
        r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3.2"
"#,
    )?;

    // Create src/main.rs
    create_test_file(
        dir,
        "src/main.rs",
        r#"
fn main() {
    println!("Hello, world!");
}
"#,
    )?;

    // Create src/lib.rs
    create_test_file(
        dir,
        "src/lib.rs",
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#,
    )?;

    // Create tests directory
    create_test_file(
        dir,
        "tests/integration_test.rs",
        r#"
use test_project::add;

#[test]
fn test_add_integration() {
    assert_eq!(add(3, 5), 8);
}
"#,
    )?;

    // Create examples directory
    create_test_file(
        dir,
        "examples/example1.rs",
        r#"
fn main() {
    println!("This is an example");
}
"#,
    )?;

    // Create .gitignore
    create_test_file(
        dir,
        ".gitignore",
        r#"
/target
Cargo.lock
"#,
    )?;

    Ok(())
}

// Helper to create a test Node.js project
fn create_node_project(dir: &Path) -> std::io::Result<()> {
    // Create package.json
    create_test_file(
        dir,
        "package.json",
        r#"
{
  "name": "test-project",
  "version": "1.0.0",
  "description": "A test Node.js project",
  "main": "src/index.js",
  "scripts": {
    "start": "node src/index.js",
    "test": "jest"
  },
  "dependencies": {
    "express": "^4.17.1",
    "lodash": "^4.17.21"
  },
  "devDependencies": {
    "jest": "^27.0.6"
  }
}
"#,
    )?;

    // Create src/index.js
    create_test_file(
        dir,
        "src/index.js",
        r#"
const express = require('express');
const app = express();
const port = 3000;

app.get('/', (req, res) => {
  res.send('Hello World!');
});

app.listen(port, () => {
  console.log(`Example app listening at http://localhost:${port}`);
});
"#,
    )?;

    // Create test file
    create_test_file(
        dir,
        "test/app.test.js",
        r#"
test('two plus two is four', () => {
  expect(2 + 2).toBe(4);
});
"#,
    )?;

    // Create .gitignore
    create_test_file(
        dir,
        ".gitignore",
        r#"
node_modules/
package-lock.json
"#,
    )?;

    Ok(())
}

// Helper to create a test Python project
fn create_python_project(dir: &Path) -> std::io::Result<()> {
    // Create setup.py
    create_test_file(
        dir,
        "setup.py",
        r#"
from setuptools import setup, find_packages

setup(
    name="test-project",
    version="0.1.0",
    packages=find_packages(),
    install_requires=[
        "requests>=2.25.1",
        "pyyaml>=5.4.1",
    ],
)
"#,
    )?;

    // Create requirements.txt
    create_test_file(
        dir,
        "requirements.txt",
        r#"
requests==2.25.1
pyyaml==5.4.1
"#,
    )?;

    // Create dev-requirements.txt
    create_test_file(
        dir,
        "dev-requirements.txt",
        r#"
pytest==6.2.5
black==21.5b2
"#,
    )?;

    // Create main module
    create_test_file(
        dir,
        "src/test_project/__init__.py",
        r#"
def add(a, b):
    return a + b
"#,
    )?;

    // Create main app
    create_test_file(
        dir,
        "src/test_project/app.py",
        r#"
def main():
    print("Hello, World!")

if __name__ == "__main__":
    main()
"#,
    )?;

    // Create test file
    create_test_file(
        dir,
        "tests/test_add.py",
        r#"
from test_project import add

def test_add():
    assert add(2, 3) == 5
"#,
    )?;

    // Create .gitignore
    create_test_file(
        dir,
        ".gitignore",
        r#"
__pycache__/
*.py[cod]
*$py.class
venv/
.pytest_cache/
"#,
    )?;

    Ok(())
}

#[tokio::test]
async fn test_project_navigator_rust() {
    // Create temporary directory with a Rust project
    let dir = tempdir().unwrap();
    create_rust_project(dir.path()).unwrap();

    // Create a project navigator with allowed paths
    let navigator = ProjectNavigator::with_config(ProjectConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None, // Override the default denied paths
        max_depth: 10,
        max_file_size: 10 * 1024 * 1024,
        skip_dirs: vec!["target".to_string(), ".git".to_string()],
        skip_extensions: vec![".o".to_string(), ".exe".to_string()],
    });

    // Test analyzing the project
    let result = navigator
        .execute(json!({
            "project_dir": dir.path().to_string_lossy(),
            "include_hidden": true
        }))
        .await
        .unwrap();

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Check project type
    assert_eq!(result.output["project_type"].as_str().unwrap(), "Rust");

    // Check structure
    let structure = &result.output["structure"];
    assert!(structure.is_object());

    // Check entry points
    let entry_points = result.output["entry_points"].as_array().unwrap();

    // Should have found main.rs and lib.rs as entry points
    let has_main = entry_points.iter().any(|e| {
        e["path"].as_str().unwrap() == "src/main.rs" && e["entry_type"].as_str().unwrap() == "main"
    });

    let has_lib = entry_points.iter().any(|e| {
        e["path"].as_str().unwrap() == "src/lib.rs"
            && e["entry_type"].as_str().unwrap() == "library"
    });

    assert!(has_main, "Should have found main.rs as an entry point");
    assert!(has_lib, "Should have found lib.rs as an entry point");

    // Check dependencies
    let dependencies = result.output["dependencies"].as_array().unwrap();

    // Should have found serde and tokio as dependencies
    let has_serde = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "serde" && !d["is_dev"].as_bool().unwrap());

    let has_tokio = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "tokio" && !d["is_dev"].as_bool().unwrap());

    let has_tempfile = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "tempfile" && d["is_dev"].as_bool().unwrap());

    assert!(has_serde, "Should have found serde dependency");
    assert!(has_tokio, "Should have found tokio dependency");
    assert!(has_tempfile, "Should have found tempfile dev dependency");
}

#[tokio::test]
async fn test_project_navigator_node() {
    // Create temporary directory with a Node.js project
    let dir = tempdir().unwrap();
    create_node_project(dir.path()).unwrap();

    // Create a project navigator with allowed paths
    let navigator = ProjectNavigator::with_config(ProjectConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        max_depth: 5,
        max_file_size: 10 * 1024 * 1024,
        skip_dirs: vec!["node_modules".to_string(), ".git".to_string()],
        skip_extensions: vec![".log".to_string()],
    });

    // Test analyzing the project
    let result = navigator
        .execute(json!({
            "project_dir": dir.path().to_string_lossy(),
            "include_hidden": true,
            "analyze_dependencies": true
        }))
        .await
        .unwrap();

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Check project type
    assert_eq!(result.output["project_type"].as_str().unwrap(), "Node.js");

    // Check entry points
    let entry_points = result.output["entry_points"].as_array().unwrap();

    // Should have found index.js as the main entry point
    let has_main = entry_points.iter().any(|e| {
        e["path"].as_str().unwrap() == "src/index.js" && e["entry_type"].as_str().unwrap() == "main"
    });

    assert!(
        has_main,
        "Should have found src/index.js as the main entry point"
    );

    // Check dependencies
    let dependencies = result.output["dependencies"].as_array().unwrap();

    // Should have found express and lodash as dependencies, jest as devDependency
    let has_express = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "express" && !d["is_dev"].as_bool().unwrap());

    let has_lodash = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "lodash" && !d["is_dev"].as_bool().unwrap());

    let has_jest = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "jest" && d["is_dev"].as_bool().unwrap());

    assert!(has_express, "Should have found express dependency");
    assert!(has_lodash, "Should have found lodash dependency");
    assert!(has_jest, "Should have found jest dev dependency");
}

#[tokio::test]
async fn test_project_navigator_python() {
    // Create temporary directory with a Python project
    let dir = tempdir().unwrap();
    create_python_project(dir.path()).unwrap();

    // Create a project navigator with allowed paths
    let navigator = ProjectNavigator::with_config(ProjectConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        max_depth: 5,
        max_file_size: 10 * 1024 * 1024,
        skip_dirs: vec![
            "__pycache__".to_string(),
            ".git".to_string(),
            "venv".to_string(),
        ],
        skip_extensions: vec![".pyc".to_string()],
    });

    // Test analyzing the project
    let result = navigator
        .execute(json!({
            "project_dir": dir.path().to_string_lossy(),
            "include_hidden": true
        }))
        .await
        .unwrap();

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Check project type
    assert_eq!(result.output["project_type"].as_str().unwrap(), "Python");

    // Check dependencies
    let dependencies = result.output["dependencies"].as_array().unwrap();

    // Should have found requests and pyyaml as dependencies
    let has_requests = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "requests" && !d["is_dev"].as_bool().unwrap());

    let has_pyyaml = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "pyyaml" && !d["is_dev"].as_bool().unwrap());

    // And pytest and black as dev dependencies
    let has_pytest = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "pytest" && d["is_dev"].as_bool().unwrap());

    let has_black = dependencies
        .iter()
        .any(|d| d["name"].as_str().unwrap() == "black" && d["is_dev"].as_bool().unwrap());

    assert!(has_requests, "Should have found requests dependency");
    assert!(has_pyyaml, "Should have found pyyaml dependency");
    assert!(has_pytest, "Should have found pytest dev dependency");
    assert!(has_black, "Should have found black dev dependency");
}

#[tokio::test]
async fn test_project_navigator_depth_limit() {
    // Create temporary directory
    let dir = tempdir().unwrap();

    // Create a deep directory structure
    create_test_file(
        dir.path(),
        "level1/level2/level3/level4/level5/file.txt",
        "test",
    )
    .unwrap();

    // Create a project navigator with a depth limit of 3
    let navigator = ProjectNavigator::with_config(ProjectConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        max_depth: 3, // Only scan 3 levels deep
        max_file_size: 10 * 1024 * 1024,
        skip_dirs: vec![],
        skip_extensions: vec![],
    });

    // Test analyzing the project
    let result = navigator
        .execute(json!({
            "project_dir": dir.path().to_string_lossy()
        }))
        .await
        .unwrap();

    // Verify the result
    assert_eq!(result.status, ToolStatus::Success);

    // Check the structure - it should contain level1/level2/level3 but not deeper
    let structure = &result.output["structure"];

    // Find the level1 directory in children
    let root_children = structure["children"].as_array().unwrap();
    let level1 = root_children
        .iter()
        .find(|c| c["path"].as_str().unwrap().contains("level1"))
        .expect("Should have found level1 directory");

    // Find level2 in level1's children
    let level1_children = level1["children"].as_array().unwrap();
    let level2 = level1_children
        .iter()
        .find(|c| c["path"].as_str().unwrap().contains("level2"))
        .expect("Should have found level2 directory");

    // Find level3 in level2's children
    let level2_children = level2["children"].as_array().unwrap();
    let level3 = level2_children
        .iter()
        .find(|c| c["path"].as_str().unwrap().contains("level3"))
        .expect("Should have found level3 directory");

    // Level3 should either have no children array or an empty children array
    // since we reached the max depth
    if let Some(level3_children) = level3.get("children") {
        assert!(
            level3_children.as_array().unwrap().is_empty(),
            "Level3 should have no children due to max_depth"
        );
    }
}

#[tokio::test]
async fn test_project_navigator_denied_path() {
    // Create a project navigator with default security
    let navigator = ProjectNavigator::new();

    // Try to analyze a sensitive path
    let result = navigator
        .execute(json!({
            "project_dir": "/etc"
        }))
        .await
        .unwrap();

    // Print the actual error message
    println!("Status: {:?}", result.status);
    println!("Error message: {:?}", result.output);

    // Verify access is denied
    assert_eq!(result.status, ToolStatus::Failure);

    // Check for any error condition - it's a test for denied path so we just need to verify it's denied
    // This is more flexible to handle different error messages
    assert!(result.error.is_some() || result.output.get("error").is_some());
}

#[tokio::test]
async fn test_project_navigator_nonexistent_dir() {
    // Create a temporary directory for an allowed path
    let dir = tempdir().unwrap();

    // Create a project navigator
    let navigator = ProjectNavigator::with_config(ProjectConfig {
        allowed_paths: Some(vec![dir.path().to_string_lossy().into_owned()]),
        denied_paths: None,
        max_depth: 5,
        max_file_size: 10 * 1024 * 1024,
        skip_dirs: vec![],
        skip_extensions: vec![],
    });

    // Try to analyze a non-existent directory
    let nonexistent_dir = dir.path().join("nonexistent");

    let result = navigator
        .execute(json!({
            "project_dir": nonexistent_dir.to_string_lossy()
        }))
        .await
        .unwrap();

    // Verify the operation fails
    assert_eq!(result.status, ToolStatus::Failure);
    assert!(result.output["error"]
        .as_str()
        .unwrap()
        .contains("does not exist"));
}
