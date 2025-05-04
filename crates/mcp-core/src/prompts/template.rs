use std::collections::HashMap;
use tracing::{debug, warn};

/// Template engine for prompt substitution
pub struct TemplateEngine {
    /// Map of variable name to value
    variables: HashMap<String, String>,
}

impl TemplateEngine {
    /// Create a new template engine with no variables
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }
    
    /// Add a variable to the template engine
    pub fn with_var<S: Into<String>>(mut self, name: S, value: S) -> Self {
        self.variables.insert(name.into(), value.into());
        self
    }
    
    /// Set a variable in the template engine
    pub fn set_var<S: Into<String>>(&mut self, name: S, value: S) {
        self.variables.insert(name.into(), value.into());
    }
    
    /// Get a variable from the template engine
    pub fn get_var(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(|s| s.as_str())
    }
    
    /// Render a template with the current variables
    pub fn render(&self, template: &str) -> String {
        let mut result = template.to_string();
        
        // Process all variables in the template
        for (name, value) in &self.variables {
            let pattern = format!("{{{{{}}}}}",name);
            
            // Replace all occurrences of the pattern with the value
            if result.contains(&pattern) {
                debug!("Substituting template variable: {} -> {}", name, value);
                result = result.replace(&pattern, value);
            }
        }
        
        // Check for any remaining variable patterns
        let mut missing_vars = Vec::new();
        let re = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        
        for cap in re.captures_iter(&result) {
            if let Some(var_name) = cap.get(1) {
                missing_vars.push(var_name.as_str().to_string());
            }
        }
        
        // Log missing variables
        if !missing_vars.is_empty() {
            warn!("Template contains undefined variables: {:?}", missing_vars);
        }
        
        result
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_template_substitution_basic() {
        let engine = TemplateEngine::new()
            .with_var("name", "World")
            .with_var("greeting", "Hello");
            
        let template = "{{greeting}}, {{name}}!";
        let result = engine.render(template);
        
        assert_eq!(result, "Hello, World!");
    }
    
    #[test]
    fn test_template_missing_vars() {
        let engine = TemplateEngine::new()
            .with_var("name", "World");
            
        let template = "{{greeting}}, {{name}}!";
        let result = engine.render(template);
        
        // Missing variable should remain in the template
        assert_eq!(result, "{{greeting}}, World!");
    }
    
    #[test]
    fn test_template_multiple_occurrences() {
        let engine = TemplateEngine::new()
            .with_var("var", "value");
            
        let template = "{{var}} {{var}} {{var}}";
        let result = engine.render(template);
        
        assert_eq!(result, "value value value");
    }
    
    #[test]
    fn test_template_set_var() {
        let mut engine = TemplateEngine::new();
        engine.set_var("var1", "value1");
        engine.set_var("var2", "value2");
        
        let template = "{{var1}} and {{var2}}";
        let result = engine.render(template);
        
        assert_eq!(result, "value1 and value2");
        
        // Change a variable
        engine.set_var("var1", "new_value");
        let result = engine.render(template);
        
        assert_eq!(result, "new_value and value2");
    }
}