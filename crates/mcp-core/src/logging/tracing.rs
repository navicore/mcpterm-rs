use std::path::PathBuf;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt;

/// Stores the worker guard to keep the non-blocking appender alive
static mut APPENDER_GUARD: Option<WorkerGuard> = None;

/// Initialize the tracing system with proper log levels from environment variables
///
/// This will configure logging to:
/// 1. Send all logs to a file in /tmp/mcpterm.log
/// 2. Respect the LOG_LEVEL environment variable
/// 3. Use the default filter pattern RUST_LOG if LOG_LEVEL isn't set
/// 4. Automatically filter out noisy dependencies when using simple log levels
///
/// Examples of valid LOG_LEVEL values:
/// - "trace" - Show trace logs for our code, but limit dependencies to info level
/// - "info" - Show logs at info level and above (default)
/// - "mcp_llm=trace,mcp_core=debug" - Different levels for different modules
///   (requires LOG_LEVEL to be set with directives as shown)
///
/// Returns the log file path so it can be displayed to the user.
pub fn init_tracing() -> PathBuf {
    // Always use /tmp for log files
    let log_file = PathBuf::from("/tmp/mcpterm.log");

    // Create a rolling file appender to /tmp/mcpterm.log
    let file_appender = RollingFileAppender::new(Rotation::NEVER, "/tmp", "mcpterm.log");

    // Create a non-blocking writer for the file appender
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Store the guard to ensure the writer stays alive
    unsafe {
        APPENDER_GUARD = Some(guard);
    }

    // Get the raw environment variable value
    let raw_log_level = std::env::var("LOG_LEVEL")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info".to_string());

    // Check if we should show debug logging messages
    let show_debug = std::env::var("MCPTERM_LOG_DEBUG").is_ok();

    // If the user provided a complex directive with = or ,, use it as-is
    // Otherwise, create an intelligent filter that shows app logs at the requested level
    // but limits dependency noise
    let env_filter = if raw_log_level.contains('=') || raw_log_level.contains(',') {
        // User provided a custom filter, use it directly
        if show_debug {
            println!("Using custom logging directive: {}", raw_log_level);
        }
        raw_log_level
    } else {
        // Simple level - create a filter that limits dependency noise
        // This sets our crates to the requested level but keeps deps at info by default
        let mcp_filter = format!(
            "info,mcp_core={0},mcp_llm={0},mcp_metrics={0},mcp_resources={0},\
            mcp_runtime={0},mcp_tools={0},mcpterm_cli={0},mcpterm_tui={0},\
            mcp={0},app={0},h2=warn",
            raw_log_level
        );

        if show_debug {
            println!(
                "Using smart filter (app logs at {}, deps at info): {}",
                raw_log_level, mcp_filter
            );
        }
        mcp_filter
    };

    // Create the EnvFilter from our prepared filter string
    let filter = tracing_subscriber::EnvFilter::try_new(&env_filter).unwrap_or_else(|e| {
        if show_debug {
            eprintln!("Warning: Invalid filter directive: {}", e);
            eprintln!("Falling back to simple level: info");
        }
        tracing_subscriber::EnvFilter::new("info")
    });

    // Initialize the subscriber
    fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(non_blocking)
        .with_file(true)
        .with_line_number(true)
        .init();

    // Return the log file path
    log_file
}

/// Helper function to get the current log level from the environment variable
/// This is useful for conditional logging decisions in code
pub fn get_log_level() -> Level {
    match std::env::var("LOG_LEVEL").ok().as_deref() {
        Some("trace") => Level::TRACE,
        Some("debug") => Level::DEBUG,
        Some("info") | None => Level::INFO,
        Some("warn") => Level::WARN,
        Some("error") => Level::ERROR,
        _ => Level::INFO, // Default to INFO if the level is invalid
    }
}
