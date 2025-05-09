// A simple terminal test that uses crossterm directly
use std::io::{self, Write};
use std::error::Error;
use crossterm::execute;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Testing terminal functionality...");
    
    // Print system information
    println!("TTY check:");
    if !atty::is(atty::Stream::Stdout) {
        println!("Warning: stdout is not a TTY");
    }
    
    if !atty::is(atty::Stream::Stdin) {
        println!("Warning: stdin is not a TTY");
    }
    
    // Attempt to get terminal size
    match crossterm::terminal::size() {
        Ok((width, height)) => {
            println!("Terminal size: {}x{}", width, height);
        }
        Err(e) => {
            println!("Failed to get terminal size: {}", e);
            return Err(e.into());
        }
    }
    
    // Try enabling raw mode
    if let Err(e) = crossterm::terminal::enable_raw_mode() {
        println!("Failed to enable raw mode: {}", e);
        return Err(e.into());
    }
    
    // Try to get cursor position
    match crossterm::cursor::position() {
        Ok((x, y)) => {
            println!("Cursor position: {}x{}", x, y);
        }
        Err(e) => {
            println!("Failed to get cursor position: {}", e);
            crossterm::terminal::disable_raw_mode()?;
            return Err(e.into());
        }
    }
    
    // Clean up
    crossterm::terminal::disable_raw_mode()?;
    
    println!("All terminal tests passed!");
    Ok(())
}