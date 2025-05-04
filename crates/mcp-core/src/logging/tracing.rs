use std::path::PathBuf;
use tracing_subscriber::{fmt, filter::LevelFilter};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_appender::non_blocking::WorkerGuard;
use tracing::Level;

/// Stores the worker guard to keep the non-blocking appender alive
static mut APPENDER_GUARD: Option<WorkerGuard> = None;

/// Initialize the tracing system with proper log levels from environment variables
///
/// This will configure logging to:
/// 1. Send all logs to a file in /tmp/mcpterm.log
/// 2. Respect the LOG_LEVEL environment variable
/// 3. Use the default filter pattern RUST_LOG if LOG_LEVEL isn't set
///
/// Examples of valid LOG_LEVEL values:
/// - "trace" - Show all logs at trace level and above
/// - "info" - Show logs at info level and above (default)
/// - "mcp_llm=trace,mcp_core=debug" - Different levels for different modules 
///   (requires LOG_LEVEL to be set with directives as shown)
///
/// Returns the log file path so it can be displayed to the user.
pub fn init_tracing() -> PathBuf {
    // Always use /tmp for log files
    let log_file = PathBuf::from("/tmp/mcpterm.log");
    
    // Create a rolling file appender to /tmp/mcpterm.log 
    let file_appender = RollingFileAppender::new(
        Rotation::NEVER,
        "/tmp",
        "mcpterm.log",
    );
    
    // Create a non-blocking writer for the file appender
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    
    // Store the guard to ensure the writer stays alive
    unsafe {
        APPENDER_GUARD = Some(guard);
    }

    // Check for LOG_LEVEL or RUST_LOG environment variables
    let log_level_str = std::env::var("LOG_LEVEL")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info".to_string());
    
    // Check if this is a custom directive format (contains = or ,)
    let is_complex_directive = log_level_str.contains('=') || log_level_str.contains(',');
    
    // Initialize subscriber based on whether we have complex directives
    if is_complex_directive {
        // Use EnvFilter for complex directives
        let env_filter = 
            tracing_subscriber::EnvFilter::try_from_env("LOG_LEVEL")
                .or_else(|_| tracing_subscriber::EnvFilter::try_from_env("RUST_LOG"))
                .unwrap_or_else(|_| {
                    // Default to show info, warn, and error logs
                    tracing_subscriber::EnvFilter::new("info")
                });
            
        fmt::Subscriber::builder()
            .with_env_filter(env_filter)
            .with_ansi(false) // Disable ANSI color codes in files
            .with_writer(non_blocking)
            .with_file(true)   // Include file and line information
            .with_line_number(true)
            .init();
    } else {
        // Use simpler LevelFilter for basic log levels
        let level_filter = match log_level_str.to_lowercase().as_str() {
            "trace" => LevelFilter::TRACE,
            "debug" => LevelFilter::DEBUG,
            "info" => LevelFilter::INFO,
            "warn" => LevelFilter::WARN,
            "error" => LevelFilter::ERROR,
            _ => LevelFilter::INFO,
        };
        
        fmt::Subscriber::builder()
            .with_max_level(level_filter)
            .with_ansi(false) // Disable ANSI color codes in files
            .with_writer(non_blocking)
            .with_file(true)   // Include file and line information
            .with_line_number(true)
            .init();
    }

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