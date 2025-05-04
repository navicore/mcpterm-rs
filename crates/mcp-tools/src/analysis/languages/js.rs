use super::common::{
    AnalysisDetail, AnalysisResults, AnalysisType, CodeDefinition, CodeImport, CodeUsage,
    LanguageAnalyzer,
};
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Analyzer for JavaScript/TypeScript source code
#[derive(Debug, Default)]
pub struct JsAnalyzer;

impl JsAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Extract function definitions from JS/TS code
    fn extract_functions(&self, code: &str, detail_level: AnalysisDetail) -> Vec<CodeDefinition> {
        let mut functions = Vec::new();

        // Regular expressions for different function definition styles

        // Standard function declaration
        let fn_decl_re = Regex::new(
            r"(?:/\*\*(?P<doc>[\s\S]*?)\*/\s*)?(?:export\s+)?(?:async\s+)?function\s+(?P<name>\w+)\s*(?P<params>\([^)]*\))(?:\s*:\s*(?P<ret>[^{]+))?\s*\{",
        )
        .unwrap();

        // Arrow functions with explicit assignment
        let arrow_fn_re = Regex::new(
            r"(?:/\*\*(?P<doc>[\s\S]*?)\*/\s*)?(?:export\s+)?(?:const|let|var)\s+(?P<name>\w+)\s*=\s*(?:async\s+)?(?P<params>\([^)]*\))(?:\s*:\s*(?P<ret>[^=]+))?\s*=>"
        )
        .unwrap();

        // Class method definitions
        let method_re = Regex::new(
            r"(?:/\*\*(?P<doc>[\s\S]*?)\*/\s*)?(?P<vis>public|private|protected)?\s*(?:static\s+)?(?:async\s+)?(?P<name>\w+)\s*(?P<params>\([^)]*\))(?:\s*:\s*(?P<ret>[^{]+))?\s*\{"
        )
        .unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        // Process standard function declarations
        for cap in fn_decl_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();

            let params_str = cap.name("params").map_or("()", |m| m.as_str());
            let args = if detail_level > AnalysisDetail::Low {
                Some(
                    params_str
                        .trim_matches(|c| c == '(' || c == ')')
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

            let doc_comment = cap.name("doc").map(|m| {
                m.as_str()
                    .lines()
                    .map(|line| line.trim_start_matches(['*', ' ']))
                    .collect::<Vec<&str>>()
                    .join("\n")
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
                visibility: Some("public".to_string()),
                doc_comment,
                full_text,
            });
        }

        // Process arrow function assignments
        for cap in arrow_fn_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();

            let params_str = cap.name("params").map_or("()", |m| m.as_str());
            let args = if detail_level > AnalysisDetail::Low {
                Some(
                    params_str
                        .trim_matches(|c| c == '(' || c == ')')
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

            let doc_comment = cap.name("doc").map(|m| {
                m.as_str()
                    .lines()
                    .map(|line| line.trim_start_matches(['*', ' ']))
                    .collect::<Vec<&str>>()
                    .join("\n")
                    .trim()
                    .to_string()
            });

            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            functions.push(CodeDefinition {
                def_type: "arrow_function".to_string(),
                name,
                line,
                column: None,
                args,
                return_type,
                visibility: Some("public".to_string()),
                doc_comment,
                full_text,
            });
        }

        // Process class methods
        for cap in method_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();

            let params_str = cap.name("params").map_or("()", |m| m.as_str());
            let args = if detail_level > AnalysisDetail::Low {
                Some(
                    params_str
                        .trim_matches(|c| c == '(' || c == ')')
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

            let visibility = cap
                .name("vis")
                .map(|m| m.as_str().to_string())
                .or_else(|| Some("public".to_string())); // Default to public

            let doc_comment = cap.name("doc").map(|m| {
                m.as_str()
                    .lines()
                    .map(|line| line.trim_start_matches(['*', ' ']))
                    .collect::<Vec<&str>>()
                    .join("\n")
                    .trim()
                    .to_string()
            });

            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            functions.push(CodeDefinition {
                def_type: "method".to_string(),
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

    /// Extract class definitions from JS/TS code
    fn extract_classes(&self, code: &str, detail_level: AnalysisDetail) -> Vec<CodeDefinition> {
        let mut classes = Vec::new();

        // Regular expression for class definitions
        let class_re = Regex::new(
            r"(?:/\*\*(?P<doc>[\s\S]*?)\*/\s*)?(?:export\s+)?class\s+(?P<name>\w+)(?:\s+extends\s+(?P<extends>\w+))?\s*\{",
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

            // Extract extends information (if available)
            let extends = cap.name("extends").map(|m| m.as_str().to_string());

            let doc_comment = cap.name("doc").map(|m| {
                m.as_str()
                    .lines()
                    .map(|line| line.trim_start_matches(['*', ' ']))
                    .collect::<Vec<&str>>()
                    .join("\n")
                    .trim()
                    .to_string()
            });

            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            // Store extends info in the args field
            let args = extends.map(|e| vec![format!("extends {}", e)]);

            classes.push(CodeDefinition {
                def_type: "class".to_string(),
                name,
                line,
                column: None,
                args,
                return_type: None,
                visibility: Some("public".to_string()),
                doc_comment,
                full_text,
            });
        }

        classes
    }

    /// Extract interface and type definitions (TypeScript)
    fn extract_types(&self, code: &str, detail_level: AnalysisDetail) -> Vec<CodeDefinition> {
        let mut types = Vec::new();

        // Regular expression for interface definitions
        let interface_re = Regex::new(
            r"(?:/\*\*(?P<doc>[\s\S]*?)\*/\s*)?(?:export\s+)?interface\s+(?P<name>\w+)(?:\s+extends\s+(?P<extends>[^{]+))?\s*\{",
        )
        .unwrap();

        // Regular expression for type aliases
        let type_re = Regex::new(
            r"(?:/\*\*(?P<doc>[\s\S]*?)\*/\s*)?(?:export\s+)?type\s+(?P<name>\w+)(?:<[^>]*>)?\s*=\s*",
        )
        .unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        // Process interface definitions
        for cap in interface_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();

            // Extract extends information (if available)
            let extends = cap.name("extends").map(|m| m.as_str().to_string());

            let doc_comment = cap.name("doc").map(|m| {
                m.as_str()
                    .lines()
                    .map(|line| line.trim_start_matches(['*', ' ']))
                    .collect::<Vec<&str>>()
                    .join("\n")
                    .trim()
                    .to_string()
            });

            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            // Store extends info in the args field
            let args = extends.map(|e| vec![format!("extends {}", e)]);

            types.push(CodeDefinition {
                def_type: "interface".to_string(),
                name,
                line,
                column: None,
                args,
                return_type: None,
                visibility: Some("public".to_string()),
                doc_comment,
                full_text,
            });
        }

        // Process type aliases
        for cap in type_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();

            let doc_comment = cap.name("doc").map(|m| {
                m.as_str()
                    .lines()
                    .map(|line| line.trim_start_matches(['*', ' ']))
                    .collect::<Vec<&str>>()
                    .join("\n")
                    .trim()
                    .to_string()
            });

            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            types.push(CodeDefinition {
                def_type: "type".to_string(),
                name,
                line,
                column: None,
                args: None,
                return_type: None,
                visibility: Some("public".to_string()),
                doc_comment,
                full_text,
            });
        }

        types
    }

    /// Extract import statements from JS/TS code
    fn extract_imports(&self, code: &str) -> Vec<CodeImport> {
        let mut imports = Vec::new();

        // Regular expression for import statements
        // Handles various import formats:
        // - import X from 'module';
        // - import { X, Y } from 'module';
        // - import * as X from 'module';
        let import_re = Regex::new(
            r#"import\s+(?:(?P<def>\w+)(?:\s*,\s*\{\s*(?P<named>[^}]+)\s*\})?|\*\s+as\s+(?P<ns>\w+)|\{\s*(?P<items>[^}]+)\s*\})\s+from\s+['"](?P<module>[^'"]+)['"];?"#,
        )
        .unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        for cap in import_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let full_text = cap.get(0).unwrap().as_str().to_string();

            let module = cap.name("module").map_or("", |m| m.as_str()).to_string();

            // Extract imported items
            let mut all_items = Vec::new();

            // Default export
            if let Some(def) = cap.name("def") {
                all_items.push(def.as_str().trim().to_string());
            }

            // Named exports
            if let Some(items) = cap.name("items").or_else(|| cap.name("named")) {
                all_items.extend(
                    items
                        .as_str()
                        .split(',')
                        .map(|s| {
                            let parts: Vec<&str> = s.split("as").collect();
                            parts[0].trim().to_string()
                        })
                        .filter(|s| !s.is_empty()),
                );
            }

            // Namespace import
            let alias = cap.name("ns").map(|m| m.as_str().trim().to_string());

            let items = if !all_items.is_empty() {
                Some(all_items)
            } else {
                None
            };

            imports.push(CodeImport {
                module,
                line,
                column: None,
                alias,
                items,
                full_text,
            });
        }

        imports
    }

    /// Extract symbol usages from JS/TS code
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
            // Simple regex to find the symbol - we'll filter out declarations later
            let usage_re = Regex::new(&format!(r"\b{}\b", regex::escape(symbol))).unwrap();

            for cap in usage_re.find_iter(code) {
                let start_pos = cap.start();

                // Skip matches at the beginning of a line (likely declarations)
                let line_start_pos = if start_pos > 0 {
                    code[..start_pos].rfind('\n').map_or(0, |pos| pos + 1)
                } else {
                    0
                };

                let prefix = &code[line_start_pos..start_pos];
                // Skip if this looks like a declaration or definition
                if prefix.trim().ends_with("function") || prefix.trim().ends_with("class") {
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

                let line_end = line_offsets.get(line - 1).copied().unwrap_or(code.len());

                let context = code[line_start..line_end].trim().to_string();
                let column = start_pos - line_start + 1;

                // Determine usage type based on context
                let usage_type = if context.contains(&format!("new {}", symbol)) {
                    Some("instantiation".to_string())
                } else if context.contains(&format!("{}.prototype", symbol)) {
                    Some("prototype_access".to_string())
                } else if context.contains(&format!("{}.call(", symbol))
                    || context.contains(&format!("{}.apply(", symbol))
                {
                    Some("method_call".to_string())
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

impl LanguageAnalyzer for JsAnalyzer {
    fn is_compatible(&self, file_path: &Path) -> bool {
        if let Some(ext) = file_path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            matches!(ext_str.as_str(), "js" | "jsx" | "ts" | "tsx")
        } else {
            false
        }
    }

    fn language_name(&self) -> &'static str {
        "JavaScript/TypeScript"
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
        }

        // Extract class definitions if requested
        if matches!(
            analysis_type,
            AnalysisType::DataStructures | AnalysisType::Comprehensive
        ) {
            definitions.extend(self.extract_classes(code, detail_level));

            // Extract TypeScript interfaces and types
            if code.contains("interface ") || code.contains("type ") {
                definitions.extend(self.extract_types(code, detail_level));
            }
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

        // Determine language variant
        let language =
            if code.contains(": ") || code.contains("interface ") || code.contains("type ") {
                "TypeScript"
            } else {
                "JavaScript"
            };

        Ok(AnalysisResults {
            language: language.to_string(),
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
                "File is not a JavaScript/TypeScript source file: {:?}",
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
