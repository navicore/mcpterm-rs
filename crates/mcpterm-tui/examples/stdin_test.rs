use std::io::{self, BufRead};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut input = String::new();
    
    println!("Enter some text:");
    
    for line in stdin.lock().lines() {
        match line {
            Ok(text) => {
                if text == "exit" {
                    break;
                }
                println!("You entered: {}", text);
            }
            Err(e) => {
                println!("Error reading input: {}", e);
                break;
            }
        }
    }
    
    println!("Exiting...");
    Ok(())
}