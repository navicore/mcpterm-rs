use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

// New tracing-based logging system
pub mod tracing;
pub use self::tracing::*;

// Use OnceLock for the log file path to ensure thread-safe initialization
static DEBUG_LOG_FILE: OnceLock<PathBuf> = OnceLock::new();
static FALLBACK_LOG_FILE: OnceLock<PathBuf> = OnceLock::new();

// Use atomic bool for faster access without locks
static LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);
static VERBOSE_LOGGING: AtomicBool = AtomicBool::new(false);

/// Initialize the debug log file
pub fn init_debug_log() -> std::io::Result<()> {
    // Create consistent log files in /tmp regardless of platform
    // This is more idiomatic for Unix systems and easier to find
    let log_path = PathBuf::from("/tmp/mcpterm-debug.log");
    let fallback_path = PathBuf::from("/tmp/mcpterm-fallback.log");

    // Print the log file paths to stdout once during startup
    println!("Debug log file: {}", log_path.display());
    println!("Fallback log file: {}", fallback_path.display());

    // Store the fallback path first
    let _ = FALLBACK_LOG_FILE.set(fallback_path.clone());

    // Create or truncate the log file
    File::create(&log_path)?;
    File::create(&fallback_path)?;

    // Store the log path
    let _ = DEBUG_LOG_FILE.set(log_path);

    // Enable logging
    LOGGING_ENABLED.store(true, Ordering::SeqCst);

    Ok(())
}

/// Get the path to the debug log file, if it has been initialized
pub fn get_log_path() -> Option<PathBuf> {
    DEBUG_LOG_FILE.get().cloned()
}

/// Set whether verbose logging is enabled
pub fn set_verbose_logging(enabled: bool) {
    VERBOSE_LOGGING.store(enabled, Ordering::SeqCst);
}

/// Check if verbose logging is enabled
pub fn is_verbose_logging() -> bool {
    VERBOSE_LOGGING.load(Ordering::Relaxed)
}

/// Log a debug message to the file only - never to stderr
pub fn debug_log(message: &str) {
    // Skip logging if disabled
    if !LOGGING_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    // Only log to file, NEVER to stderr
    if let Some(path) = DEBUG_LOG_FILE.get() {
        // Try to append to the log file, ignore errors
        if let Ok(mut file) = OpenOptions::new().append(true).open(path) {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S.%3f");
            if writeln!(file, "[{}] {}", timestamp, message).is_err() {
                // Try the fallback file if primary fails
                let _ = write_to_fallback(&format!("[{}] {}", timestamp, message));
            }
        } else {
            // Try the fallback file if we couldn't open the primary
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S.%3f");
            let _ = write_to_fallback(&format!("[{}] {}", timestamp, message));
        }
    }
}

/// Log API request/response data when verbose logging is enabled
pub fn api_log(message: &str) {
    // Skip if disabled or verbose logging is off
    if !LOGGING_ENABLED.load(Ordering::Relaxed) || !VERBOSE_LOGGING.load(Ordering::Relaxed) {
        return;
    }

    let full_message = format!("[API] {}", message);
    debug_log(&full_message);
}

/// Log UI-related issues to a special logger that's safe for clipboard/UI operations
pub fn ui_log(message: &str) {
    if !LOGGING_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    let full_message = format!("[UI] {}", message);

    // Use the fallback logger since these are likely UI-related issues
    // that might indicate problems with the main logger
    let _ = write_to_fallback(&full_message);
}

/// Write directly to the fallback log without using the main logger
fn write_to_fallback(message: &str) -> std::io::Result<()> {
    if let Some(path) = FALLBACK_LOG_FILE.get() {
        let mut file = OpenOptions::new().append(true).create(true).open(path)?;

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S.%3f");
        writeln!(file, "[{}] {}", timestamp, message)?;
    }
    Ok(())
}
