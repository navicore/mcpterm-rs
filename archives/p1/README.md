[![Rust CI](https://github.com/navicore/mcpterm-rs/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/navicore/mcpterm-rs/actions/workflows/rust-ci.yml)
# mcpterm

A terminal-based AI coding assistant using the MCP protocol with a focus on a clean, efficient UI.

## Features

- Terminal-based user interface using ratatui
- Vi-style keybindings (normal/insert modes) with compound commands
- Connects to Anthropic Claude models via AWS Bedrock
- Split layout with message history and persistent input area
- Multi-line input support

## Requirements

- Rust toolchain (cargo, rustc)
- AWS credentials configured for Bedrock access

### Setting up AWS Credentials for Bedrock

To use AWS Bedrock with mcpterm, you need to set up AWS credentials:

1. **Create AWS IAM User with Bedrock Access**:
   - Log in to the AWS Management Console
   - Go to IAM (Identity and Access Management)
   - Create a new user with programmatic access
   - Attach the `AmazonBedrockFullAccess` policy (or a more limited policy for production)

2. **Configure AWS Credentials Locally**:
   - Create a credentials file at `~/.aws/credentials` with:
     ```
     [default]
     aws_access_key_id = YOUR_ACCESS_KEY_ID
     aws_secret_access_key = YOUR_SECRET_ACCESS_KEY
     ```
   - Create a config file at `~/.aws/config` with:
     ```
     [default]
     region = us-east-1
     output = json
     ```
   - Alternatively, you can run `aws configure` to set up these files

## Installation

Clone the repository and build with cargo:

```bash
git clone https://github.com/yourusername/mcpterm.git
cd mcpterm
cargo build --release
```

The binary will be available at `./target/release/mcpterm`.

## Usage

The application features an advanced terminal UI with:
- Top 70% of screen shows scrollable message history
- Bottom 30% is a persistent input area with vi navigation and editing
- Full vi editing support including compound commands like 'dw', 'dd'

```bash
# Run with default settings (using stub agent)
cargo run
# or
./target/release/mcpterm

# Run with Bedrock integration
./run-with-bedrock.sh
# or 
USE_BEDROCK=1 cargo run
```

### Command Line Options

The application supports the following options:
```bash
# Specify AWS region
--region us-west-2

# Use a specific model
--model-id anthropic.claude-3-sonnet-20240229-v1:0

# Use a custom config file
--config /path/to/config.json
```

## Keybindings

The interface has two focus areas (Input and Messages) that you can toggle between with `Tab`.

### Global Keybindings
- `Tab`: Switch focus between Input and Messages areas
- `q`: Quit application from any mode

### Input Area Keybindings

#### Normal Mode 
- **Navigation**
  - `h/l`: Move cursor left/right
  - `0/$`: Move to beginning/end of line
  - `w`: Move forward one word
  - `b`: Move backward one word

- **Entering Insert Mode**
  - `i`: Enter insert mode at cursor
  - `a`: Enter insert mode after cursor
  - `A`: Enter insert mode at end of line
  - `I`: Enter insert mode at beginning of line

- **Simple Editing**
  - `x`: Delete character under cursor
  - `D`: Delete from cursor to end of line
  - `C`: Change to end of line (delete + insert mode)
  - `c`: Clear entire input
  - `Enter`: Submit message
  
- **Compound Commands** (Type first letter, then motion)
  - `dw`: Delete word
  - `db`: Delete backward to start of word
  - `d$`: Delete to end of line
  - `d0`: Delete to beginning of line
  - `dd`: Delete entire line

#### Insert Mode
- `Esc`: Return to normal mode (stays in input area)
- Standard typing functionality
- `Enter`: Submit message
- `Shift+Enter`: Add newline (for multi-line input)
- `Left/Right/Home/End`: Move cursor
- `Backspace/Delete`: Delete characters

### Message History Area Keybindings
- `j`: Scroll down message history
- `k`: Scroll up message history
- `G`: Go to bottom of message history
- `g`: Go to top of message history
- `Ctrl+d`: Scroll down multiple lines
- `Ctrl+u`: Scroll up multiple lines
- `Enter` or `i`: Return to input area (in insert mode)

## Configuration

mcpterm uses a JSON configuration file located at `~/.config/mcpterm/config.json` by default. You can specify a different file with the `--config` flag.

Example configuration:

```json
{
  "aws": {
    "region": "us-east-1",
    "profile": "default"
  },
  "model": {
    "model_id": "anthropic.claude-3-sonnet-20240229-v1:0",
    "max_tokens": 4096,
    "temperature": 0.7
  }
}
```

### Environment Variables

You can also configure mcpterm using environment variables:

- `USE_BEDROCK`: Set to any value to enable Bedrock integration
- `BEDROCK_MODEL_ID`: Specify the model ID to use (defaults to `anthropic.claude-3-sonnet-20240229-v1:0`)
- `AWS_REGION`: Override the AWS region
- `AWS_PROFILE`: Use a specific AWS profile

### Available Bedrock Models

Anthropic Claude models available on AWS Bedrock:

- `anthropic.claude-3-sonnet-20240229-v1:0` - Most capable Claude model, good balance of intelligence and speed
- `anthropic.claude-3-haiku-20240307-v1:0` - Fastest and most cost-effective Claude model
- `anthropic.claude-3-opus-20240229-v1:0` - Most powerful Claude model for complex tasks
- `anthropic.claude-v2:1` - Previous generation Claude model
- `anthropic.claude-instant-v1` - Fast previous generation model

## License

MIT

## Acknowledgments

- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI library
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation library
- [aws-sdk-rust](https://github.com/awslabs/aws-sdk-rust) - AWS SDK for Rust
