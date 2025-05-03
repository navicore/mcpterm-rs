use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::{AccessMode, Resource, ResourceMetadata, ResourceType};

/// File resource implementation
pub struct FileResource {
    /// File path
    path: PathBuf,

    /// Access mode
    mode: AccessMode,
}

impl FileResource {
    /// Create a new file resource
    pub fn new<P: AsRef<Path>>(path: P, mode: AccessMode) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            mode,
        }
    }

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Resource for FileResource {
    fn metadata(&self) -> Result<ResourceMetadata> {
        // Get file metadata
        let metadata = fs::metadata(&self.path).context(format!(
            "Failed to get metadata for {}",
            self.path.display()
        ))?;

        // Determine resource type
        let resource_type = if metadata.is_dir() {
            ResourceType::Directory
        } else {
            ResourceType::File
        };

        // Create resource URI
        let uri = format!("file://{}", self.path.display());

        // Create resource metadata
        Ok(ResourceMetadata {
            uri,
            resource_type,
            content_type: None, // Could infer content type from file extension
            size: Some(metadata.len()),
            last_modified: metadata
                .modified()
                .ok()
                .map(|time| time.elapsed().unwrap_or_default().as_secs().to_string()),
            metadata: None,
        })
    }

    fn read(&self) -> Result<Vec<u8>> {
        // Check access mode
        if self.mode == AccessMode::Write {
            anyhow::bail!("Resource is write-only");
        }

        // Check if file exists
        if !self.exists() {
            anyhow::bail!("File not found: {}", self.path.display());
        }

        // Read file content
        let mut file = File::open(&self.path)
            .context(format!("Failed to open file: {}", self.path.display()))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .context(format!("Failed to read file: {}", self.path.display()))?;

        Ok(buffer)
    }

    fn write(&mut self, content: &[u8]) -> Result<()> {
        // Check access mode
        if self.mode == AccessMode::Read {
            anyhow::bail!("Resource is read-only");
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = self.path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .context(format!("Failed to create directory: {}", parent.display()))?;
            }
        }

        // Write file content
        let mut file = File::create(&self.path)
            .context(format!("Failed to create file: {}", self.path.display()))?;

        file.write_all(content)
            .context(format!("Failed to write to file: {}", self.path.display()))?;

        Ok(())
    }

    fn exists(&self) -> bool {
        self.path.exists()
    }

    fn access_mode(&self) -> AccessMode {
        self.mode
    }

    fn delete(&mut self) -> Result<()> {
        if self.path.is_dir() {
            fs::remove_dir_all(&self.path).context(format!(
                "Failed to delete directory: {}",
                self.path.display()
            ))
        } else {
            fs::remove_file(&self.path)
                .context(format!("Failed to delete file: {}", self.path.display()))
        }
    }
}
