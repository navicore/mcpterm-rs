use super::common::{
    AnalysisDetail, AnalysisResults, AnalysisType, CodeDefinition, CodeImport, CodeUsage,
    LanguageAnalyzer,
};
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Analyzer for Rust source code
#[derive(Debug, Default)]
pub struct RustAnalyzer;

impl RustAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Extract function definitions from Rust code
    fn extract_functions(&self, code: &str, detail_level: AnalysisDetail) -> Vec<CodeDefinition> {
        let mut functions = Vec::new();

        // Regular expression for function definitions
        // Captures:
        // 1. Documentation comments (optional)
        // 2. Visibility modifier (optional)
        // 3. Function name
        // 4. Function parameters
        // 5. Return type (optional)
        let re = Regex::new(
            r"(?:///(?P<doc>.*)\n)*(?P<vis>pub(?:\s*\(.*\))?\s+)?(?:async\s+)?fn\s+(?P<name>\w+)\s*(?P<params>\([^)]*\))(?:\s*->\s*(?P<ret>[^{]+))?\s*\{",
        )
        .unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        for cap in re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            
            // Convert byte position to line number
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;
            
            // Extract the name
            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();
            
            // Extract parameters
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
            
            // Extract return type
            let return_type = if detail_level > AnalysisDetail::Low {
                cap.name("ret").map(|m| m.as_str().trim().to_string())
            } else {
                None
            };
            
            // Extract visibility
            let visibility = cap
                .name("vis")
                .map(|m| {
                    if m.as_str().starts_with("pub(") {
                        "restricted".to_string()
                    } else {
                        "public".to_string()
                    }
                })
                .or_else(|| Some("private".to_string()));
            
            // Extract doc comment
            let doc_comment = cap.name("doc").map(|m| m.as_str().trim().to_string());
            
            // Extract full text if detail level is high
            let full_text = if detail_level == AnalysisDetail::High {
                // Get the matched text plus some surrounding context
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };
            
            functions.push(CodeDefinition {
                def_type: "function".to_string(),
                name,
                line,
                column: None, // We'd need a more complex parser to get accurate column numbers
                args,
                return_type,
                visibility,
                doc_comment,
                full_text,
            });
        }

        functions
    }

    /// Extract struct and enum definitions from Rust code
    fn extract_data_structures(&self, code: &str, detail_level: AnalysisDetail) -> Vec<CodeDefinition> {
        let mut data_structures = Vec::new();

        // Regular expression for struct definitions
        let struct_re = Regex::new(
            r"(?:///(?P<doc>.*)\n)*(?P<vis>pub(?:\s*\(.*\))?\s+)?struct\s+(?P<name>\w+)(?:<[^>]*>)?(?:\s*\{[^}]*\}|\s*\([^)]*\)|\s*;)",
        )
        .unwrap();

        // Regular expression for enum definitions
        let enum_re = Regex::new(
            r"(?:///(?P<doc>.*)\n)*(?P<vis>pub(?:\s*\(.*\))?\s+)?enum\s+(?P<name>\w+)(?:<[^>]*>)?\s*\{",
        )
        .unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        // Process struct definitions
        for cap in struct_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();
            let visibility = cap
                .name("vis")
                .map(|m| {
                    if m.as_str().starts_with("pub(") {
                        "restricted".to_string()
                    } else {
                        "public".to_string()
                    }
                })
                .or_else(|| Some("private".to_string()));

            let doc_comment = cap.name("doc").map(|m| m.as_str().trim().to_string());
            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            data_structures.push(CodeDefinition {
                def_type: "struct".to_string(),
                name,
                line,
                column: None,
                args: None,
                return_type: None,
                visibility,
                doc_comment,
                full_text,
            });
        }

        // Process enum definitions
        for cap in enum_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;

            let name = cap.name("name").map_or("", |m| m.as_str()).to_string();
            let visibility = cap
                .name("vis")
                .map(|m| {
                    if m.as_str().starts_with("pub(") {
                        "restricted".to_string()
                    } else {
                        "public".to_string()
                    }
                })
                .or_else(|| Some("private".to_string()));

            let doc_comment = cap.name("doc").map(|m| m.as_str().trim().to_string());
            let full_text = if detail_level == AnalysisDetail::High {
                Some(cap.get(0).unwrap().as_str().to_string())
            } else {
                None
            };

            data_structures.push(CodeDefinition {
                def_type: "enum".to_string(),
                name,
                line,
                column: None,
                args: None,
                return_type: None,
                visibility,
                doc_comment,
                full_text,
            });
        }

        data_structures
    }

    /// Extract use statements (imports) from Rust code
    fn extract_imports(&self, code: &str) -> Vec<CodeImport> {
        let mut imports = Vec::new();

        // Regular expression for use statements
        let use_re = Regex::new(r"use\s+(?P<module>[^;{]+)(?:\s*\{(?P<items>[^}]+)\})?(?:\s+as\s+(?P<alias>\w+))?\s*;").unwrap();

        // Get line offsets for converting byte positions to line numbers
        let line_offsets: Vec<usize> = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i)
            .collect();

        for cap in use_re.captures_iter(code) {
            let start_pos = cap.get(0).unwrap().start();
            let line = line_offsets
                .iter()
                .take_while(|&&offset| offset < start_pos)
                .count()
                + 1;
            
            let full_text = cap.get(0).unwrap().as_str().to_string();
            
            let module = cap.name("module").map_or("", |m| m.as_str()).trim().to_string();
            
            let alias = cap.name("alias").map(|m| m.as_str().trim().to_string());
            
            let items = cap.name("items").map(|m| {
                m.as_str()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<String>>()
            });

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

    /// Extract symbol usages from Rust code
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
            let usage_re = Regex::new(&format!(
                r"\b{}\b",
                regex::escape(symbol)
            ))
            .unwrap();

            for cap in usage_re.find_iter(code) {
                let start_pos = cap.start();
                let _end_pos = cap.end();
                
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

                usages.push(CodeUsage {
                    name: symbol.clone(),
                    line,
                    column,
                    context,
                    usage_type: None,
                });
            }
        }

        usages
    }
}

impl LanguageAnalyzer for RustAnalyzer {
    fn is_compatible(&self, file_path: &Path) -> bool {
        if let Some(ext) = file_path.extension() {
            ext == "rs"
        } else {
            false
        }
    }

    fn language_name(&self) -> &'static str {
        "Rust"
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

        // Extract definitions if requested
        if matches!(
            analysis_type,
            AnalysisType::Definitions | AnalysisType::Comprehensive
        ) {
            definitions.extend(self.extract_functions(code, detail_level));
        }

        // Extract data structures if requested
        if matches!(
            analysis_type,
            AnalysisType::DataStructures | AnalysisType::Comprehensive
        ) {
            definitions.extend(self.extract_data_structures(code, detail_level));
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
                "File is not a Rust source file: {:?}",
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