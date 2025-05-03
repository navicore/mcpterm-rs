use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::error::Error;

/// Validates a file path
pub fn validate_file_path(path: &str) -> Result<(), Error> {
    // Check that the path is not empty
    if path.is_empty() {
        return Err(Error::invalid_params());
    }

    // Create a Path object
    let path_obj = Path::new(path);

    // Basic path validation - check if it has a root, is absolute, etc.
    if !path_obj.is_absolute() {
        return Err(Error::invalid_params());
    }

    // Check for any suspicious path components
    if path.contains("..") {
        return Err(Error::invalid_params());
    }

    Ok(())
}

/// Validates a resource URI
pub fn validate_resource_uri(uri: &str) -> Result<(), Error> {
    // Check that the URI is not empty
    if uri.is_empty() {
        return Err(Error::invalid_params());
    }

    // Basic URI validation - must have a scheme
    if !uri.contains("://") {
        return Err(Error::invalid_params());
    }

    // Extract scheme
    let scheme = uri.split("://").next().unwrap_or("");

    // Validate supported schemes
    match scheme {
        "file" => {
            // For file URIs, validate the path
            let path = uri.trim_start_matches("file://");
            validate_file_path(path)?;
        }
        "memory" => {
            // Memory URIs are valid by default
        }
        _ => {
            // Unsupported scheme
            return Err(Error::invalid_params());
        }
    }

    Ok(())
}

/// Validates a shell command
pub fn validate_shell_command(command: &str) -> Result<(), Error> {
    // Check that the command is not empty
    if command.is_empty() {
        return Err(Error::invalid_params());
    }

    // Basic command validation
    // In a full implementation, you would check against a whitelist
    // or apply more sophisticated security checks

    Ok(())
}

/// Validates a path component (filename or directory name)
pub fn validate_path_component(path: &str) -> Result<()> {
    // Check that the path is not empty
    if path.is_empty() {
        return Err(anyhow!("Path cannot be empty"));
    }

    // Check for suspicious path components
    if path.contains("..") || path.contains("~") {
        return Err(anyhow!("Path contains invalid components"));
    }

    // Check for absolute paths in components
    if Path::new(path).is_absolute() {
        return Err(anyhow!("Path must be relative to base directory"));
    }

    Ok(())
}

/// Validates that a path is within a base directory
pub fn validate_path_within_base(base_dir: &Path, path: &Path) -> Result<()> {
    // Convert both paths to absolute and canonical form
    let base_canonical = base_dir
        .canonicalize()
        .map_err(|_| anyhow!("Failed to canonicalize base directory"))?;

    // Check if the path exists, if not use its parent directory for validation
    let path_to_check = if path.exists() {
        path.canonicalize()
            .map_err(|_| anyhow!("Failed to canonicalize path"))?
    } else {
        // If path doesn't exist, check its parent directory
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("Path has no parent directory"))?;

        if !parent.exists() {
            return Err(anyhow!("Parent directory does not exist"));
        }

        let parent_canonical = parent
            .canonicalize()
            .map_err(|_| anyhow!("Failed to canonicalize parent directory"))?;

        // Construct a canonical path by joining the parent with the file name
        let file_name = path
            .file_name()
            .ok_or_else(|| anyhow!("Path has no file name"))?;

        parent_canonical.join(file_name)
    };

    // Check if the path starts with the base directory
    let is_within_base = path_to_check.starts_with(&base_canonical);

    if !is_within_base {
        return Err(anyhow!("Path is not within the base directory"));
    }

    Ok(())
}

/// Validates JSON Schema compliance
pub fn validate_against_schema(value: &Value, schema: &Value) -> Result<(), Error> {
    // In a full implementation, you would use a JSON Schema validator
    // For now, we just do some basic type checking

    // If schema specifies a type, check that value matches
    if let Some(type_value) = schema.get("type") {
        if let Some(type_str) = type_value.as_str() {
            match type_str {
                "string" if !value.is_string() => {
                    return Err(Error::invalid_params());
                }
                "number" if !value.is_number() => {
                    return Err(Error::invalid_params());
                }
                "boolean" if !value.is_boolean() => {
                    return Err(Error::invalid_params());
                }
                "object" if !value.is_object() => {
                    return Err(Error::invalid_params());
                }
                "array" if !value.is_array() => {
                    return Err(Error::invalid_params());
                }
                "null" if !value.is_null() => {
                    return Err(Error::invalid_params());
                }
                _ => {}
            }
        }
    }

    // Check required properties for objects
    if let Some(required) = schema.get("required") {
        if let Some(required_props) = required.as_array() {
            if let Some(obj) = value.as_object() {
                for prop in required_props {
                    if let Some(prop_name) = prop.as_str() {
                        if !obj.contains_key(prop_name) {
                            return Err(Error::invalid_params());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
