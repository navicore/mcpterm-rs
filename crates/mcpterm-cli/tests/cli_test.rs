#[cfg(test)]
mod tests {
    use mcpterm_cli::CliApp;
    
    #[tokio::test]
    async fn test_cli_app() {
        let mut app = CliApp::new();
        let result = app.run("test prompt").await.unwrap();
        assert!(result.contains("test prompt"));
    }
}