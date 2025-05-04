use super::languages::{
    common::{AnalysisDetail, AnalysisResults, AnalysisType, LanguageAnalyzer},
    JsAnalyzer, PythonAnalyzer, RustAnalyzer,
};
use crate::{Tool, ToolCategory, ToolMetadata, ToolResult, ToolStatus};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

/// Tool for analyzing code in various programming languages
pub struct LanguageAnalyzerTool {
    analyzers: Vec<Arc<dyn LanguageAnalyzer + Send + Sync>>,
}

impl Default for LanguageAnalyzerTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzerTool {
    pub fn new() -> Self {
        let analyzers: Vec<Arc<dyn LanguageAnalyzer + Send + Sync>> = vec![
            // Register available language analyzers
            Arc::new(RustAnalyzer::new()),
            Arc::new(JsAnalyzer::new()),
            Arc::new(PythonAnalyzer::new()),
        ];

        Self { analyzers }
    }

    /// Get the appropriate analyzer for a file
    fn get_analyzer_for_file(
        &self,
        file_path: &Path,
    ) -> Option<Arc<dyn LanguageAnalyzer + Send + Sync>> {
        for analyzer in &self.analyzers {
            if analyzer.is_compatible(file_path) {
                return Some(Arc::clone(analyzer));
            }
        }
        None
    }

    /// Get analyzer by language name
    fn get_analyzer_by_language(
        &self,
        language: &str,
    ) -> Option<Arc<dyn LanguageAnalyzer + Send + Sync>> {
        let language_lower = language.to_lowercase();
        for analyzer in &self.analyzers {
            let analyzer_lang = analyzer.language_name().to_lowercase();
            if analyzer_lang.contains(&language_lower) || language_lower.contains(&analyzer_lang) {
                return Some(Arc::clone(analyzer));
            }
        }
        None
    }

    /// Analyze code using the appropriate analyzer
    fn analyze(
        &self,
        file_path: Option<&str>,
        code: Option<&str>,
        language: Option<&str>,
        analysis_type_str: Option<&str>,
        detail_level_str: Option<&str>,
    ) -> Result<AnalysisResults> {
        // Determine the analysis type
        let analysis_type = match analysis_type_str {
            Some(t) => AnalysisType::from(t),
            None => AnalysisType::default(),
        };

        // Determine the detail level
        let detail_level = match detail_level_str {
            Some(d) => AnalysisDetail::from(d),
            None => AnalysisDetail::default(),
        };

        // If we have a file path, get an analyzer for that file type
        if let Some(path_str) = file_path {
            let path = Path::new(path_str);

            // First try to get an analyzer based on file extension
            if let Some(analyzer) = self.get_analyzer_for_file(path) {
                return analyzer.analyze_file(path, analysis_type, detail_level);
            }

            // If that fails and we have a language specified, try by language
            if let Some(lang) = language {
                if let Some(analyzer) = self.get_analyzer_by_language(lang) {
                    // For an incompatible file extension but specified language,
                    // we'll read the file and analyze the content directly
                    if path.exists() {
                        let content = std::fs::read_to_string(path)?;
                        let mut results =
                            analyzer.analyze_code(&content, analysis_type, detail_level)?;
                        results.file_path = Some(path_str.to_string());
                        return Ok(results);
                    }
                }
            }

            return Err(anyhow!(
                "No compatible analyzer found for file: {}",
                path_str
            ));
        }

        // If we have code but no file, we need a language specification
        if let Some(code_str) = code {
            if let Some(lang) = language {
                if let Some(analyzer) = self.get_analyzer_by_language(lang) {
                    return analyzer.analyze_code(code_str, analysis_type, detail_level);
                }
                return Err(anyhow!("Unsupported language: {}", lang));
            }
            return Err(anyhow!(
                "Language must be specified when analyzing code content"
            ));
        }

        Err(anyhow!("Either file_path or code content must be provided"))
    }
}

#[async_trait]
impl Tool for LanguageAnalyzerTool {
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            id: "code_analyzer".to_string(),
            name: "Code Analyzer".to_string(),
            description:
                "Analyze code structure and relationships in various programming languages"
                    .to_string(),
            category: ToolCategory::Search,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "Path to the file to analyze"
                    },
                    "code": {
                        "type": "string",
                        "description": "Code content to analyze (alternative to file)"
                    },
                    "language": {
                        "type": "string",
                        "description": "Programming language of the code (required when using code content)"
                    },
                    "analysis_type": {
                        "type": "string",
                        "description": "Type of analysis to perform (definitions, imports, usages, datastructures, comprehensive)",
                        "enum": ["definitions", "imports", "usages", "datastructures", "comprehensive"],
                        "default": "comprehensive"
                    },
                    "detail_level": {
                        "type": "string",
                        "description": "Level of detail to include in results (low, medium, high)",
                        "enum": ["low", "medium", "high"],
                        "default": "medium"
                    }
                },
                "anyOf": [
                    {"required": ["file"]},
                    {"required": ["code", "language"]}
                ]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string"
                    },
                    "file_path": {
                        "type": ["string", "null"]
                    },
                    "definitions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "def_type": {"type": "string"},
                                "name": {"type": "string"},
                                "line": {"type": "integer"},
                                "column": {"type": ["integer", "null"]},
                                "args": {"type": ["array", "null"]},
                                "return_type": {"type": ["string", "null"]},
                                "visibility": {"type": ["string", "null"]},
                                "doc_comment": {"type": ["string", "null"]},
                                "full_text": {"type": ["string", "null"]}
                            }
                        }
                    },
                    "imports": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "module": {"type": "string"},
                                "line": {"type": "integer"},
                                "column": {"type": ["integer", "null"]},
                                "alias": {"type": ["string", "null"]},
                                "items": {"type": ["array", "null"]},
                                "full_text": {"type": "string"}
                            }
                        }
                    },
                    "usages": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string"},
                                "line": {"type": "integer"},
                                "column": {"type": "integer"},
                                "context": {"type": "string"},
                                "usage_type": {"type": ["string", "null"]}
                            }
                        }
                    },
                    "messages": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                }
            }),
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Extract parameters
        let file_path = params["file"].as_str();
        let code = params["code"].as_str();
        let language = params["language"].as_str();
        let analysis_type = params["analysis_type"].as_str();
        let detail_level = params["detail_level"].as_str();

        // Perform the analysis
        match self.analyze(file_path, code, language, analysis_type, detail_level) {
            Ok(results) => Ok(ToolResult {
                tool_id: "code_analyzer".to_string(),
                status: ToolStatus::Success,
                output: serde_json::to_value(results)?,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                tool_id: "code_analyzer".to_string(),
                status: ToolStatus::Failure,
                output: json!({
                    "error": e.to_string()
                }),
                error: Some(e.to_string()),
            }),
        }
    }
}
