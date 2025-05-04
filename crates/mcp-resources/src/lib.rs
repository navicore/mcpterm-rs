use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    File,
    Memory,
    // Could add more types in the future
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    pub uri: String,
    pub resource_type: ResourceType,
    pub access_mode: AccessMode,
    // Additional metadata fields will be added as needed
}

pub trait Resource {
    fn metadata(&self) -> Result<ResourceMetadata>;
    fn read(&self) -> Result<Vec<u8>>;
    fn write(&mut self, content: &[u8]) -> Result<()>;
    fn access_mode(&self) -> AccessMode;
    fn delete(&mut self) -> Result<()>;
}

pub struct ResourceManager {
    base_dir: PathBuf,
    // Additional fields will be added in the implementation
}

impl ResourceManager {
    pub fn new<P: Into<PathBuf>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    // These are placeholder functions to be implemented
    pub fn register_file(&self, _path: &str, _mode: AccessMode) -> String {
        // Placeholder
        String::from("file://example")
    }

    pub fn read_resource(&self, _uri: &str) -> Result<Vec<u8>> {
        // Placeholder
        Ok(Vec::new())
    }
}

// File-specific resource implementation will be added here
// Memory resource implementation will be added here

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_manager_creation() {
        let manager = ResourceManager::new("/tmp");
        assert_eq!(manager.base_dir, PathBuf::from("/tmp"));
    }
}
