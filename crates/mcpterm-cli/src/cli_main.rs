use anyhow::Result;
use clap::Parser;
use mcp_core::{init_tracing, set_verbose_logging, Config};
use mcp_metrics::{LogDestination, MetricsDestination, MetricsRegistry};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, trace};

use crate::{CliApp, CliConfig};

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Cli {
    /// Prompt to send to the model
    #[clap(index = 1)]
    prompt: Option<String>,

    /// Input file containing prompts (one per line)
    #[clap(long, short, value_name = "FILE")]
    input: Option<String>,

    /// Output file for responses
    #[clap(long, short = 'o', value_name = "FILE")]
    output: Option<String>,

    /// LLM model to use
    #[clap(long, default_value = "us.anthropic.claude-3-7-sonnet-20250219-v1:0")]
    model: String,

    /// Enable MCP protocol
    #[clap(long)]
    mcp: bool,

    /// AWS region for Bedrock
    #[clap(long)]
    region: Option<String>,

    /// Disable streaming responses
    #[clap(long)]
    no_streaming: bool,

    /// Interactive mode (chat with the model)
    #[clap(long, short = 'I')]
    interactive: bool,

    /// Path to config file
    #[clap(long, short)]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[clap(long)]
    verbose: bool,

    /// Enable tool execution
    #[clap(long)]
    tools: bool,

    /// Disable tool execution
    #[clap(long)]
    no_tools: bool,

    /// Skip confirmation for tool execution
    #[clap(long)]
    no_tool_confirmation: bool,

    /// Automatically approve all tool executions
    #[clap(long, short = 'y')]
    yes: bool,
}

/// Handle slash commands for the CLI
async fn handle_slash_command(app: &mut CliApp, input: &str) {
    // Log that we're handling this locally
    debug!(
        "Handling slash command locally (not sending to LLM): {}",
        input
    );

    // Get the slash command handler
    let handler = app.get_slash_command_handler();

    // Parse the command
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    // Extract the command name without the slash
    let command_name = parts[0].trim_start_matches('/');

    // Check if this handler can process this command
    if command_name != handler.name() {
        println!("Unknown command: /{}", command_name);
        println!("Currently supported commands: /mcp");
        return;
    }

    // Execute the command with args
    let args = &parts[1..];
    let result = handler.execute(args);

    // Display the result
    match result.status {
        mcp_core::CommandStatus::Success => {
            if let Some(content) = result.content {
                println!("{}", content);
            }
        }
        mcp_core::CommandStatus::Error => {
            if let Some(error) = result.error {
                println!("Error: {}", error);
            } else {
                println!("Command failed with unknown error");
            }
        }
        mcp_core::CommandStatus::NeedsMoreInfo => {
            if let Some(content) = result.content {
                println!("{}", content);
            } else {
                println!("More information needed for this command");
            }
        }
    }
}

pub async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize our tracing-based logging system only
    let log_file = init_tracing();
    println!("Log file: {}", log_file.display());

    // Set verbose logging if requested
    if cli.verbose {
        set_verbose_logging(true);
        // Use tracing for logging instead of the old system
        debug!("Verbose logging enabled");
    }

    // Log initial messages with tracing
    debug!("Starting mcpterm-cli with tracing");
    debug!("Log level debugging enabled");
    trace!("Log level tracing enabled - will show detailed API requests/responses");

    // In CLI we don't need periodic reporting since most runs are short-lived
    // Instead, we'll log once at the end of execution
    // But we'll keep a background task just in case a CLI session runs for a long time
    let log_destination_bg = LogDestination;
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(300)).await; // Report every 5 minutes
            let report = MetricsRegistry::global().generate_report();
            if let Err(e) = log_destination_bg.send_report(&report) {
                debug!("Error sending periodic metrics report: {}", e);
            }
        }
    });

    // Load configuration
    debug!("Loading configuration");
    let config = match Config::load(cli.config.as_ref(), Some(&cli.model), cli.region.as_deref()) {
        Ok(config) => {
            debug!("Configuration loaded successfully");
            config
        }
        Err(e) => {
            debug!("Error loading config: {}", e);
            eprintln!("Warning: Could not load configuration: {}", e);
            // Create a default config
            Config::default()
        }
    };

    // Get the active model
    let model_config = config.get_active_model().unwrap_or_else(|| {
        debug!("No active model found in config, using default");
        config.model_settings.models.first().unwrap().clone()
    });

    debug!("Using model: {}", model_config.model_id);

    // Check if stdin input is being used
    let using_stdin_input = std::env::var("MCP_STDIN_INPUT").is_ok();

    // If using stdin with piped input, provide appropriate warnings
    if using_stdin_input {
        if cli.yes || cli.no_tool_confirmation {
            println!("Warning: Reading from stdin with auto-approval enabled.");
            println!("Any tool commands from the LLM will be executed without confirmation.");
        } else {
            println!("Warning: Reading from stdin. Tool executions will require confirmation.");
            println!("Use --yes to automatically approve tool executions when using pipes.");
            println!("This may cause prompts to hang waiting for approval.");
        }
    }

    // Create CLI configuration
    let cli_config = CliConfig {
        model: model_config.model_id.clone(),
        use_mcp: cli.mcp || config.mcp.enabled,
        region: Some(config.aws.region.clone()),
        streaming: !cli.no_streaming,
        enable_tools: if cli.no_tools {
            false
        } else {
            match cli.tools {
                true => true,
                false => {
                    true // Default to enabled
                }
            }
        },
        require_tool_confirmation: !cli.no_tool_confirmation,
        auto_approve_tools: cli.yes,
    };

    debug!("CLI config: {:#?}", cli_config);

    // Create CLI application with configuration
    let mut app = CliApp::new().with_config(cli_config);

    // Initialize the application
    debug!("Initializing CLI application");
    if let Err(e) = app.initialize().await {
        debug!("Failed to initialize app: {}", e);
        return Err(e);
    }

    // Process in interactive or batch mode
    if cli.interactive {
        debug!("Starting interactive mode");
        run_interactive_mode(&mut app).await?;
    } else {
        // Process input according to the following hierarchy:
        // 1. Command-line prompt
        // 2. Input file
        // 3. Piped stdin
        // 4. Error if none of the above
        if let Some(prompt) = cli.prompt {
            debug!("Processing single prompt from command line argument");

            // Check if this is a slash command
            if prompt.starts_with('/') {
                debug!("Handling slash command: {}", prompt);
                handle_slash_command(&mut app, &prompt).await;
            } else {
                // Not a slash command, send to LLM
                let _response = app.run(&prompt).await?;
                // Response is already printed in app.run

                // Wait for any follow-up responses after tool execution
                debug!("Waiting for any follow-up responses...");
                
                // First wait for a longer time to give the LLM a chance to respond
                sleep(Duration::from_secs(5)).await;
                
                // Check if there are any recent tool messages that might need follow-up
                let has_recent_tools = app.has_recent_tool_messages();
                
                if has_recent_tools {
                    debug!("Found recent tool executions, waiting longer for follow-up...");
                    // If we've executed tools recently, wait longer for the LLM to process results
                    sleep(Duration::from_secs(15)).await;
                }
            }

            // Add some diagnostic logs to help debug tool response issues
            debug!(
                "Context size after processing: {} messages",
                app.debug_context_size()
            );
            debug!("Last 3 message roles: {}", app.debug_last_message_roles(3));
            debug!("Processing complete");
        } else if let Some(input_file) = cli.input {
            debug!("Processing input file: {}", input_file);
            process_input_file(&mut app, &input_file, cli.output).await?;
        } else if std::env::var("MCP_STDIN_INPUT").is_ok() {
            // Read from stdin
            debug!("Reading prompt from stdin");
            println!("Reading prompt from stdin...");
            let mut input = String::new();
            // Read directly from stdin
            std::io::stdin().read_to_string(&mut input)?;

            if !input.trim().is_empty() {
                println!("Processing prompt ({} characters)...", input.len());
                debug!("Processing prompt from stdin, length: {}", input.len());

                // Check if this is a slash command
                if input.trim().starts_with('/') {
                    debug!("Handling slash command from stdin: {}", input);
                    handle_slash_command(&mut app, input.trim()).await;
                } else {
                    let _response = app.run(&input).await?;
                }

                // Add a deliberate delay for tool responses
                debug!("Waiting for any follow-up responses...");
                sleep(Duration::from_secs(5)).await;

                debug!(
                    "Context size after processing: {} messages",
                    app.debug_context_size()
                );
                debug!("Last 3 message roles: {}", app.debug_last_message_roles(3));
                debug!("Processing complete");
            } else {
                debug!("Empty input from stdin");
                eprintln!("Error: Empty input from stdin");
                std::process::exit(1);
            }
        } else {
            debug!("No prompt, input file, or stdin input provided");
            eprintln!("Error: No prompt or input provided. Use a command line argument, --input file, or pipe content to stdin.");
            std::process::exit(1);
        }
    }

    // Log metrics report at info level before exiting
    debug!("Generating metrics summary for this CLI execution");
    let log_destination = LogDestination;
    let report = MetricsRegistry::global().generate_report();
    if let Err(e) = log_destination.send_report(&report) {
        debug!("Error sending final metrics report: {}", e);
    }

    debug!("Exiting mcpterm-cli");
    Ok(())
}

// Interactive chat session with the model
async fn run_interactive_mode(app: &mut CliApp) -> Result<()> {
    println!("Starting interactive chat session. Type 'exit' or 'quit' to end.");
    println!("Type your messages and press Enter to send.");

    loop {
        print!("> ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            break;
        }

        // Handle any slash commands locally
        if input.starts_with('/') {
            // Process these commands locally instead of sending to the LLM
            handle_slash_command(app, input).await;
            continue; // Skip sending to LLM
        }

        // For all other input, send to the LLM
        match app.run(input).await {
            Ok(_) => {
                // Add a delay for tool responses in interactive mode
                sleep(Duration::from_secs(3)).await;

                // Log context size and roles for debugging
                debug!(
                    "Context size after command: {} messages",
                    app.debug_context_size()
                );
                debug!("Last 3 message roles: {}", app.debug_last_message_roles(3));
            }
            Err(e) => eprintln!("Error: {}", e),
        }

        println!(); // Add a blank line for readability
    }

    println!("Chat session ended.");
    Ok(())
}

// Process prompts from an input file
async fn process_input_file(
    app: &mut CliApp,
    input_file: &str,
    output_file: Option<String>,
) -> Result<()> {
    // Read prompts from file (one per line)
    let input_content = std::fs::read_to_string(input_file)?;
    let prompts: Vec<&str> = input_content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    println!("Processing {} prompts from {}", prompts.len(), input_file);

    // Prepare output file if specified
    let mut output_writer = if let Some(output_path) = output_file {
        Some(std::fs::File::create(output_path)?)
    } else {
        None
    };

    // Process each prompt
    for (i, prompt) in prompts.iter().enumerate() {
        println!("Processing prompt {} of {}", i + 1, prompts.len());

        // Check if this is a slash command
        if prompt.starts_with('/') {
            println!("Handling slash command: {}", prompt);
            handle_slash_command(app, prompt).await;
            continue;
        }

        match app.run(prompt).await {
            Ok(response) => {
                // Write to output file if specified
                if let Some(writer) = &mut output_writer {
                    writeln!(writer, "PROMPT: {}", prompt)?;
                    writeln!(writer, "RESPONSE: {}", response)?;
                    writeln!(writer, "---")?;
                }
            }
            Err(e) => {
                eprintln!("Error processing prompt {}: {}", i + 1, e);
                if let Some(writer) = &mut output_writer {
                    writeln!(writer, "PROMPT: {}", prompt)?;
                    writeln!(writer, "ERROR: {}", e)?;
                    writeln!(writer, "---")?;
                }
            }
        }
    }

    println!("Finished processing all prompts.");
    Ok(())
}
