// MCP Resources Module
// Provides access to files and other resources through a URI-based interface

use crate::mcp::protocol::error::Error;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use url::Url;

mod file;
mod memory;
pub mod methods;
#[cfg(test)]
mod tests;

pub use file::FileResource;
pub use memory::MemoryResource;

/// Resource metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    /// Resource URI
    pub uri: String,

    /// Resource type
    pub resource_type: ResourceType,

    /// Content type
    pub content_type: Option<String>,

    /// Size in bytes
    pub size: Option<u64>,

    /// Last modified timestamp
    pub last_modified: Option<String>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Resource type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    /// File resource
    File,

    /// Directory resource
    Directory,

    /// Memory resource
    Memory,

    /// Other resource type
    Other(String),
}

/// Resource access mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    /// Read-only access
    Read,

    /// Write-only access
    Write,

    /// Read-write access
    ReadWrite,
}

/// Resource trait - all resource types must implement this
pub trait Resource {
    /// Get resource metadata
    fn metadata(&self) -> Result<ResourceMetadata>;

    /// Read resource content
    fn read(&self) -> Result<Vec<u8>>;

    /// Write resource content
    fn write(&mut self, content: &[u8]) -> Result<()>;

    /// Check if resource exists
    fn exists(&self) -> bool;

    /// Get resource access mode
    fn access_mode(&self) -> AccessMode;

    /// Delete the resource
    fn delete(&mut self) -> Result<()>;
}

/// Resource manager
pub struct ResourceManager {
    /// Base directory for file resources
    base_dir: PathBuf,

    /// Memory resources
    memory_resources: std::collections::HashMap<String, Vec<u8>>,
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();

        // Create base directory if it doesn't exist
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir).context("Failed to create base directory")?;
        }

        Ok(Self {
            base_dir,
            memory_resources: std::collections::HashMap::new(),
        })
    }

    /// Parse a resource URI
    pub fn parse_uri(&self, uri: &str) -> Result<(String, PathBuf)> {
        let url = Url::parse(uri).context("Failed to parse URI")?;

        // Get scheme and path
        let scheme = url.scheme();
        let path = url.path();

        // Only file URIs are supported
        if scheme != "file" {
            return Err(anyhow!("Unsupported scheme: {}", scheme));
        }

        // Get the path
        let path = if path == "." || path.is_empty() {
            // Use base directory for "." or empty path
            self.base_dir.clone()
        } else {
            PathBuf::from(path)
        };

        // Convert to absolute path
        let absolute_path = if path.is_absolute() {
            path
        } else {
            self.base_dir.join(path)
        };

        // Security check - ensure path is within base_dir
        let canonicalized_path = absolute_path
            .canonicalize()
            .unwrap_or(absolute_path.clone());

        let canonicalized_base_dir = self
            .base_dir
            .canonicalize()
            .unwrap_or(self.base_dir.clone());

        // Convert paths to strings for comparison (handles Windows paths)
        let path_str = canonicalized_path.to_string_lossy().to_string();
        let base_dir_str = canonicalized_base_dir.to_string_lossy().to_string();

        if !path_str.starts_with(&base_dir_str) && path_str != base_dir_str {
            return Err(anyhow!(
                "Access denied: {} is outside of {}",
                path_str,
                base_dir_str
            ));
        }

        Ok((scheme.to_string(), absolute_path))
    }

    /// Get a resource by URI
    pub fn get_resource(&self, uri: &str) -> Result<Box<dyn Resource>> {
        let url = Url::parse(uri).context("Failed to parse URI")?;

        // Get scheme
        let scheme = url.scheme();

        match scheme {
            "file" => {
                // Parse URI to get path
                let (_, path) = self.parse_uri(uri)?;

                // Create file resource
                let resource = FileResource::new(path, AccessMode::Read);
                Ok(Box::new(resource))
            }
            "memory" => {
                // Get key from path
                let key = url.path().trim_start_matches('/');

                // Create memory resource
                let resource = MemoryResource::with_content(
                    key.to_string(),
                    self.memory_resources.get(key).cloned().unwrap_or_default(),
                    AccessMode::Read,
                );
                Ok(Box::new(resource))
            }
            _ => Err(anyhow!("Unsupported scheme: {}", scheme)),
        }
    }

    /// Create a new resource by URI
    pub fn create_resource(
        &mut self,
        uri: &str,
        access_mode: AccessMode,
    ) -> Result<Box<dyn Resource>> {
        let url = Url::parse(uri).context("Failed to parse URI")?;

        // Get scheme
        let scheme = url.scheme();

        match scheme {
            "file" => {
                // Parse URI to get path
                let (_, path) = self.parse_uri(uri)?;

                // Create parent directory if it doesn't exist
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent).context("Failed to create parent directory")?;
                    }
                }

                // Create file resource
                let resource = FileResource::new(path, access_mode);
                Ok(Box::new(resource))
            }
            "memory" => {
                // Get key from path
                let key = url.path().trim_start_matches('/');

                // Get existing content
                let content = self.memory_resources.get(key).cloned().unwrap_or_default();

                // Create memory resource
                let resource = MemoryResource::with_content(key.to_string(), content, access_mode);

                // Store in memory resources if it's a new key
                if !self.memory_resources.contains_key(key) {
                    self.memory_resources.insert(key.to_string(), Vec::new());
                }

                Ok(Box::new(resource))
            }
            _ => Err(anyhow!("Unsupported scheme: {}", scheme)),
        }
    }

    /// List resources in a directory
    pub fn list_resources(&self, uri: &str) -> Result<Vec<ResourceMetadata>> {
        let url = Url::parse(uri).context("Failed to parse URI")?;

        // Get scheme
        let scheme = url.scheme();

        match scheme {
            "file" => {
                // Parse URI to get path
                let (_, path) = self.parse_uri(uri)?;

                // Ensure path is a directory
                if !path.is_dir() {
                    return Err(anyhow!("Not a directory: {}", path.display()));
                }

                // List files in directory
                let mut resources = Vec::new();

                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let path = entry.path();

                    // Create resource URI
                    let path_str = path.to_string_lossy();
                    let resource_uri = format!("file://{}", path_str);

                    // Determine resource type
                    let resource_type = if path.is_dir() {
                        ResourceType::Directory
                    } else {
                        ResourceType::File
                    };

                    // Get metadata
                    let metadata = fs::metadata(&path)?;

                    // Create resource metadata
                    let resource_metadata = ResourceMetadata {
                        uri: resource_uri,
                        resource_type,
                        content_type: None, // Could infer content type from file extension
                        size: Some(metadata.len()),
                        last_modified: metadata
                            .modified()
                            .ok()
                            .map(|time| time.elapsed().unwrap_or_default().as_secs().to_string()),
                        metadata: None,
                    };

                    resources.push(resource_metadata);
                }

                Ok(resources)
            }
            "memory" => {
                // List memory resources
                let mut resources = Vec::new();

                for key in self.memory_resources.keys() {
                    // Create resource URI
                    let resource_uri = format!("memory://{}", key);

                    // Get content
                    let content_len = self
                        .memory_resources
                        .get(key)
                        .map(|content| content.len())
                        .unwrap_or(0);

                    // Create resource metadata
                    let resource_metadata = ResourceMetadata {
                        uri: resource_uri,
                        resource_type: ResourceType::Memory,
                        content_type: None,
                        size: Some(content_len as u64),
                        last_modified: None,
                        metadata: None,
                    };

                    resources.push(resource_metadata);
                }

                Ok(resources)
            }
            _ => Err(anyhow!("Unsupported scheme: {}", scheme)),
        }
    }

    /// Delete a resource
    pub fn delete_resource(&mut self, uri: &str) -> Result<()> {
        let mut resource = self.get_resource(uri)?;
        resource.delete()
    }
}
