#[cfg(test)]
mod tests {
    use crate::mcp::resources::{AccessMode, FileResource, MemoryResource, Resource, ResourceType};
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_file_resource() -> anyhow::Result<()> {
        // Create a temporary directory for test
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path();

        // Create a test file
        let test_file_path = temp_path.join("test.txt");
        let mut file = File::create(&test_file_path)?;
        file.write_all(b"test content")?;

        // Create a file resource directly
        let resource = FileResource::new(&test_file_path, AccessMode::Read);

        // Check if resource exists
        assert!(resource.exists());

        // Read resource content
        let content = resource.read()?;
        assert_eq!(content, b"test content");

        // Check resource metadata
        let metadata = resource.metadata()?;
        assert_eq!(metadata.resource_type, ResourceType::File);
        assert_eq!(metadata.size, Some(12)); // "test content".len()

        // Create a new file resource
        let new_file_path = temp_path.join("new_file.txt");
        let mut new_resource = FileResource::new(&new_file_path, AccessMode::ReadWrite);

        // Write content to the new resource
        new_resource.write(b"new content")?;

        // Check if the file was created
        assert!(new_file_path.exists());

        // Read content from the new resource
        let new_content = new_resource.read()?;
        assert_eq!(new_content, b"new content");

        // Delete the new resource
        new_resource.delete()?;

        // Check if the file was deleted
        assert!(!new_file_path.exists());

        Ok(())
    }

    #[test]
    fn test_memory_resource() -> anyhow::Result<()> {
        // Create a memory resource directly
        let mut resource = MemoryResource::new("test".to_string(), AccessMode::ReadWrite);

        // Write content to the resource
        resource.write(b"test content")?;

        // Check resource metadata
        let metadata = resource.metadata()?;
        assert_eq!(metadata.uri, "memory://test");
        assert_eq!(metadata.size, Some(12)); // "test content".len()

        // Read resource content
        let content = resource.read()?;
        assert_eq!(content, b"test content");

        Ok(())
    }
}
