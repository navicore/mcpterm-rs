use mcp_tools::analysis::languages::{
    common::{AnalysisDetail, AnalysisType, LanguageAnalyzer},
    rust::RustAnalyzer,
    js::JsAnalyzer,
    python::PythonAnalyzer,
};
use std::path::Path;

#[test]
fn test_rust_analyzer_basic() {
    let analyzer = RustAnalyzer::new();
    let rust_code = r#"
/// A simple function
pub fn hello_world() -> String {
    "Hello, World!".to_string()
}

struct Point {
    x: f64,
    y: f64,
}

// Test usage
fn test() {
    let message = hello_world();
    println!("{}", message);
}
"#;

    let result = analyzer
        .analyze_code(rust_code, AnalysisType::Comprehensive, AnalysisDetail::Medium)
        .unwrap();

    // Verify language detection
    assert_eq!(result.language, "Rust");
    
    // Verify function detection
    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type == "function")
        .collect();
    assert!(functions.len() > 0, "Should detect at least one function");
    
    // Verify struct detection
    let structs: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type == "struct")
        .collect();
    assert!(structs.len() > 0, "Should detect at least one struct");
    
    // Verify function details if hello_world is found
    if let Some(hello_fn) = functions.iter().find(|d| d.name == "hello_world") {
        if let Some(vis) = &hello_fn.visibility {
            assert!(vis.contains("public"), "hello_world should be public");
        }
        if let Some(ret) = &hello_fn.return_type {
            assert!(ret.contains("String"), "hello_world should return a String");
        }
    }
    
    // Check if there are any usages
    assert!(!result.usages.is_empty(), "Should detect at least some usages");
}

#[test]
fn test_js_analyzer_basic() {
    let analyzer = JsAnalyzer::new();
    let js_code = r#"
/**
 * A simple greeting function
 */
function greet(name) {
    return `Hello, ${name}!`;
}

// Arrow function
const farewell = (name) => {
    return `Goodbye, ${name}!`;
};

// Class
class Person {
    constructor(name) {
        this.name = name;
    }
    
    sayHello() {
        console.log(greet(this.name));
    }
}

// Usage
const person = new Person('John');
person.sayHello();
"#;

    let result = analyzer
        .analyze_code(js_code, AnalysisType::Comprehensive, AnalysisDetail::Medium)
        .unwrap();

    // Verify language detection
    assert_eq!(result.language, "JavaScript");
    
    // Verify function detection
    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type == "function")
        .collect();
    assert!(functions.len() >= 1);
    
    // Verify arrow function
    let arrow_functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type == "arrow_function")
        .collect();
    assert!(arrow_functions.len() >= 1);
    
    // Verify class detection
    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type == "class")
        .collect();
    assert_eq!(classes.len(), 1);
    
    // Verify method detection
    let methods: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type == "method")
        .collect();
    assert!(methods.len() >= 1);
}

#[test]
fn test_python_analyzer_basic() {
    let analyzer = PythonAnalyzer::new();
    let python_code = r#"
import os
from datetime import datetime

class Animal:
    """Base class for all animals"""
    
    def __init__(self, name):
        self.name = name
    
    def speak(self):
        pass
        
class Dog(Animal):
    def speak(self):
        return f"{self.name} says Woof!"

def get_current_time():
    """Returns the current time as string"""
    return datetime.now().strftime("%H:%M:%S")

# Usage
dog = Dog("Rex")
print(dog.speak())
print(get_current_time())
"#;

    let result = analyzer
        .analyze_code(python_code, AnalysisType::Comprehensive, AnalysisDetail::Medium)
        .unwrap();

    // Verify language detection
    assert_eq!(result.language, "Python");
    
    // Verify function detection
    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type == "function")
        .collect();
    assert!(functions.len() > 0, "Should detect at least one function");
    
    // Verify class detection
    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type == "class")
        .collect();
    assert!(classes.len() > 0, "Should detect at least one class");
    
    // Verify method detection
    let methods: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.def_type.contains("method"))
        .collect();
    assert!(methods.len() > 0, "Should detect at least one method");
    
    // Verify import detection
    assert!(result.imports.len() > 0, "Should detect at least one import");
}

#[test]
fn test_file_extension_detection() {
    let rust_analyzer = RustAnalyzer::new();
    let js_analyzer = JsAnalyzer::new();
    let python_analyzer = PythonAnalyzer::new();
    
    // Test Rust file detection
    assert!(rust_analyzer.is_compatible(Path::new("file.rs")));
    assert!(!rust_analyzer.is_compatible(Path::new("file.js")));
    assert!(!rust_analyzer.is_compatible(Path::new("file.py")));
    
    // Test JS/TS file detection
    assert!(js_analyzer.is_compatible(Path::new("file.js")));
    assert!(js_analyzer.is_compatible(Path::new("file.jsx")));
    assert!(js_analyzer.is_compatible(Path::new("file.ts")));
    assert!(js_analyzer.is_compatible(Path::new("file.tsx")));
    assert!(!js_analyzer.is_compatible(Path::new("file.rs")));
    assert!(!js_analyzer.is_compatible(Path::new("file.py")));
    
    // Test Python file detection
    assert!(python_analyzer.is_compatible(Path::new("file.py")));
    assert!(!python_analyzer.is_compatible(Path::new("file.rs")));
    assert!(!python_analyzer.is_compatible(Path::new("file.js")));
}