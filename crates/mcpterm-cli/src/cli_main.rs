use anyhow::Result;
use clap::Parser;
use mcp_core::{init_tracing, set_verbose_logging, Config};
use mcp_metrics::{LogDestination, MetricsDestination, MetricsRegistry};
use std::io::Write;
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
        // Process single prompt or input file
        if let Some(prompt) = cli.prompt {
            debug!("Processing single prompt");
            let _response = app.run(&prompt).await?;
            // Response is already printed in app.run

            // Add a deliberate delay for tool responses
            debug!("Waiting for any follow-up responses...");
            sleep(Duration::from_secs(3)).await;

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
        } else {
            debug!("No prompt or input file provided");
            eprintln!("Error: No prompt or input file provided");
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

        match app.run(input).await {
            Ok(_) => {
                // Add a short delay for tool responses in interactive mode
                sleep(Duration::from_millis(500)).await;

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