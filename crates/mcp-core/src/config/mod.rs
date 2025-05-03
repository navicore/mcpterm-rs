use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub aws: AwsConfig,
    pub model_settings: ModelSettings,
    pub ui: UiConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub mcp: McpConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    pub emacs_mode: bool,
    pub command_timeout: Option<u64>, // Timeout in seconds for command execution
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoggingConfig {
    pub api_debug: bool,
    pub log_dir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpConfig {
    pub enabled: bool,
    pub base_dir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AwsConfig {
    pub region: String,
    pub profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelConfig {
    pub model_id: String,
    pub max_tokens: usize,
    pub temperature: f32,
    pub active: bool,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelSettings {
    pub models: Vec<ModelConfig>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_dir: None, // If None, will use home directory
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            aws: AwsConfig {
                region: "us-east-1".to_string(),
                profile: None,
            },
            model_settings: ModelSettings {
                models: vec![
                    ModelConfig {
                        model_id: "us.anthropic.claude-3-7-sonnet-20250219-v1:0".to_string(),
                        max_tokens: 4096,
                        temperature: 0.7,
                        active: true,
                        description: Some("Claude 3.7 Sonnet - Good Coder".to_string()),
                    },
                    ModelConfig {
                        model_id: "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
                        max_tokens: 4096,
                        temperature: 0.7,
                        active: false,
                        description: Some(
                            "Claude 3 Sonnet - Balanced performance and quality".to_string(),
                        ),
                    },
                    ModelConfig {
                        model_id: "anthropic.claude-3-opus-20240229-v1:0".to_string(),
                        max_tokens: 4096,
                        temperature: 0.7,
                        active: false,
                        description: Some(
                            "Claude 3 Opus - Highest capability and quality".to_string(),
                        ),
                    },
                ],
            },
            ui: UiConfig {
                emacs_mode: false,          // Default to Vi mode
                command_timeout: Some(180), // Increased timeout to 180 seconds
            },
            logging: LoggingConfig {
                api_debug: false,
                log_dir: None, // Default to None, will use system temp directory
            },
            mcp: McpConfig::default(),
        }
    }
}

impl Config {
    pub fn load(config_path: Option<&PathBuf>, model_id: Option<&str>, region: Option<&str>) -> std::io::Result<Self> {
        let config_path = if let Some(path) = config_path {
            path.clone()
        } else {
            let mut default_path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
            default_path.push("mcpterm");
            default_path.push("config.json");
            default_path
        };

        let config = if config_path.exists() {
            // Load existing config
            let config_str = fs::read_to_string(&config_path)?;
            let mut config: Config = serde_json::from_str(&config_str)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            // Override with args if provided
            if let Some(region_str) = region {
                config.aws.region = region_str.to_string();
            }

            if let Some(model_id_str) = model_id {
                // Find the existing model or add it, set it to active
                let mut found = false;
                for model in &mut config.model_settings.models {
                    if model.model_id == model_id_str {
                        model.active = true;
                        found = true;
                    } else {
                        model.active = false;
                    }
                }

                if !found {
                    // Add the new model
                    config.model_settings.models.push(ModelConfig {
                        model_id: model_id_str.to_string(),
                        max_tokens: 4096,
                        temperature: 0.7,
                        active: true,
                        description: None,
                    });
                }
            }

            config
        } else {
            // Create a default config
            let mut config = Config::default();

            // Override with args if provided
            if let Some(region_str) = region {
                config.aws.region = region_str.to_string();
            }

            if let Some(model_id_str) = model_id {
                // Find the existing model or add it, set it to active
                let mut found = false;
                for model in &mut config.model_settings.models {
                    if model.model_id == model_id_str {
                        model.active = true;
                        found = true;
                    } else {
                        model.active = false;
                    }
                }

                if !found {
                    // Add the new model
                    config.model_settings.models.push(ModelConfig {
                        model_id: model_id_str.to_string(),
                        max_tokens: 4096,
                        temperature: 0.7,
                        active: true,
                        description: None,
                    });
                }
            }

            // Create config directory if it doesn't exist
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Write the default config to the file
            let config_str = serde_json::to_string_pretty(&config)?;
            fs::write(&config_path, config_str)?;

            config
        };

        Ok(config)
    }

    /// Get the active model configuration
    pub fn get_active_model(&self) -> Option<ModelConfig> {
        self.model_settings
            .models
            .iter()
            .find(|model| model.active)
            .cloned()
    }
}