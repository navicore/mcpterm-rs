use anyhow::Result;
use clap::Parser;
use mcpterm_cli::{CliApp, CliConfig};
use mcp_metrics::{LogDestination, MetricsRegistry, MetricsDestination};
use mcp_core::{init_debug_log, debug_log, set_verbose_logging, Config};
use std::time::Duration;
use std::io::Write;
use std::path::PathBuf;
use tokio::time::sleep;

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
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
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();
    
    // Initialize our custom logging
    init_debug_log()?;
    debug_log("Starting mcpterm-cli");
    
    // Set verbose logging if requested
    if cli.verbose {
        set_verbose_logging(true);
        debug_log("Verbose logging enabled");
    }
    
    // Setup metrics reporting
    let log_destination = LogDestination;
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(300)).await; // Report every 5 minutes
            let report = MetricsRegistry::global().generate_report();
            if let Err(e) = log_destination.send_report(&report) {
                debug_log(&format!("Error sending metrics report: {}", e));
            }
        }
    });
    
    // Load configuration
    debug_log("Loading configuration");
    let config = match Config::load(cli.config.as_ref(), Some(&cli.model), cli.region.as_deref()) {
        Ok(config) => {
            debug_log("Configuration loaded successfully");
            config
        },
        Err(e) => {
            debug_log(&format!("Error loading config: {}", e));
            eprintln!("Warning: Could not load configuration: {}", e);
            // Create a default config
            Config::default()
        }
    };
    
    // Get the active model
    let model_config = config.get_active_model().unwrap_or_else(|| {
        debug_log("No active model found in config, using default");
        config.model_settings.models.first().unwrap().clone()
    });
    
    debug_log(&format!("Using model: {}", model_config.model_id));
    
    // Create CLI configuration
    let cli_config = CliConfig {
        model: model_config.model_id.clone(),
        use_mcp: cli.mcp || config.mcp.enabled,
        region: Some(config.aws.region.clone()),
        streaming: !cli.no_streaming,
    };
    
    debug_log(&format!("CLI config: {:#?}", cli_config));
    
    // Create CLI application with configuration
    let mut app = CliApp::new().with_config(cli_config);
    
    // Initialize the application
    debug_log("Initializing CLI application");
    if let Err(e) = app.initialize().await {
        debug_log(&format!("Failed to initialize app: {}", e));
        return Err(e);
    }
    
    // Process in interactive or batch mode
    if cli.interactive {
        debug_log("Starting interactive mode");
        run_interactive_mode(&mut app).await?;
    } else {
        // Process single prompt or input file
        if let Some(prompt) = cli.prompt {
            debug_log("Processing single prompt");
            let _response = app.run(&prompt).await?;
            // Response is already printed in app.run
        } else if let Some(input_file) = cli.input {
            debug_log(&format!("Processing input file: {}", input_file));
            process_input_file(&mut app, &input_file, cli.output).await?;
        } else {
            debug_log("No prompt or input file provided");
            eprintln!("Error: No prompt or input file provided");
            std::process::exit(1);
        }
    }
    
    debug_log("Exiting mcpterm-cli");
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
            Ok(_) => {}, // Response is already printed in app.run
            Err(e) => eprintln!("Error: {}", e),
        }
        
        println!(); // Add a blank line for readability
    }
    
    println!("Chat session ended.");
    Ok(())
}

// Process prompts from an input file
async fn process_input_file(app: &mut CliApp, input_file: &str, output_file: Option<String>) -> Result<()> {
    // Read prompts from file (one per line)
    let input_content = std::fs::read_to_string(input_file)?;
    let prompts: Vec<&str> = input_content.lines()
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
            },
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