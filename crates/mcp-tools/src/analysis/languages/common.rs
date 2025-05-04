use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Types of analysis that can be performed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisType {
    /// Analyze function, class, and method definitions
    Definitions,
    /// Analyze import statements
    Imports,
    /// Analyze symbol usages
    Usages,
    /// Analyze data structures (structs, types, etc.)
    DataStructures,
    /// Comprehensive analysis including all the above
    Comprehensive,
}

impl Default for AnalysisType {
    fn default() -> Self {
        AnalysisType::Comprehensive
    }
}

impl From<&str> for AnalysisType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "definitions" => AnalysisType::Definitions,
            "imports" => AnalysisType::Imports,
            "usages" => AnalysisType::Usages,
            "datastructures" | "data_structures" => AnalysisType::DataStructures,
            "comprehensive" | "all" => AnalysisType::Comprehensive,
            _ => AnalysisType::Comprehensive, // Default
        }
    }
}

/// Level of detail to include in analysis results
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AnalysisDetail {
    /// Basic information only
    Low,
    /// Standard level of detail
    Medium,
    /// Comprehensive detail
    High,
}

impl Default for AnalysisDetail {
    fn default() -> Self {
        AnalysisDetail::Medium
    }
}

impl From<&str> for AnalysisDetail {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "low" | "basic" => AnalysisDetail::Low,
            "medium" | "standard" => AnalysisDetail::Medium,
            "high" | "detailed" => AnalysisDetail::High,
            _ => AnalysisDetail::Medium, // Default
        }
    }
}

/// Represents a code definition (function, class, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeDefinition {
    /// Type of definition (function, class, method, etc.)
    pub def_type: String,
    /// Name of the definition
    pub name: String,
    /// Line number where the definition starts
    pub line: usize,
    /// Column number where the definition starts
    pub column: Option<usize>,
    /// Arguments or parameters (if applicable)
    pub args: Option<Vec<String>>,
    /// Return type (if applicable)
    pub return_type: Option<String>,
    /// Visibility or access modifier (public, private, etc.)
    pub visibility: Option<String>,
    /// Documentation comment associated with this definition
    pub doc_comment: Option<String>,
    /// Full text of the definition including body (if detail level is high)
    pub full_text: Option<String>,
}

/// Represents an import statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeImport {
    /// Module, package, or namespace being imported
    pub module: String,
    /// Line number where the import appears
    pub line: usize,
    /// Column number where the import appears
    pub column: Option<usize>,
    /// Alias or 'as' name if applicable
    pub alias: Option<String>,
    /// Specific items imported from the module
    pub items: Option<Vec<String>>,
    /// Full text of the import statement
    pub full_text: String,
}

/// Represents a usage of a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeUsage {
    /// Name of the symbol being used
    pub name: String,
    /// Line number where the usage appears
    pub line: usize,
    /// Column number where the usage appears
    pub column: usize,
    /// Context snippet showing how the symbol is used
    pub context: String,
    /// Type of the usage (if determinable)
    pub usage_type: Option<String>,
}

/// Analysis results from a language analyzer
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResults {
    /// Language of the analyzed code
    pub language: String,
    /// File path that was analyzed
    pub file_path: Option<String>,
    /// Detected code definitions
    pub definitions: Vec<CodeDefinition>,
    /// Detected imports
    pub imports: Vec<CodeImport>,
    /// Detected symbol usages
    pub usages: Vec<CodeUsage>,
    /// Analysis errors or warnings
    pub messages: Vec<String>,
}

/// Common trait for all language analyzers
pub trait LanguageAnalyzer {
    /// Identifies if a file is compatible with this analyzer
    fn is_compatible(&self, file_path: &Path) -> bool;
    
    /// Returns the name of the language this analyzer handles
    fn language_name(&self) -> &'static str;
    
    /// Analyzes code from a string
    fn analyze_code(
        &self,
        code: &str,
        analysis_type: AnalysisType,
        detail_level: AnalysisDetail,
    ) -> Result<AnalysisResults>;
    
    /// Analyzes code from a file
    fn analyze_file(
        &self,
        file_path: &Path,
        analysis_type: AnalysisType,
        detail_level: AnalysisDetail,
    ) -> Result<AnalysisResults>;
}