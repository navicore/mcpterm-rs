// A simple test that checks terminal capabilities without ratatui
use std::error::Error;
use std::io::{self, Write};
use std::time::Duration;
use crossterm::execute;

fn main() -> Result<(), Box<dyn Error>> {
    // Print system information
    println!("Raw Terminal Test");
    println!("================");
    println!("Environment information:");
    println!("  TERM: {:?}", std::env::var("TERM").unwrap_or_else(|_| "not set".to_string()));
    println!("  TERM_PROGRAM: {:?}", std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "not set".to_string()));
    
    println!("\nTTY check:");
    if !atty::is(atty::Stream::Stdin) {
        println!("  Warning: stdin is not a TTY");
    } else {
        println!("  stdin is a TTY");
    }
    
    if !atty::is(atty::Stream::Stdout) {
        println!("  Warning: stdout is not a TTY");
    } else {
        println!("  stdout is a TTY");
    }
    
    if !atty::is(atty::Stream::Stderr) {
        println!("  Warning: stderr is not a TTY");
    } else {
        println!("  stderr is a TTY");
    }
    
    println!("\nTesting terminal capabilities:");
    
    // Test terminal size
    println!("  Testing terminal size...");
    match crossterm::terminal::size() {
        Ok((width, height)) => {
            println!("  Success: Terminal size: {}x{}", width, height);
        }
        Err(e) => {
            println!("  Error: Failed to get terminal size: {}", e);
        }
    }
    
    // Test raw mode
    println!("  Testing raw mode...");
    
    let enable_result = crossterm::terminal::enable_raw_mode();
    match &enable_result {
        Ok(_) => {
            println!("  Success: Raw mode enabled");
            
            // In raw mode, we have to manually print
            print!("  Testing key input in raw mode (press any key, or wait 10 seconds)...");
            io::stdout().flush()?;
            
            // Try to read input with timeout
            if crossterm::event::poll(Duration::from_secs(10))? {
                match crossterm::event::read() {
                    Ok(event) => {
                        print!("\r  Success: Received event: {:?}                 \n", event);
                        io::stdout().flush()?;
                    }
                    Err(e) => {
                        print!("\r  Error: Failed to read event: {}               \n", e);
                        io::stdout().flush()?;
                    }
                }
            } else {
                print!("\r  No input received within timeout                 \n");
                io::stdout().flush()?;
            }
            
            // Try to get cursor position
            print!("  Testing cursor position...");
            io::stdout().flush()?;
            
            match crossterm::cursor::position() {
                Ok((x, y)) => {
                    print!("\r  Success: Cursor position: {}x{}                 \n", x, y);
                    io::stdout().flush()?;
                }
                Err(e) => {
                    print!("\r  Error: Failed to get cursor position: {}               \n", e);
                    io::stdout().flush()?;
                }
            }
            
            // Disable raw mode
            if let Err(e) = crossterm::terminal::disable_raw_mode() {
                println!("  Error: Failed to disable raw mode: {}", e);
            } else {
                println!("  Raw mode disabled successfully");
            }
        }
        Err(e) => {
            println!("  Error: Failed to enable raw mode: {}", e);
        }
    }
    
    // Only try alternate screen if raw mode worked
    if enable_result.is_ok() {
        println!("\n  Testing alternate screen...");
        match execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen) {
            Ok(_) => {
                println!("  Success: Entered alternate screen");
                println!("  Will exit alternate screen in 3 seconds...");
                std::thread::sleep(Duration::from_secs(3));
                
                match execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen) {
                    Ok(_) => println!("  Success: Left alternate screen"),
                    Err(e) => println!("  Error: Failed to leave alternate screen: {}", e),
                }
            }
            Err(e) => {
                println!("  Error: Failed to enter alternate screen: {}", e);
            }
        }
    }
    
    println!("\nTest complete.");
    Ok(())
}