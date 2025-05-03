# MCPTerm CLI

This crate provides a command-line interface for the Model Context Protocol (MCP) terminal application, allowing for non-interactive usage.

## Features

- Single-prompt interactions
- File input/output
- Tool execution from command line
- Batch processing
- Configuration via command line flags or config file

## Usage

```bash
# Simple query
mcpterm-cli "What is the capital of France?"

# With file input
mcpterm-cli --input questions.txt --output answers.txt

# Specify model
mcpterm-cli --model anthropic.claude-3-sonnet-20240229-v1:0 "Write a function to sort a list"

# Enable MCP tools
mcpterm-cli --mcp "Show me the contents of the current directory"

# Specify AWS region
mcpterm-cli --region us-west-2 "What files are in this project?"
```