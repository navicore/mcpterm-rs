#[cfg(test)]
mod tests {
    use mcp_resources::{AccessMode, ResourceManager};

    #[test]
    fn test_register_file() {
        let manager = ResourceManager::new("/tmp");
        let uri = manager.register_file("test.txt", AccessMode::ReadOnly);
        assert!(uri.starts_with("file://"));
    }
}
