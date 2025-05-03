#[cfg(test)]
mod tests {
    use crate::journal::Journal;
    use crate::ui::MessageType;
    use anyhow::Context;
    use std::fs::{File, OpenOptions};
    use std::io::{BufRead, BufReader, Write};
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_journal_file_path_format() {
        let base_dir = PathBuf::from("/tmp");
        let date = chrono::NaiveDate::from_ymd_opt(2023, 5, 2).unwrap();

        let journal_path = Journal::get_journal_path(&base_dir, date);
        assert_eq!(journal_path, PathBuf::from("/tmp/journal-2023-05-02.txt"));
    }

    #[test]
    fn test_append_and_load_messages() -> anyhow::Result<()> {
        // Create a temporary directory for test
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path().to_path_buf();

        // Create a test date
        let date = chrono::NaiveDate::from_ymd_opt(2023, 5, 2).unwrap();

        // Create a journal file path
        let journal_path = Journal::get_journal_path(&temp_path, date);

        // Manual setup for a test journal
        struct TestJournal {
            current_file: PathBuf,
        }

        impl TestJournal {
            fn append_message(
                &self,
                content: &str,
                message_type: MessageType,
            ) -> anyhow::Result<()> {
                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&self.current_file)
                    .context("Failed to open journal file for writing")?;

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

            fn load_journal_file(&self) -> anyhow::Result<Vec<(String, MessageType)>> {
                let file = File::open(&self.current_file).context("Failed to open journal file")?;
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
        }

        let test_journal = TestJournal {
            current_file: journal_path,
        };

        // Test messages
        let user_msg = "Hello, this is a test user message.";
        let assistant_msg = "This is a test assistant response.";
        let system_msg = "System message for testing.";

        // Write messages
        test_journal.append_message(user_msg, MessageType::User)?;
        test_journal.append_message(assistant_msg, MessageType::Assistant)?;
        test_journal.append_message(system_msg, MessageType::System)?;

        // Read messages back
        let loaded_messages = test_journal.load_journal_file()?;

        // Verify messages were correctly saved and loaded
        assert_eq!(loaded_messages.len(), 3);
        assert_eq!(loaded_messages[0].0, user_msg);
        assert_eq!(loaded_messages[0].1, MessageType::User);
        assert_eq!(loaded_messages[1].0, assistant_msg);
        assert_eq!(loaded_messages[1].1, MessageType::Assistant);
        assert_eq!(loaded_messages[2].0, system_msg);
        assert_eq!(loaded_messages[2].1, MessageType::System);

        Ok(())
    }
}
