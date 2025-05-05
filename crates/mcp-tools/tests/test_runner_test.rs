use mcp_tools::testing::TestRunnerTool;
use mcp_tools::Tool;
use mcp_tools::ToolStatus;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_framework_detection() {
    let tool = TestRunnerTool::new();

    // Create temporary rust project
    let rust_dir = tempdir().unwrap();
    fs::write(
        rust_dir.path().join("Cargo.toml"),
        "[package]\nname = \"test-project\"\nversion = \"0.1.0\"",
    )
    .unwrap();

    // Create temporary node project with jest
    let jest_dir = tempdir().unwrap();
    fs::write(
        jest_dir.path().join("package.json"),
        r#"{"name": "test-project", "version": "1.0.0", "devDependencies": {"jest": "^27.0.0"}}"#,
    )
    .unwrap();

    // Create temporary node project with mocha
    let mocha_dir = tempdir().unwrap();
    fs::write(
        mocha_dir.path().join("package.json"),
        r#"{"name": "test-project", "version": "1.0.0", "devDependencies": {"mocha": "^9.0.0"}}"#,
    )
    .unwrap();

    // Create temporary python project with pytest
    let pytest_dir = tempdir().unwrap();
    fs::write(
        pytest_dir.path().join("pytest.ini"),
        "[pytest]\naddopts = -xvs",
    )
    .unwrap();

    // Create temporary python project with unittest
    let unittest_dir = tempdir().unwrap();
    fs::write(
        unittest_dir.path().join("test_example.py"),
        "import unittest\n\nclass TestExample(unittest.TestCase):\n    def test_example(self):\n        self.assertTrue(True)"
    ).unwrap();

    // Test Rust framework detection
    let params = json!({
        "path": rust_dir.path().to_str().unwrap(),
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result.status, ToolStatus::Success);

    let output_obj = result.output.as_object().unwrap();
    assert!(output_obj.contains_key("framework"));
    assert_eq!(output_obj["framework"], "Rust");

    // Test Jest framework detection
    let params = json!({
        "path": jest_dir.path().to_str().unwrap(),
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result.status, ToolStatus::Success);

    let output_obj = result.output.as_object().unwrap();
    assert!(output_obj.contains_key("framework"));
    assert_eq!(output_obj["framework"], "Jest");

    // Test Mocha framework detection
    let params = json!({
        "path": mocha_dir.path().to_str().unwrap(),
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result.status, ToolStatus::Success);

    let output_obj = result.output.as_object().unwrap();
    assert!(output_obj.contains_key("framework"));
    assert_eq!(output_obj["framework"], "Mocha");

    // Test Pytest framework detection
    let params = json!({
        "path": pytest_dir.path().to_str().unwrap(),
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result.status, ToolStatus::Success);

    let output_obj = result.output.as_object().unwrap();
    assert!(output_obj.contains_key("framework"));
    assert_eq!(output_obj["framework"], "Pytest");
}

#[tokio::test]
async fn test_explicit_framework_selection() {
    let tool = TestRunnerTool::new();
    let dir = tempdir().unwrap();

    // Test with a standard framework
    let params = json!({
        "path": dir.path().to_str().unwrap(),
        "framework": "Rust"  // Using a standard framework explicitly
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result.status, ToolStatus::Success);

    let output_obj = result.output.as_object().unwrap();
    assert!(output_obj.contains_key("framework"));
    assert_eq!(output_obj["framework"], "Rust");

    // Now test another framework
    let params = json!({
        "path": dir.path().to_str().unwrap(),
        "framework": "Jest"  // Using a different framework
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result.status, ToolStatus::Success);

    let output_obj = result.output.as_object().unwrap();
    assert!(output_obj.contains_key("framework"));
    assert_eq!(output_obj["framework"], "Jest");
}

#[tokio::test]
async fn test_timeout_parameter() {
    let tool = TestRunnerTool::new();
    let dir = tempdir().unwrap();

    // Create a simple rust project that will timeout
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"test-project\"\nversion = \"0.1.0\"",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        r#"
        #[cfg(test)]
        mod tests {
            #[test]
            fn infinite_test() {
                let mut i = 0;
                loop {
                    i += 1;
                    if i == std::u64::MAX {
                        break;
                    }
                }
            }
        }
        "#,
    )
    .unwrap();

    // Run with a very short timeout
    let params = json!({
        "path": dir.path().to_str().unwrap(),
        "timeout_seconds": 1
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result.status, ToolStatus::Success);

    let output_obj = result.output.as_object().unwrap();
    // This should timeout
    assert!(output_obj.contains_key("status"));
    assert_eq!(output_obj["status"], "TimedOut");
}

#[tokio::test]
async fn test_tool_interface() {
    let tool = TestRunnerTool::new();

    // Test tool metadata
    let metadata = tool.metadata();
    assert_eq!(metadata.id, "test_runner");
    assert!(metadata.description.contains("test"));

    // Test validation (empty path)
    let params = json!({});
    let result = tool.execute(params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_filtering() {
    let tool = TestRunnerTool::new();
    let dir = tempdir().unwrap();

    // Create a simple rust project with multiple tests
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"test-project\"\nversion = \"0.1.0\"",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        r#"
        #[cfg(test)]
        mod tests {
            #[test]
            fn test_one() {
                assert!(true);
            }

            #[test]
            fn test_two() {
                assert!(true);
            }

            #[test]
            fn another_test() {
                assert!(true);
            }
        }
        "#,
    )
    .unwrap();

    // Run with a filter
    let params = json!({
        "path": dir.path().to_str().unwrap(),
        "test_filter": "test_one"
    });

    let result = tool.execute(params).await.unwrap();
    assert_eq!(result.status, ToolStatus::Success);

    let output_obj = result.output.as_object().unwrap();
    // Should succeed
    assert!(output_obj.contains_key("status"));
    assert_eq!(output_obj["status"], "Passed");

    // Should only include filtered tests
    if let Some(tests) = output_obj.get("tests").and_then(|t| t.as_array()) {
        // Check if any test includes the word "one"
        let has_test_one = tests.iter().any(|t| {
            t.get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .contains("one")
        });

        assert!(has_test_one);
    }
}

// This test is commented out because it requires real command execution
// and depends on the environment. Uncomment to run manually if needed.
// #[tokio::test]
// async fn test_real_command_execution() {
//     // This test only runs if we're in a Rust project (like this one)
//     if !Path::new("Cargo.toml").exists() {
//         return;
//     }
//
//     let tool = TestRunnerTool::new();
//
//     // Run a real test in the current project
//     let params = json!({
//         "path": ".",
//         "test_filter": "test_framework_detection"
//     });
//
//     let result = tool.execute(params).await.unwrap();
//     let result_obj = result.as_object().unwrap();
//
//     // Should be able to run the test
//     assert!(result_obj.contains_key("status"));
//     assert_eq!(result_obj["status"], "Passed");
// }
