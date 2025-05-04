use mcp_core::logging::tracing::{init_tracing, get_log_level};
use tracing::{debug, error, info, trace, warn};

/// This example demonstrates how to use the different logging levels
/// with the tracing-based logging system.
/// 
/// Run with different log levels to see how the output changes:
/// 
/// ```
/// # Default info level
/// cargo run --example logging_levels
/// 
/// # Debug level
/// LOG_LEVEL=debug cargo run --example logging_levels
/// 
/// # Trace level
/// LOG_LEVEL=trace cargo run --example logging_levels
/// 
/// # Module-specific levels
/// LOG_LEVEL=info,example_module=trace cargo run --example logging_levels
/// ```

fn main() {
    // Initialize tracing
    let log_file = init_tracing();
    println!("Logs are being written to: {}", log_file.display());
    
    // Get the current log level
    let current_level = get_log_level();
    println!("Current log level: {:?}", current_level);
    
    // Print a message at each log level
    println!("\n--- Logging at different levels ---");
    trace!("This is a TRACE message: Very detailed information for debugging");
    debug!("This is a DEBUG message: Useful information for troubleshooting");
    info!("This is an INFO message: Important application events");
    warn!("This is a WARN message: Concerning issues that don't break functionality");
    error!("This is an ERROR message: Critical problems that prevent functionality");
    
    // Demonstrate module-specific logging
    println!("\n--- Module-specific logging ---");
    trace!(target: "example_module", "TRACE from example_module");
    debug!(target: "example_module", "DEBUG from example_module");
    info!(target: "example_module", "INFO from example_module");
    warn!(target: "example_module", "WARN from example_module");
    error!(target: "example_module", "ERROR from example_module");
    
    // Demonstrate structured logging
    println!("\n--- Structured logging ---");
    trace!(request_id = "req-123", path = "/api/users", "Processing API request");
    debug!(user_id = 42, action = "login", "User logged in");
    info!(
        status = "success", 
        duration_ms = 157, 
        "Operation completed successfully"
    );
    warn!(
        component = "database", 
        retries = 3, 
        "Connection timeout, retrying"
    );
    error!(
        error_code = 500, 
        message = "Internal server error", 
        "Request processing failed"
    );
    
    println!("\nCheck the log file at {} to see the full logs.", log_file.display());
    println!("Note: Only messages at or above the current log level ({:?}) will be shown.", current_level);
}