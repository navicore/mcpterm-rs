// Step 1: Minimal terminal UI that handles keyboard input reliably
// This is the starting point for rebuilding the mcpterm-tui application

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::time::Duration;

// Minimalist application that just echoes keys
fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    // Print instructions
    println!("Minimal Keyboard Test");
    println!("====================");
    println!("Type any key to see it echoed");
    println!("Press 'q' to quit");
    println!();

    // Main loop
    let mut running = true;
    let mut key_count = 0;

    while running {
        // Non-blocking poll for events with a short timeout
        if event::poll(Duration::from_millis(100))? {
            // Read the event
            if let Event::Key(key) = event::read()? {
                // Only process press events (not releases or repeats)
                if key.kind == KeyEventKind::Press {
                    // Update count and display the key
                    key_count += 1;
                    let key_str = format!("Key #{}: {:?}", key_count, key);

                    // Clear the line and display the key
                    execute!(
                        io::stdout(),
                        crossterm::cursor::MoveToColumn(0),
                        crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)
                    )?;
                    print!("{}", key_str);
                    io::stdout().flush()?;

                    // Check for quit key
                    if key.code == KeyCode::Char('q') {
                        running = false;
                    }
                }
            }
        }
    }

    // Clean up
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    Ok(())
}
