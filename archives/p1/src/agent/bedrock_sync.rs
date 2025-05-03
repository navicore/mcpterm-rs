use serde_json::{json, Value};
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use super::Agent;
use crate::config::{Config, ModelConfig};

#[derive(Clone)]
pub struct BedrockAgentSync {
    model_id: String,
    model_config: ModelConfig,
    log_path: PathBuf,
    api_debug: bool,
    aws_region: String,
}

impl BedrockAgentSync {
    // Add a logging helper method
    fn log(&self, message: &str) {
        if self.api_debug {
            let log_file = &self.log_path;

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)
                .unwrap_or_else(|_| File::create(log_file).expect("Failed to create log file"));

            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            if let Err(e) = writeln!(file, "[{}] {}", timestamp, message) {
                // We have a chicken-and-egg problem with logging failures
                // Using a direct file write as fallback
                let _ = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(std::env::temp_dir().join("mcpterm-fallback.log"))
                    .and_then(|mut file| writeln!(file, "Failed to write to log file: {}", e));
            }
        }
    }

    pub fn new() -> Self {
        // Create with default values initially
        let default_model = ModelConfig {
            model_id: "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            active: true,
            description: None,
        };

        let log_path = env::temp_dir().join("mcpterm-bedrock.log");

        Self {
            model_id: default_model.model_id.clone(),
            model_config: default_model,
            log_path,
            api_debug: false,
            aws_region: "us-east-1".to_string(),
        }
    }

    pub fn from_config(config: &Config) -> Self {
        // Get the active model from config, or use default if none is active
        let model_config = config.get_active_model().unwrap_or_else(|| ModelConfig {
            model_id: "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            active: true,
            description: None,
        });

        // Determine log path
        let log_path = if let Some(log_dir) = &config.logging.log_dir {
            PathBuf::from(log_dir).join("mcpterm-bedrock.log")
        } else {
            env::temp_dir().join("mcpterm-bedrock.log")
        };

        let agent = Self {
            model_id: model_config.model_id.clone(),
            model_config,
            log_path: log_path.clone(),
            api_debug: config.logging.api_debug,
            aws_region: config.aws.region.clone(),
        };

        // Log initialization if debug is enabled
        if agent.api_debug {
            agent.log(&format!(
                "Initializing Bedrock agent with model ID: {}",
                agent.model_id
            ));

            if let Some(desc) = &agent.model_config.description {
                agent.log(&format!("Model description: {}", desc));
            }

            agent.log(&format!("Max tokens: {}", agent.model_config.max_tokens));
            agent.log(&format!("Temperature: {}", agent.model_config.temperature));
            agent.log(&format!("AWS region: {}", agent.aws_region));
            agent.log(&format!("Log file location: {}", log_path.display()));
        }

        agent
    }
}

impl Agent for BedrockAgentSync {
    fn process_message(&self, input: &str) -> String {
        // Use AWS CLI as a synchronous way to call Bedrock
        self.invoke_model_sync(input)
            .unwrap_or_else(|e| format!("Error invoking Bedrock: {}", e))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

impl BedrockAgentSync {
    fn invoke_model_sync(&self, input: &str) -> Result<String, String> {
        // Create a request body based on the model type
        let request_body = if self.model_id.contains("claude") {
            // Determine Claude model version based on the model ID
            if self.model_id.contains("claude-3-") || self.model_id.contains("claude-3.") {
                // Claude 3 format (newer models)
                json!({
                    "anthropic_version": "bedrock-2023-05-31", // This is the version used in AWS examples
                    "max_tokens": self.model_config.max_tokens,
                    "temperature": self.model_config.temperature,
                    "system": r#"You are MCP, a helpful terminal-based coding assistant with the ability to DIRECTLY interact with the system. You have access to special Model Context Protocol (MCP) commands that you should ACTIVELY USE to perform actions.

IMPORTANT EXECUTION INSTRUCTIONS:
1. EXECUTE ONE COMMAND AT A TIME - Do not send multiple commands at once
2. After executing one command, WAIT FOR THE RESULT before running the next command
3. You must use the proper 'mcp' prefix for every command you run
4. The system will extract your command from code blocks and normal text, but you should be consistent

Available MCP commands:
- mcp help - Display information about MCP features
- mcp tools - List all available tools
- mcp shell <command> - Execute a shell command (e.g., mcp shell ls -la)
- mcp search <pattern> - Search for content in files
- mcp write <file_path> <content> - Create or update a file with the specified content

CORRECT INTERACTION PATTERN:
1. USER: "Create a Go project with a main.go file"
2. YOU: "I will help you create a Go project. First, lets check the environment."
3. YOU: `mcp shell go version`
4. SYSTEM: [runs command and shows output]
5. YOU: "Now I will create the main.go file."
6. YOU: `mcp write main.go package main\n\nimport "fmt"\n\nfunc main() {\n\tfmt.Println("Hello")\n}`
7. SYSTEM: [creates file and shows result]

EXAMPLES OF PROPER USAGE (ONE COMMAND AT A TIME):
✓ mcp shell ls -la
✓ mcp shell mkdir -p cmd/add
✓ mcp write main.go package main\\n\\nimport \"fmt\"\\n\\nfunc main() {\\n\\tfmt.Println(\"Hello\")\\n}
✓ mcp search TODO

INCORRECT USAGE (DO NOT DO THIS):
✗ You can run `mkdir cmd` to create a directory
✗ Here's some code for main.go: ```go...```
✗ First, we need to check if any files exist in the current directory
✗ [Running multiple commands at once without waiting for results]

When helping with coding tasks, DIRECTLY USE these commands:
1. 'mcp shell' to create directories, check environment, install packages, etc.
2. 'mcp write' to create ALL new files - never just suggest code
3. 'mcp search' to find content in existing files

For creating new projects, you should:
1. First check the environment with 'mcp shell'
2. Create directories with 'mcp shell mkdir'
3. Create each file using 'mcp write'
4. Run build/test commands with 'mcp shell'

For Golang Cobra CLI applications specifically:
1. EXECUTE 'mcp shell go mod init <module-name>'
2. EXECUTE 'mcp shell go get github.com/spf13/cobra'
3. CREATE the directory structure with 'mcp shell mkdir -p cmd'
4. WRITE each file with 'mcp write <filepath> <code>'
5. TEST with 'mcp shell go build' or 'mcp shell go run main.go'

BE PROACTIVE - you have real system access. You should TAKE DIRECT ACTION rather than explaining what to do. Use the proper MCP commands to accomplish tasks immediately."#,
                    "messages": [
                        {
                            "role": "user",
                            "content": input
                        }
                    ]
                })
            } else {
                // Older Claude models (claude-v2, claude-instant)
                json!({
                    "prompt": format!("\n\nHuman: {}\n\nAssistant:", input),
                    "max_tokens_to_sample": self.model_config.max_tokens,
                    "temperature": self.model_config.temperature,
                    "stop_sequences": ["\n\nHuman:"]
                })
            }
        } else if self.model_id.contains("titan") {
            // Amazon Titan format
            json!({
                "inputText": input,
                "textGenerationConfig": {
                    "maxTokenCount": self.model_config.max_tokens,
                    "temperature": self.model_config.temperature,
                    "topP": 0.9
                }
            })
        } else if self.model_id.contains("nova") {
            // Amazon Nova format
            json!({
                "prompt": input,
                "max_tokens": self.model_config.max_tokens,
                "temperature": self.model_config.temperature
            })
        } else if self.model_id.contains("mistral") {
            // Mistral model format
            json!({
                "prompt": input,
                "max_tokens": self.model_config.max_tokens,
                "temperature": self.model_config.temperature,
                "top_p": 0.9
            })
        } else {
            // Generic format for other models
            json!({
                "prompt": input,
                "max_tokens": self.model_config.max_tokens,
                "temperature": self.model_config.temperature
            })
        };

        // Create a temporary file for the request body
        let temp_dir = std::env::temp_dir();
        let request_id = Uuid::new_v4().to_string();
        let request_file = temp_dir.join(format!("mcpterm-request-{}.json", request_id));
        let response_file = temp_dir.join(format!("mcpterm-response-{}.json", request_id));

        // Write the request body to the temporary file
        std::fs::write(&request_file, request_body.to_string())
            .map_err(|e| format!("Failed to write request body: {}", e))?;

        // Use the region from config, falling back to environment vars if needed
        let region = if !self.aws_region.is_empty() {
            self.aws_region.clone()
        } else {
            env::var("AWS_REGION")
                .or_else(|_| env::var("AWS_DEFAULT_REGION"))
                .unwrap_or_else(|_| "us-east-1".to_string()) // Default to us-east-1
        };

        // Always use the standard invoke-model command
        // with outfile as a positional argument at the end
        let mut cmd = Command::new("aws");
        cmd.args([
            "bedrock-runtime",
            "invoke-model",
            "--region",
            &region, // Use the detected region
            "--model-id",
            &self.model_id,
            "--body",
            &format!("fileb://{}", request_file.display()),
            "--cli-binary-format",
            "raw-in-base64-out",
            "--output",
            "json",
            "--no-cli-pager",
        ]);

        // Add the output file as a positional argument
        cmd.arg(response_file.display().to_string());

        // Log command execution
        let all_args = cmd.get_args().fold(String::new(), |mut acc, arg| {
            acc.push_str(&format!("{} ", arg.to_string_lossy()));
            acc
        });
        self.log(&format!("Executing AWS CLI command: aws {}", all_args));
        self.log(&format!("Using model ID: {}", &self.model_id));
        self.log(&format!("Using region: {}", &region));
        self.log(&format!("Request file: {}", request_file.display()));
        self.log(&format!("Response file: {}", response_file.display()));

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute AWS CLI: {}", e))?;

        // Check if the command executed successfully
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Log error details to file
            self.log("AWS CLI error details:");
            self.log(&format!("Exit code: {}", output.status));
            self.log(&format!("STDOUT: {}", stdout));
            self.log(&format!("STDERR: {}", stderr));

            return Err(format!(
                "AWS CLI returned an error (exit code {}): {}",
                output.status.code().unwrap_or(-1),
                stderr
            ));
        }

        // Log successful output
        let stdout = String::from_utf8_lossy(&output.stdout);
        self.log("AWS CLI command completed successfully");
        self.log(&format!("STDOUT: {}", stdout));

        // Read the response body from the file
        let response_body = std::fs::read_to_string(&response_file)
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        // Clean up temporary files (skip cleanup if debug is enabled)
        if !self.api_debug {
            let _ = std::fs::remove_file(&request_file);
            let _ = std::fs::remove_file(&response_file);
        } else {
            self.log("Debug mode: keeping temporary files for inspection");
            self.log(&format!("  Request file: {}", request_file.display()));
            self.log(&format!("  Response file: {}", response_file.display()));
        }

        // Log the response body for debugging
        self.log(&format!("Response body: {}", response_body));

        let response_json: Value = serde_json::from_str(&response_body).map_err(|e| {
            format!(
                "Failed to parse response JSON: {} (raw response: {})",
                e, response_body
            )
        })?;

        // Extract the text from the response
        if self.model_id.contains("claude") {
            // Claude response format for Claude 3 models
            if let Some(content) = response_json["content"].as_array() {
                if let Some(first_content) = content.first() {
                    if let Some(text) = first_content["text"].as_str() {
                        return Ok(text.to_string());
                    }
                }
            }

            // Alternative Claude format for older models
            if let Some(completion) = response_json["completion"].as_str() {
                return Ok(completion.to_string());
            }

            // Log the actual response for debugging
            self.log(&format!("Response JSON: {:?}", response_json));
            Err("Failed to extract text from Claude response".to_string())
        } else if self.model_id.contains("titan") {
            // Titan model response format
            if let Some(result) = response_json["results"].as_array() {
                if let Some(first_result) = result.first() {
                    if let Some(text) = first_result["outputText"].as_str() {
                        return Ok(text.to_string());
                    }
                }
            }

            // Try alternative format
            if let Some(text) = response_json["outputText"].as_str() {
                return Ok(text.to_string());
            }

            self.log(&format!("Response JSON: {:?}", response_json));
            return Err("Failed to extract text from Titan response".to_string());
        } else if self.model_id.contains("nova") {
            // Nova model response format
            if let Some(text) = response_json["output"].as_str() {
                return Ok(text.to_string());
            }

            self.log(&format!("Response JSON: {:?}", response_json));
            return Err("Failed to extract text from Nova response".to_string());
        } else if self.model_id.contains("mistral") {
            // Mistral model response format
            if let Some(text) = response_json["outputs"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|obj| obj["text"].as_str())
            {
                return Ok(text.to_string());
            }

            if let Some(text) = response_json["completion"].as_str() {
                return Ok(text.to_string());
            }

            self.log(&format!("Response JSON: {:?}", response_json));
            return Err("Failed to extract text from Mistral response".to_string());
        } else {
            // Generic format for other models
            let text = response_json["completion"]
                .as_str()
                .or_else(|| response_json["generated_text"].as_str())
                .or_else(|| response_json["output"].as_str())
                .ok_or_else(|| {
                    self.log(&format!("Response JSON: {:?}", response_json));
                    "Failed to extract text from response"
                })?;

            Ok(text.to_string())
        }
    }
}

impl Default for BedrockAgentSync {
    fn default() -> Self {
        Self::new()
    }
}
