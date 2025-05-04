use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    mcpterm_cli::cli_main::main().await
}
