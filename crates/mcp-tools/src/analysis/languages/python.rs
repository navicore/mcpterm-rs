use super::common::{
    AnalysisDetail, AnalysisResults, AnalysisType, CodeDefinition, CodeImport, CodeUsage,
    LanguageAnalyzer,
};
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Analyzer for Python source code
#[derive(Debug, Default)]
pub struct PythonAnalyzer;

impl PythonAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Extract function definitions from Python code
    fn extract_functions(&self, code: &str, detail_level: AnalysisDetail) -> Vec<CodeDefinition> {
        let mut functions = Vec::new();

        // Regular expression for function definitions
        // Captures: docstrings, decorators, function name, parameters
        let fn_re = Regex::new(
            r#"(?:@(?P<decorator>\w+)(?:\(.*?\))?\n)*\s*def\s+(?P<name>\w+)\s*\((?P<params>[^)]*)\)(?:\s*->\s*(?P<ret>[^:]+))?\s*:"#,
        )
        .unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        for cap in fn_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();

            let params_str = cap.name("params").map_or("", |m| m.as_str());
            let args = if detail_level > AnalysisDetail::Low {
                Some(
                    params_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                )
            } else {
                None
            };

            let return_type = if detail_level > AnalysisDetail::Low {
                cap.name("ret").map(|m| m.as_str().trim().to_string())
            } else {
                None
            };

            // Determine visibility from name (Python convention)
            let visibility = if name.starts_with('_') && !name.starts_with("__") {
                Some("protected".to_string())
            } else if name.starts_with("__") && !name.ends_with("__") {
                Some("private".to_string())
            } else {
                Some("public".to_string())
            };

            // Get docstring
            let doc_comment = cap.name("docstring").map(|m| {
                m.as_str()
                    .trim_matches(|c| c == '\'' || c == '"')
                    .trim()
                    .to_string()
            });

            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            functions.push(CodeDefinition {
                def_type: "function".to_string(),
                name,
                line,
                column: None,
                args,
                return_type,
                visibility,
                doc_comment,
                full_text,
            });
        }

        functions
    }

    /// Extract class definitions from Python code
    fn extract_classes(&self, code: &str, detail_level: AnalysisDetail) -> Vec<CodeDefinition> {
        let mut classes = Vec::new();

        // Regular expression for class definitions
        // Captures: docstrings, decorators, class name, parent classes
        let class_re = Regex::new(
            r#"(?:@(?P<decorator>\w+)(?:\(.*?\))?\n)*\s*class\s+(?P<name>\w+)(?:\s*\((?P<parents>[^)]*)\))?\s*:"#,
        )
        .unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        for cap in class_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();

            // Get parent classes
            let parents = cap.name("parents").map(|m| {
                m.as_str()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<String>>()
            });

            // Determine visibility from name (Python convention)
            let visibility = if name.starts_with('_') && !name.starts_with("__") {
                Some("protected".to_string())
            } else if name.starts_with("__") && !name.ends_with("__") {
                Some("private".to_string())
            } else {
                Some("public".to_string())
            };

            // Get docstring
            let doc_comment = cap.name("docstring").map(|m| {
                m.as_str()
                    .trim_matches(|c| c == '\'' || c == '"')
                    .trim()
                    .to_string()
            });

            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            classes.push(CodeDefinition {
                def_type: "class".to_string(),
                name,
                line,
                column: None,
                args: parents,
                return_type: None,
                visibility,
                doc_comment,
                full_text,
            });
        }

        classes
    }

    /// Extract method definitions from classes in Python code
    fn extract_methods(&self, code: &str, detail_level: AnalysisDetail) -> Vec<CodeDefinition> {
        let mut methods = Vec::new();

        // Regular expression for method definitions in classes
        // Similar to function regex but requires indentation
        let method_re = Regex::new(
            r#"(?:[ \t]+@(?P<decorator>\w+)(?:\(.*?\))?\n)*[ \t]+def\s+(?P<name>\w+)\s*\((?P<params>[^)]*)\)(?:\s*->\s*(?P<ret>[^:]+))?\s*:"#,
        )
        .unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        for cap in method_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();

            let params_str = cap.name("params").map_or("", |m| m.as_str());

            // For methods, the first parameter is usually self/cls
            let has_self =
                params_str.trim().starts_with("self") || params_str.trim().starts_with("cls");

            let is_static = cap
                .name("decorator")
                .map_or(false, |m| m.as_str() == "staticmethod");

            let is_class_method = cap
                .name("decorator")
                .map_or(false, |m| m.as_str() == "classmethod");

            let method_type = if is_static {
                "static_method"
            } else if is_class_method {
                "class_method"
            } else if has_self {
                "instance_method"
            } else {
                "method" // Generic fallback
            };

            let args = if detail_level > AnalysisDetail::Low {
                Some(
                    params_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                )
            } else {
                None
            };

            let return_type = if detail_level > AnalysisDetail::Low {
                cap.name("ret").map(|m| m.as_str().trim().to_string())
            } else {
                None
            };

            // Determine visibility from name (Python convention)
            let visibility = if name.starts_with('_') && !name.starts_with("__") {
                Some("protected".to_string())
            } else if name.starts_with("__") && !name.ends_with("__") {
                Some("private".to_string())
            } else {
                Some("public".to_string())
            };

            // Get docstring
            let doc_comment = cap.name("docstring").map(|m| {
                m.as_str()
                    .trim_matches(|c| c == '\'' || c == '"')
                    .trim()
                    .to_string()
            });

            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            methods.push(CodeDefinition {
                def_type: method_type.to_string(),
                name,
                line,
                column: None,
                args,
                return_type,
                visibility,
                doc_comment,
                full_text,
            });
        }

        methods
    }

    /// Extract import statements from Python code
    fn extract_imports(&self, code: &str) -> Vec<CodeImport> {
        let mut imports = Vec::new();

        // Regular expressions for different import styles

        // Regular import: import module or import module as alias
        let import_re =
            Regex::new(r"import\s+(?P<module>[\w.]+)(?:\s+as\s+(?P<alias>\w+))?").unwrap();

        // From import: from module import item, item2
        let from_import_re =
            Regex::new(r"from\s+(?P<module>[\w.]+)\s+import\s+(?P<items>[^#\n]+)").unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        // Process regular imports
        for cap in import_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let full_text = cap.get(0).unwrap().as_str().to_string();

            let module = cap.name("module").map_or("", |m| m.as_str()).to_string();

            let alias = cap.name("alias").map(|m| m.as_str().to_string());

            imports.push(CodeImport {
                module,
                line,
                column: None,
                alias,
                items: None,
                full_text,
            });
        }

        // Process from imports
        for cap in from_import_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let full_text = cap.get(0).unwrap().as_str().to_string();

            let module = cap.name("module").map_or("", |m| m.as_str()).to_string();

            let items_str = cap.name("items").map_or("", |m| m.as_str());

            // Parse the imported items, handling potential 'as' aliases
            let items = Some(
                items_str
                    .split(',')
                    .map(|s| {
                        let parts: Vec<&str> = s.trim().split("as").collect();
                        parts[0].trim().to_string()
                    })
                    .filter(|s| !s.is_empty())
                    .collect(),
            );

            imports.push(CodeImport {
                module,
                line,
                column: None,
                alias: None,
                items,
                full_text,
            });
        }

        imports
    }

    /// Extract symbol usages from Python code
    fn extract_usages(&self, code: &str, defined_symbols: &[String]) -> Vec<CodeUsage> {
        let mut usages = Vec::new();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        // Find usages of defined symbols
        for symbol in defined_symbols {
            // Create a simple regex to find the symbol
            let usage_re = Regex::new(&format!(r"\b{}\b", regex::escape(symbol))).unwrap();

            for cap in usage_re.find_iter(code) {
                let start_pos = cap.start();

                // Skip matches that are part of a definition
                let line_content = if start_pos > 0 {
                    let line_start = code[..start_pos].rfind('\n').map_or(0, |pos| pos + 1);
                    &code[line_start..start_pos]
                } else {
                    ""
                };

                if line_content.trim().starts_with("def ")
                    || line_content.trim().starts_with("class ")
                {
                    continue;
                }

                // Convert byte position to line and column
                let line = line_offsets
                    .iter()
                    .take_while(|&&offset| offset < start_pos)
                    .count()
                    + 1;

                // Calculate context range (one line of code)
                let line_start = if line > 1 {
                    line_offsets[line - 2] + 1
                } else {
                    0
                };

                let line_end = line_offsets
                    .get(line - 1)
                    .copied()
                    .unwrap_or_else(|| code.len());

                let context = code[line_start..line_end].trim().to_string();
                let column = start_pos - line_start + 1;

                // Determine usage type based on context
                let usage_type = if context.contains(&format!("{}.call(", symbol)) {
                    Some("method_call".to_string())
                } else if context.contains(&format!("= {}", symbol)) {
                    Some("assignment".to_string())
                } else {
                    Some("reference".to_string())
                };

                usages.push(CodeUsage {
                    name: symbol.clone(),
                    line,
                    column,
                    context,
                    usage_type,
                });
            }
        }

        usages
    }
}

impl LanguageAnalyzer for PythonAnalyzer {
    fn is_compatible(&self, file_path: &Path) -> bool {
        if let Some(ext) = file_path.extension() {
            ext == "py"
        } else {
            false
        }
    }

    fn language_name(&self) -> &'static str {
        "Python"
    }

    fn analyze_code(
        &self,
        code: &str,
        analysis_type: AnalysisType,
        detail_level: AnalysisDetail,
    ) -> Result<AnalysisResults> {
        let mut definitions = Vec::new();
        let mut imports = Vec::new();
        let mut usages = Vec::new();
        let messages = Vec::new();

        // Extract function definitions if requested
        if matches!(
            analysis_type,
            AnalysisType::Definitions | AnalysisType::Comprehensive
        ) {
            definitions.extend(self.extract_functions(code, detail_level));
            definitions.extend(self.extract_methods(code, detail_level));
        }

        // Extract class definitions if requested
        if matches!(
            analysis_type,
            AnalysisType::DataStructures | AnalysisType::Comprehensive
        ) {
            definitions.extend(self.extract_classes(code, detail_level));
        }

        // Extract imports if requested
        if matches!(
            analysis_type,
            AnalysisType::Imports | AnalysisType::Comprehensive
        ) {
            imports = self.extract_imports(code);
        }

        // Extract usages if requested
        if matches!(
            analysis_type,
            AnalysisType::Usages | AnalysisType::Comprehensive
        ) {
            // Collect defined symbol names for usage lookup
            let defined_symbols = definitions
                .iter()
                .map(|def| def.name.clone())
                .collect::<Vec<String>>();

            usages = self.extract_usages(code, &defined_symbols);
        }

        Ok(AnalysisResults {
            language: self.language_name().to_string(),
            file_path: None,
            definitions,
            imports,
            usages,
            messages,
        })
    }

    fn analyze_file(
        &self,
        file_path: &Path,
        analysis_type: AnalysisType,
        detail_level: AnalysisDetail,
    ) -> Result<AnalysisResults> {
        // Check if file exists and has the correct extension
        if !file_path.exists() {
            return Err(anyhow::anyhow!("File does not exist: {:?}", file_path));
        }

        if !self.is_compatible(file_path) {
            return Err(anyhow::anyhow!(
                "File is not a Python source file: {:?}",
                file_path
            ));
        }

        // Read the file content
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;

        // Analyze the code
        let mut results = self.analyze_code(&content, analysis_type, detail_level)?;

        // Add the file path to the results
        results.file_path = Some(file_path.to_string_lossy().to_string());

        Ok(results)
    }
}
