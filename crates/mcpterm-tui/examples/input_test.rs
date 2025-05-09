use std::io::{self, Write};
use crossterm::{
    terminal::{enable_raw_mode, disable_raw_mode},
    event::{read, Event, KeyEvent, KeyCode},
};

fn main() -> io::Result<()> {
    // Ensure we have raw mode for direct key input
    enable_raw_mode()?;
    
    println!("Press keys (type 'exit' to quit):");
    
    let mut input = String::new();
    
    loop {
        // Read events from crossterm
        match read()? {
            Event::Key(KeyEvent { code, .. }) => {
                match code {
                    KeyCode::Char(c) => {
                        input.push(c);
                        print!("{}", c);
                        io::stdout().flush()?;
                        
                        // Check for exit command
                        if input.ends_with("exit") {
                            break;
                        }
                    }
                    KeyCode::Enter => {
                        input.clear();
                        println!("\r");
                    }
                    KeyCode::Backspace => {
                        if !input.is_empty() {
                            input.pop();
                            print!("\x08 \x08"); // Backspace, space, backspace to erase character
                            io::stdout().flush()?;
                        }
                    }
                    KeyCode::Esc => {
                        break;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    
    // Restore terminal
    disable_raw_mode()?;
    println!("\nExiting...");
    
    Ok(())
}