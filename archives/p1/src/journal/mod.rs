use crate::ui::MessageType;
use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use dirs::data_dir;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod test;

/// Journal struct for handling conversation logs
pub struct Journal {
    #[allow(dead_code)]
    app_data_dir: PathBuf,
    current_file: PathBuf,
}

impl Journal {
    /// Create a new Journal instance
    pub fn new() -> Result<Self> {
        // Get the standard app data directory
        let mut app_data_dir = data_dir()
            .context("Could not determine app data directory")?
            .join("mcpterm-rs");

        // Create the app data directory if it doesn't exist
        fs::create_dir_all(&app_data_dir).context("Failed to create app data directory")?;

        // Get the current date for the filename
        let today = Local::now().date_naive();

        // Create the journals subdirectory
        let journals_dir = app_data_dir.join("journals");
        fs::create_dir_all(&journals_dir).context("Failed to create journals directory")?;

        // Set app_data_dir to include the journals subdirectory
        app_data_dir = journals_dir;

        // Create the current journal file path
        let current_file = Self::get_journal_path(&app_data_dir, today);

        Ok(Self {
            app_data_dir,
            current_file,
        })
    }

    /// Generate a journal file path for a specific date
    fn get_journal_path(base_dir: &Path, date: NaiveDate) -> PathBuf {
        base_dir.join(format!("journal-{}.txt", date.format("%Y-%m-%d")))
    }

    /// Get today's journal file path
    // Returns the current journal path (available for external use if needed)
    #[allow(dead_code)]
    pub fn get_current_journal_path(&self) -> &PathBuf {
        &self.current_file
    }

    /// Load messages from the current journal file
    pub fn load_current_journal(&self) -> Result<Vec<(String, MessageType)>> {
        // If the file doesn't exist yet, return an empty vector
        if !self.current_file.exists() {
            return Ok(Vec::new());
        }

        self.load_journal_file(&self.current_file)
    }

    /// Load messages from a specific journal file
    pub fn load_journal_file(&self, file_path: &PathBuf) -> Result<Vec<(String, MessageType)>> {
        let file = File::open(file_path).context("Failed to open journal file")?;
        let reader = BufReader::new(file);

        let mut messages = Vec::new();
        let mut current_message_type = None;
        let mut current_content = String::new();

        for line in reader.lines() {
            let line = line.context("Failed to read line from journal file")?;

            // Check for message type markers
            if line.starts_with("--- USER ---") {
                // Save previous message if any
                if let Some(msg_type) = current_message_type {
                    if !current_content.is_empty() {
                        messages.push((current_content.trim().to_string(), msg_type));
                        current_content.clear();
                    }
                }
                current_message_type = Some(MessageType::User);
            } else if line.starts_with("--- ASSISTANT ---") {
                // Save previous message if any
                if let Some(msg_type) = current_message_type {
                    if !current_content.is_empty() {
                        messages.push((current_content.trim().to_string(), msg_type));
                        current_content.clear();
                    }
                }
                current_message_type = Some(MessageType::Assistant);
            } else if line.starts_with("--- SYSTEM ---") {
                // Save previous message if any
                if let Some(msg_type) = current_message_type {
                    if !current_content.is_empty() {
                        messages.push((current_content.trim().to_string(), msg_type));
                        current_content.clear();
                    }
                }
                current_message_type = Some(MessageType::System);
            } else if current_message_type.is_some() {
                // Append to current content
                current_content.push_str(&line);
                current_content.push('\n');
            }
        }

        // Add the last message if there is one
        if let Some(msg_type) = current_message_type {
            if !current_content.is_empty() {
                messages.push((current_content.trim().to_string(), msg_type));
            }
        }

        Ok(messages)
    }

    /// Append a message to the current journal file
    pub fn append_message(&self, content: &str, message_type: MessageType) -> Result<()> {
        // Create the file if it doesn't exist
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.current_file)
            .context("Failed to open journal file for writing")?;

        // Write message type header
        let header = match message_type {
            MessageType::User => "--- USER ---\n",
            MessageType::Assistant => "--- ASSISTANT ---\n",
            MessageType::System => "--- SYSTEM ---\n",
        };

        file.write_all(header.as_bytes())?;
        file.write_all(content.as_bytes())?;
        file.write_all(b"\n\n")?;

        Ok(())
    }
}
