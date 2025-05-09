use std::io::{self, Write};

fn main() -> io::Result<()> {
    // Print information about terminal environment
    println!("TTY check:");
    
    // Check if stdin is a TTY
    println!("Is stdin a TTY: {}", atty::is(atty::Stream::Stdin));
    
    // Check if stdout is a TTY
    println!("Is stdout a TTY: {}", atty::is(atty::Stream::Stdout));
    
    // Check if stderr is a TTY
    println!("Is stderr a TTY: {}", atty::is(atty::Stream::Stderr));
    
    // Check environment variables
    println!("\nEnvironment variables:");
    if let Ok(term) = std::env::var("TERM") {
        println!("TERM: {}", term);
    } else {
        println!("TERM not set");
    }
    
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        println!("TERM_PROGRAM: {}", term_program);
    } else {
        println!("TERM_PROGRAM not set");
    }
    
    Ok(())
}