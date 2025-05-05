use crate::Tool;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

/// Supported test frameworks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TestFramework {
    /// Rust's built-in test framework
    Rust,
    /// JavaScript/TypeScript Jest framework
    Jest,
    /// JavaScript/TypeScript Mocha framework
    Mocha,
    /// Python's pytest framework
    Pytest,
    /// Python's unittest framework
    Unittest,
    /// Generic command-based test runner
    Custom(String),
}

/// Test result status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TestStatus {
    /// All tests passed
    Passed,
    /// Some tests failed
    Failed,
    /// Tests were skipped
    Skipped,
    /// Tests timed out
    TimedOut,
    /// Error occurred during test execution
    Error,
}

/// Individual test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Name of the test
    pub name: String,
    /// Test status
    pub status: TestStatus,
    /// Test execution time in milliseconds
    pub duration_ms: Option<u64>,
    /// Test output or error message
    pub message: Option<String>,
}

/// Aggregated test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResults {
    /// Overall test status
    pub status: TestStatus,
    /// Test framework used
    pub framework: TestFramework,
    /// Total number of tests
    pub total: usize,
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests
    pub failed: usize,
    /// Number of skipped tests
    pub skipped: usize,
    /// Total execution time in milliseconds
    pub duration_ms: u64,
    /// Individual test results
    pub tests: Vec<TestResult>,
    /// Raw output from the test command
    pub raw_output: String,
}

/// Test runner parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunnerParams {
    /// Directory or file to run tests on
    pub path: String,
    /// Specific test or pattern to run (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_filter: Option<String>,
    /// Specific test framework to use (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<TestFramework>,
    /// Maximum execution time in seconds (defaults to 300 if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Test Runner tool implementation
pub struct TestRunnerTool;

impl Default for TestRunnerTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRunnerTool {
    /// Create a new Test Runner tool
    pub fn new() -> Self {
        Self
    }

    /// Detect the appropriate test framework for a given directory
    fn detect_framework(&self, dir_path: &Path) -> Result<TestFramework> {
        // Check for Rust project
        if Path::new(&dir_path.join("Cargo.toml")).exists() {
            return Ok(TestFramework::Rust);
        }

        // Check for Node.js project
        let package_json_path = dir_path.join("package.json");
        if package_json_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&package_json_path) {
                if content.contains("\"jest\"") {
                    return Ok(TestFramework::Jest);
                } else if content.contains("\"mocha\"") {
                    return Ok(TestFramework::Mocha);
                }
            }
            
            // Default to Jest if we can't determine the framework
            return Ok(TestFramework::Jest);
        }

        // Check for Python project
        if Path::new(&dir_path.join("pytest.ini")).exists() 
            || Path::new(&dir_path.join("conftest.py")).exists() {
            return Ok(TestFramework::Pytest);
        }
        
        // Look for Python files that might use unittest
        let mut has_python_files = false;
        if let Ok(entries) = std::fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "py" {
                        has_python_files = true;
                        if let Ok(content) = std::fs::read_to_string(entry.path()) {
                            if content.contains("import unittest") || content.contains("from unittest import") {
                                return Ok(TestFramework::Unittest);
                            }
                        }
                    }
                }
            }
        }
        
        if has_python_files {
            return Ok(TestFramework::Pytest);
        }

        Err(anyhow!("Could not detect a supported test framework. Please specify the framework explicitly."))
    }

    /// Build the test command for a given framework
    fn build_test_command(
        &self,
        framework: &TestFramework,
        path: &Path,
        test_filter: Option<&str>,
    ) -> Result<(Command, Regex)> {
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        
        let command_str = match framework {
            TestFramework::Rust => {
                let mut args = String::from("cargo test");
                if let Some(filter) = test_filter {
                    args.push_str(&format!(" {}", filter));
                }
                if path.is_dir() {
                    // If path is a directory, cd to it and run tests
                    format!("cd {} && {}", path.display(), args)
                } else {
                    // If path is a file, run tests in the current directory with the file path
                    let mut dir = path.parent().unwrap_or(Path::new("."));
                    if dir.as_os_str().is_empty() {
                        dir = Path::new(".");
                    }
                    format!("cd {} && {}", dir.display(), args)
                }
            },
            TestFramework::Jest => {
                let mut args = String::from("npx jest");
                if let Some(filter) = test_filter {
                    args.push_str(&format!(" -t \"{}\"", filter));
                }
                format!("cd {} && {}", path.display(), args)
            },
            TestFramework::Mocha => {
                let mut args = String::from("npx mocha");
                if let Some(filter) = test_filter {
                    args.push_str(&format!(" -g \"{}\"", filter));
                }
                format!("cd {} && {}", path.display(), args)
            },
            TestFramework::Pytest => {
                let mut args = String::from("python -m pytest");
                if let Some(filter) = test_filter {
                    args.push_str(&format!(" -k \"{}\"", filter));
                }
                format!("cd {} && {}", path.display(), args)
            },
            TestFramework::Unittest => {
                let mut args = String::from("python -m unittest");
                if let Some(filter) = test_filter {
                    args.push_str(&format!(" {}", filter));
                }
                format!("cd {} && {}", path.display(), args)
            },
            TestFramework::Custom(custom_cmd) => {
                format!("cd {} && {}", path.display(), custom_cmd)
            },
        };
        
        cmd.arg(command_str);
        
        // Create regex patterns for parsing test output based on framework
        let output_pattern = match framework {
            TestFramework::Rust => Regex::new(r"test (.*?)\s+\.\.\.\s+(ok|failed|ignored)").unwrap(),
            TestFramework::Jest => Regex::new(r"(PASS|FAIL)\s+.*?([\w\-\.\/]+)").unwrap(),
            TestFramework::Mocha => Regex::new(r"✓|✖\s+(.*?)(\(\d+ms\))?").unwrap(),
            TestFramework::Pytest => Regex::new(r"(PASSED|FAILED|SKIPPED)\s+\[([\d\.]+)s\]\s+(.*?)$").unwrap(),
            TestFramework::Unittest => Regex::new(r"(test\w+).*?\.\.\.\s+(ok|FAIL)").unwrap(),
            TestFramework::Custom(_) => Regex::new(r"(pass|fail|error|ok|PASS|FAIL|ERROR)").unwrap(),
        };
        
        Ok((cmd, output_pattern))
    }

    /// Run tests using the specified framework and parse the results
    async fn run_tests(
        &self,
        framework: TestFramework,
        path: &str,
        test_filter: Option<&str>,
        timeout_secs: u64,
    ) -> Result<TestRunResults> {
        let path = PathBuf::from(path);
        
        // Ensure the path exists
        if !path.exists() {
            return Err(anyhow!("Path does not exist: {}", path.display()));
        }
        
        // Build the command for the detected framework
        let (cmd, output_pattern) = self.build_test_command(&framework, &path, test_filter)?;
        
        // Convert std::process::Command to tokio::process::Command for async execution
        let mut tokio_cmd = TokioCommand::from(cmd);
        
        // Set timeout for test execution
        let start_time = std::time::Instant::now();
        let timeout_duration = Duration::from_secs(timeout_secs);
        
        // Execute the command with timeout
        let output = match timeout(timeout_duration, tokio_cmd.output()).await {
            Ok(result) => match result {
                Ok(output) => output,
                Err(e) => return Err(anyhow!("Failed to execute test command: {}", e)),
            },
            Err(_) => {
                return Ok(TestRunResults {
                    status: TestStatus::TimedOut,
                    framework: framework.clone(),
                    total: 0,
                    passed: 0,
                    failed: 0,
                    skipped: 0,
                    duration_ms: timeout_secs * 1000,
                    tests: vec![],
                    raw_output: format!("Tests timed out after {} seconds", timeout_secs),
                });
            }
        };
        
        let end_time = std::time::Instant::now();
        let duration_ms = end_time.duration_since(start_time).as_millis() as u64;
        
        // Convert output to string
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined_output = format!("{}\n{}", stdout, stderr);
        
        // Parse test results
        let mut test_results = Vec::new();
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        
        for cap in output_pattern.captures_iter(&combined_output) {
            match framework {
                TestFramework::Rust => {
                    if cap.len() >= 3 {
                        let test_name = cap[1].to_string();
                        let status = match &cap[2] {
                            "ok" => {
                                passed += 1;
                                TestStatus::Passed
                            },
                            "failed" => {
                                failed += 1;
                                TestStatus::Failed
                            },
                            "ignored" => {
                                skipped += 1;
                                TestStatus::Skipped
                            },
                            _ => {
                                failed += 1;
                                TestStatus::Error
                            },
                        };
                        
                        test_results.push(TestResult {
                            name: test_name,
                            status,
                            duration_ms: None, // Rust test output doesn't include durations by default
                            message: None,
                        });
                    }
                },
                TestFramework::Jest | TestFramework::Mocha => {
                    if cap.len() >= 2 {
                        let (test_name, status) = if framework == TestFramework::Jest {
                            let status = match &cap[1] {
                                "PASS" => {
                                    passed += 1;
                                    TestStatus::Passed
                                },
                                "FAIL" => {
                                    failed += 1;
                                    TestStatus::Failed
                                },
                                _ => {
                                    failed += 1;
                                    TestStatus::Error
                                },
                            };
                            (cap[2].to_string(), status)
                        } else {
                            let status = if cap[0].starts_with("✓") {
                                passed += 1;
                                TestStatus::Passed
                            } else {
                                failed += 1;
                                TestStatus::Failed
                            };
                            (cap[1].to_string(), status)
                        };
                        
                        // Try to extract duration if available
                        let duration = if cap.len() >= 3 && framework == TestFramework::Mocha {
                            let duration_str = &cap[2];
                            if let Some(ms_str) = duration_str.strip_prefix("(").and_then(|s| s.strip_suffix("ms)")) {
                                ms_str.parse::<u64>().ok()
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        
                        test_results.push(TestResult {
                            name: test_name,
                            status,
                            duration_ms: duration,
                            message: None,
                        });
                    }
                },
                TestFramework::Pytest => {
                    if cap.len() >= 4 {
                        let test_name = cap[3].to_string();
                        let duration_str = &cap[2];
                        let duration_ms = if let Ok(secs) = duration_str.parse::<f64>() {
                            Some((secs * 1000.0) as u64)
                        } else {
                            None
                        };
                        
                        let status = match &cap[1] {
                            "PASSED" => {
                                passed += 1;
                                TestStatus::Passed
                            },
                            "FAILED" => {
                                failed += 1;
                                TestStatus::Failed
                            },
                            "SKIPPED" => {
                                skipped += 1;
                                TestStatus::Skipped
                            },
                            _ => {
                                failed += 1;
                                TestStatus::Error
                            },
                        };
                        
                        test_results.push(TestResult {
                            name: test_name,
                            status,
                            duration_ms,
                            message: None,
                        });
                    }
                },
                TestFramework::Unittest => {
                    if cap.len() >= 3 {
                        let test_name = cap[1].to_string();
                        let status = match &cap[2] {
                            "ok" => {
                                passed += 1;
                                TestStatus::Passed
                            },
                            "FAIL" => {
                                failed += 1;
                                TestStatus::Failed
                            },
                            _ => {
                                failed += 1;
                                TestStatus::Error
                            },
                        };
                        
                        test_results.push(TestResult {
                            name: test_name,
                            status,
                            duration_ms: None,
                            message: None,
                        });
                    }
                },
                TestFramework::Custom(_) => {
                    // For custom commands, just try to extract pass/fail information
                    let status_str = &cap[1].to_lowercase();
                    if status_str.contains("pass") || status_str.contains("ok") {
                        passed += 1;
                    } else if status_str.contains("fail") || status_str.contains("error") {
                        failed += 1;
                    }
                },
            }
        }
        
        // If we couldn't parse any tests but the command succeeded, add a placeholder result
        if test_results.is_empty() && output.status.success() {
            passed = 1;
            test_results.push(TestResult {
                name: "unknown".to_string(),
                status: TestStatus::Passed,
                duration_ms: None,
                message: Some("Test command succeeded but no individual test results could be parsed".to_string()),
            });
        }
        
        // Determine overall status
        let status = if output.status.success() && failed == 0 {
            TestStatus::Passed
        } else if failed > 0 {
            TestStatus::Failed
        } else {
            TestStatus::Error
        };
        
        let total = passed + failed + skipped;
        
        Ok(TestRunResults {
            status,
            framework,
            total,
            passed,
            failed,
            skipped,
            duration_ms,
            tests: test_results,
            raw_output: combined_output,
        })
    }
}

#[async_trait]
impl Tool for TestRunnerTool {
    fn metadata(&self) -> crate::ToolMetadata {
        crate::ToolMetadata {
            id: "test_runner".to_string(),
            name: "Test Runner".to_string(),
            description: "Runs tests and analyzes test results".to_string(),
            category: crate::ToolCategory::Utility,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory or file to run tests on"
                    },
                    "test_filter": {
                        "type": "string",
                        "description": "Specific test or pattern to run"
                    },
                    "framework": {
                        "type": "string",
                        "description": "Specific test framework to use"
                    },
                    "timeout_seconds": {
                        "type": "number",
                        "description": "Maximum execution time in seconds"
                    }
                },
                "required": ["path"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "Overall test status"
                    },
                    "total": {
                        "type": "number",
                        "description": "Total number of tests"
                    },
                    "passed": {
                        "type": "number",
                        "description": "Number of passed tests"
                    },
                    "failed": {
                        "type": "number",
                        "description": "Number of failed tests"
                    }
                }
            }),
        }
    }

    async fn execute(&self, params_json: serde_json::Value) -> Result<crate::ToolResult> {
        // Parse parameters
        let params: TestRunnerParams = serde_json::from_value(params_json)
            .map_err(|e| anyhow!("Invalid parameters: {}", e))?;
        
        // Validate path
        if params.path.is_empty() {
            return Err(anyhow!("Path must be specified"));
        }
        
        let path = Path::new(&params.path);
        
        // Determine framework (detect or use specified)
        let framework = if let Some(fw) = params.framework {
            fw
        } else {
            if path.is_dir() {
                self.detect_framework(path)?
            } else if let Some(parent) = path.parent() {
                self.detect_framework(parent)?
            } else {
                return Err(anyhow!("Could not detect framework from file path"));
            }
        };
        
        // Set timeout (default or user-specified)
        let timeout_secs = params.timeout_seconds.unwrap_or(300);
        
        // Run tests
        let test_filter = params.test_filter.as_deref();
        let results = self.run_tests(framework, &params.path, test_filter, timeout_secs).await?;
        
        // Convert results to JSON
        let json_result = serde_json::to_value(results)
            .map_err(|e| anyhow!("Failed to serialize results: {}", e))?;
        
        Ok(crate::ToolResult {
            tool_id: "test_runner".to_string(),
            status: crate::ToolStatus::Success,
            output: json_result,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_detect_rust_framework() {
        let dir = tempdir().unwrap();
        let cargo_toml_path = dir.path().join("Cargo.toml");
        
        // Create a minimal Cargo.toml
        fs::write(cargo_toml_path, "[package]\nname = \"test\"\nversion = \"0.1.0\"").unwrap();
        
        let tool = TestRunnerTool::new();
        let result = tool.detect_framework(dir.path());
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TestFramework::Rust);
    }
    
    #[test]
    fn test_detect_jest_framework() {
        let dir = tempdir().unwrap();
        let package_json_path = dir.path().join("package.json");
        
        // Create a minimal package.json with Jest
        fs::write(package_json_path, 
            "{\"name\": \"test\", \"version\": \"1.0.0\", \"devDependencies\": {\"jest\": \"^27.0.0\"}}").unwrap();
        
        let tool = TestRunnerTool::new();
        let result = tool.detect_framework(dir.path());
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TestFramework::Jest);
    }
    
    #[test]
    fn test_detect_pytest_framework() {
        let dir = tempdir().unwrap();
        let pytest_ini_path = dir.path().join("pytest.ini");
        
        // Create a minimal pytest.ini
        fs::write(pytest_ini_path, "[pytest]\naddopts = -xvs").unwrap();
        
        let tool = TestRunnerTool::new();
        let result = tool.detect_framework(dir.path());
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TestFramework::Pytest);
    }
    
    #[test]
    fn test_build_command_rust() {
        let tool = TestRunnerTool::new();
        let dir = Path::new(".");
        
        let (cmd, _) = tool.build_test_command(&TestFramework::Rust, dir, Some("test_name")).unwrap();
        
        let args: Vec<_> = cmd.get_args().collect();
        let command_str = args[1].to_str().unwrap();
        
        assert!(command_str.contains("cargo test test_name"));
    }
}