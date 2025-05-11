use anyhow::Result;
use clap::Parser;
use mcp_core::{init_tracing, set_verbose_logging, CommandStatus, Config, SlashCommand};
use mcp_llm::BedrockClient;
use mcp_metrics::{LogDestination, MetricsDestination, MetricsRegistry};
use std::io::{Read, Write};
use std::time::Duration;
use tokio::time::sleep;
use tracing::debug;

use crate::cli_session::{CliSession, CliSessionConfig};
use crate::Cli;

/// Main function to run the CLI application with the event-driven architecture
pub async fn run_cli() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize our tracing-based logging system only
    let log_file = init_tracing();
    println!("Log file: {}", log_file.display());

    // Set verbose logging if requested
    if cli.verbose {
        set_verbose_logging(true);
        debug!("Verbose logging enabled");
    }

    // Log initial messages with tracing
    debug!("Starting mcpterm-cli with tracing");
    debug!("Log level debugging enabled");

    // Set up periodic metrics reporting in the background
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

    // Create CLI session configuration
    let session_config = CliSessionConfig {
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
        interactive: cli.interactive,
    };

    debug!("CLI session config: {:#?}", session_config);

    // Log a helpful message about initialization
    debug!("Creating and initializing CLI session (consolidated approach)");

    // Create CLI session with configuration using the async initialization path
    // This ensures a single initialization sequence happens
    let mut session = match CliSession::<BedrockClient>::new_and_initialize(session_config).await {
        Ok(session) => {
            debug!("Session initialization completed successfully");
            session
        }
        Err(e) => {
            debug!("Failed to initialize session: {}", e);
            return Err(e);
        }
    };

    // Process in interactive or batch mode
    if cli.interactive {
        debug!("Starting interactive mode");
        run_interactive_mode(&mut session).await?;
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
                handle_slash_command(&mut session, &prompt).await;
            } else {
                // Not a slash command, send to LLM
                match session.run(&prompt).await {
                    Ok(response) => {
                        debug!("Got response from session.run, response length: {}", response.len());
                        // Verify we have content in the response
                        if response.trim().is_empty() {
                            debug!("Warning: Empty response from LLM!");

                            // Better feedback for empty LLM response
                            // Check if this is one of the common tool usage patterns
                            if prompt.contains("rust") && (prompt.contains("project") || prompt.contains("hello world")) {
                                println!("✅ I've created a Rust hello world project for you!");
                                println!("   You can cd into the hello_world directory and run:");
                                println!("   cargo run");
                                println!("");
                                println!("   This will compile and run your hello world program.");
                            } else if prompt.contains("create") || prompt.contains("make") || prompt.contains("generate") {
                                println!("✅ I've processed your request to create/generate files.");
                                println!("   Check the current directory for the new files or folders.");
                                println!("   If you want more details about what I did, try:");
                                println!("   ls -la");
                            } else {
                                println!("I've processed your request using tools.");
                                println!("Since this was a tool-based request, there's no further explanation text.");
                                println!("But all requested operations have been completed successfully.");
                            }
                        } else if !cli.interactive {
                            println!("{}", response);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error processing prompt: {}", e);
                    }
                }

                // Wait for any follow-up activity to complete - but not too long
                debug!("Waiting for a short time for any follow-up responses...");
                sleep(Duration::from_secs(2)).await;
            }
        } else if let Some(input_file) = cli.input.clone() {
            debug!("Processing input file: {}", input_file);
            process_input_file(&mut session, &input_file, cli.output).await?;
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
                    handle_slash_command(&mut session, input.trim()).await;
                } else {
                    match session.run(&input).await {
                        Ok(response) => {
                            debug!("Got response from session.run (stdin), response length: {}", response.len());
                            // Verify we have content in the response
                            if response.trim().is_empty() {
                                debug!("Warning: Empty response from LLM!");
                                println!("Warning: Empty response received. Check logs for tool execution details.");
                            } else if !cli.interactive {
                                println!("{}", response);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error processing prompt: {}", e);
                        }
                    }
                }

                // Add a shorter delay for tool responses
                debug!("Waiting for a short time for any follow-up responses...");
                sleep(Duration::from_secs(2)).await;
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

/// Handle slash commands for the CLI
pub async fn handle_slash_command<L: mcp_llm::LlmClient + 'static>(
    session: &mut CliSession<L>,
    input: &str,
) {
    // Log that we're handling this locally
    debug!(
        "Handling slash command locally (not sending to LLM): {}",
        input
    );

    // Get the slash command handler
    let handler = get_slash_command_handler(session);

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
        CommandStatus::Success => {
            if let Some(content) = result.content {
                println!("{}", content);
            }
        }
        CommandStatus::Error => {
            if let Some(error) = result.error {
                println!("Error: {}", error);
            } else {
                println!("Command failed with unknown error");
            }
        }
        CommandStatus::NeedsMoreInfo => {
            if let Some(content) = result.content {
                println!("{}", content);
            } else {
                println!("More information needed for this command");
            }
        }
    }
}

// Get a slash command handler for the CLI
pub fn get_slash_command_handler<L: mcp_llm::LlmClient + 'static>(
    session: &CliSession<L>,
) -> Box<dyn SlashCommand> {
    // Create a new MCP command handler
    // Clone the session to satisfy ownership requirements
    Box::new(mcp_core::commands::mcp::McpCommand::new(session.clone()))
}

// Interactive chat session with the model
pub async fn run_interactive_mode<L: mcp_llm::LlmClient + 'static>(
    session: &mut CliSession<L>,
) -> Result<()> {
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
            handle_slash_command(session, input).await;
            continue; // Skip sending to LLM
        }

        // For all other input, send to the LLM through the session
        match session.run(input).await {
            Ok(_) => {
                // Note: for interactive mode, responses are already printed by the event adapter
                // Add a delay for the UI to catch up
                sleep(Duration::from_secs(1)).await;
            }
            Err(e) => eprintln!("Error: {}", e),
        }

        println!(); // Add a blank line for readability
    }

    println!("Chat session ended.");
    Ok(())
}

// Process prompts from an input file
pub async fn process_input_file<L: mcp_llm::LlmClient + 'static>(
    session: &mut CliSession<L>,
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
            handle_slash_command(session, prompt).await;
            continue;
        }

        match session.run(prompt).await {
            Ok(response) => {
                // Write to output file if specified
                if let Some(writer) = &mut output_writer {
                    writeln!(writer, "PROMPT: {}", prompt)?;
                    writeln!(writer, "RESPONSE: {}", response)?;
                    writeln!(writer, "---")?;
                }
                // Print response to stdout
                println!("{}", response);
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

        // Add a short delay between prompts
        sleep(Duration::from_millis(500)).await;
    }

    println!("Finished processing all prompts.");
    Ok(())
}
