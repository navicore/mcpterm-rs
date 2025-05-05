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

### Installation

To install MCP locally from the source:

```bash
# Install from the app crate that contains both CLI and TUI
cargo install --path crates/app

# Force reinstallation if already installed
cargo install --path crates/app --force
```

This will install the binary to your `~/.cargo/bin` directory, which should be in your PATH.

### Running

Run the installed binary:

```bash
mcp
```

Or run directly from the source:

```bash
cargo run -p mcpterm-tui
```

For CLI mode:

```bash
cargo run -p mcpterm-cli
```

For interactive CLI mode with slash commands:

```bash
cargo run -p mcpterm-cli -- -I
```

### Local Slash Commands

The CLI supports local slash commands in interactive mode for debugging and inspecting MCP tools. These commands are processed directly by the application, not sent to the LLM:

```bash
/mcp list                # List all available tools
/mcp show <tool_id>      # Show details and JSON schema for a tool
/mcp help                # Show help for slash commands
```

Local slash commands give you direct access to tool information from the source of truth - your mcpterm implementation.

See [SLASH_COMMANDS.md](./docs/SLASH_COMMANDS.md) for more details.

## Development

Each crate has its own README, tests, and example usage in its directory. The implementation follows a test-driven approach.

### Current Status

- Full crate structure and dependencies implemented
- Core functionality implemented
- Comprehensive test coverage added
- End-to-end workflow functioning
- Tools implemented:
  - Shell commands
  - Filesystem operations (read/write/list)
  - Search tools (grep/find)
  - Diff and patch tools
  - Project navigation
  - Language analysis for Rust, JavaScript/TypeScript, and Python
  - Test runner for various frameworks (Rust, Jest, Mocha, Pytest, Unittest)

### Next Steps

1. Enhance error handling and user feedback
2. Add more language analyzers
3. Improve test coverage
4. Add documentation and examples

## Previous Implementation

The original implementation has been moved to `archives/p1` for reference.

## License

See the [LICENSE](LICENSE) file for details.