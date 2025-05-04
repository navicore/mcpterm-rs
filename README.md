# MCPTerm-RS

A terminal-based client for the Model Context Protocol (MCP), written in Rust.

## Project Structure

This project follows a modular architecture to ensure separation of concerns and testability. It uses a workspace-based Cargo structure with the following crates:

- `mcp-core`: Core protocol definitions and conversation context management
- `mcp-resources`: Resource management abstraction (files, memory)
- `mcp-tools`: Tool implementations (shell, filesystem, search)
- `mcp-runtime`: Event bus and session management
- `mcp-llm`: LLM provider adapters (Bedrock, Anthropic, etc.)
- `mcpterm-tui`: Terminal user interface using Ratatui
- `mcpterm-cli`: Command-line interface for batch operations

## Architecture

The project uses a Staged Event-Driven Architecture (SEDA) approach to ensure a responsive UI when working with potentially slow LLM interactions. See [ARCHITECTURE.md](./docs/ARCHITECTURE.md) for more details.

## Getting Started

### Requirements

- Rust (stable, 2021 edition)
- AWS CLI configured (for Bedrock access, if using)

### Building

```bash
cargo build
```

### Running

```bash
cargo run -p mcpterm-tui
```

## Development

Each crate has its own README, tests, and example usage in its directory. The implementation follows a test-driven approach.

### Current Status

- Basic crate structure and dependencies set up
- Placeholder implementations in place
- Core types defined
- Need to implement actual functionality

### Next Steps

1. Start implementing core components
2. Add comprehensive test coverage
3. Implement a minimal end-to-end flow
4. Refine and expand functionality

## Previous Implementation

The original implementation has been moved to `archives/p1` for reference.

## License

See the [LICENSE](LICENSE) file for details.