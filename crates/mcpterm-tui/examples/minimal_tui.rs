use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::time::Duration;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    
    // Clear screen
    execute!(stdout, crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
    execute!(stdout, crossterm::cursor::MoveTo(0, 0))?;
    write!(stdout, "Type characters. Press 'q' to quit.")?;
    stdout.flush()?;
    
    // Set cursor position
    execute!(stdout, crossterm::cursor::MoveTo(0, 2))?;
    stdout.flush()?;
    
    // Store current input
    let mut input = String::new();
    
    // Input handling loop
    loop {
        // Poll for events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char(c) => {
                            input.push(c);
                            execute!(stdout, crossterm::cursor::MoveTo(0, 3))?;
                            execute!(stdout, crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine))?;
                            write!(stdout, "Current input: {}", input)?;
                            stdout.flush()?;
                        }
                        KeyCode::Backspace => {
                            input.pop();
                            execute!(stdout, crossterm::cursor::MoveTo(0, 3))?;
                            execute!(stdout, crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine))?;
                            write!(stdout, "Current input: {}", input)?;
                            stdout.flush()?;
                        }
                        KeyCode::Enter => {
                            execute!(stdout, crossterm::cursor::MoveTo(0, 4))?;
                            write!(stdout, "You entered: {}", input)?;
                            input.clear();
                            execute!(stdout, crossterm::cursor::MoveTo(0, 3))?;
                            execute!(stdout, crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine))?;
                            write!(stdout, "Current input: {}", input)?;
                            stdout.flush()?;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    
    // Clean up
    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    
    Ok(())
}