use anyhow::{Context, Result};

use super::{AccessMode, Resource, ResourceMetadata, ResourceType};

/// Memory resource implementation
pub struct MemoryResource {
    /// Resource key
    key: String,

    /// Content
    content: Vec<u8>,

    /// Access mode
    mode: AccessMode,
}

impl MemoryResource {
    /// Create a new memory resource
    pub fn new(key: String, mode: AccessMode) -> Self {
        Self {
            key,
            content: Vec::new(),
            mode,
        }
    }

    /// Create a memory resource with initial content
    pub fn with_content(key: String, content: Vec<u8>, mode: AccessMode) -> Self {
        Self { key, content, mode }
    }

    /// Get the resource key
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Resource for MemoryResource {
    fn metadata(&self) -> Result<ResourceMetadata> {
        // Create resource URI
        let uri = format!("memory://{}", self.key);

        // Create resource metadata
        Ok(ResourceMetadata {
            uri,
            resource_type: ResourceType::Memory,
            content_type: None,
            size: Some(self.content.len() as u64),
            last_modified: None,
            metadata: None,
        })
    }

    fn read(&self) -> Result<Vec<u8>> {
        // Check access mode
        if self.mode == AccessMode::Write {
            anyhow::bail!("Resource is write-only");
        }

        // Return content clone
        Ok(self.content.clone())
    }

    fn write(&mut self, content: &[u8]) -> Result<()> {
        // Check access mode
        if self.mode == AccessMode::Read {
            anyhow::bail!("Resource is read-only");
        }

        // Replace content
        self.content = content.to_vec();

        Ok(())
    }

    fn exists(&self) -> bool {
        true
    }

    fn access_mode(&self) -> AccessMode {
        self.mode
    }

    fn delete(&mut self) -> Result<()> {
        // Clear content
        self.content.clear();

        Ok(())
    }
}
