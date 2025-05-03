use crate::mcp::ui_log;
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use edtui::{
    EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Lines, StyleRange,
    TextStyle,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Widget},
};

use super::common::HandleResult;

/// EdtuiEditor is a wrapper around edtui's Editor
/// for our mcpterm application.
#[derive(Clone)]
pub struct EdtuiEditor {
    state: EditorState,
    event_handler: EditorEventHandler,
    pub title: String,
    pub block: Option<Block<'static>>,
    command_history: Vec<String>,
    history_index: Option<usize>,
}

impl Default for EdtuiEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl EdtuiEditor {
    /// Create a new EdtuiEditor
    pub fn new() -> Self {
        let state = EditorState::default();
        let event_handler = EditorEventHandler::default();

        Self {
            state,
            event_handler,
            title: "Input".to_string(),
            block: None,
            command_history: Vec::new(),
            history_index: None,
        }
    }

    /// Navigate to previous command in history (up)
    fn history_up(&mut self) {
        // If history is empty, do nothing
        if self.command_history.is_empty() {
            return;
        }

        // If we're not already navigating history, store the current index
        // at one past the end of the history (for restoring current input)
        if self.history_index.is_none() {
            self.history_index = Some(self.command_history.len());
        }

        // Move up in history if possible
        if let Some(idx) = self.history_index {
            if idx > 0 {
                let new_idx = idx - 1;
                self.history_index = Some(new_idx);

                // Set editor content to the history item
                let history_item = &self.command_history[new_idx];
                self.state = EditorState::new(Lines::from(history_item.clone()));

                // Put cursor at the end of the content
                let last_row = self.state.lines.len().saturating_sub(1);
                if last_row < self.state.lines.len() {
                    let last_col = self.state.lines.len_col(last_row).unwrap_or(0);
                    self.state.cursor = edtui::Index2::new(last_row, last_col);
                }
            }
        }
    }

    /// Navigate to next command in history (down)
    fn history_down(&mut self) {
        // If not navigating history, do nothing
        if self.history_index.is_none() {
            return;
        }

        let idx = self.history_index.unwrap();

        // If we're at the end of history, clear the input
        if idx >= self.command_history.len().saturating_sub(1) {
            self.history_index = None;
            self.clear();
            return;
        }

        // Move down in history
        let new_idx = idx + 1;
        self.history_index = Some(new_idx);

        // If we've reached the end of history, clear the input
        if new_idx >= self.command_history.len() {
            self.clear();
            self.history_index = None;
        } else {
            // Set editor content to the history item
            let history_item = &self.command_history[new_idx];
            self.state = EditorState::new(Lines::from(history_item.clone()));

            // Put cursor at the end of the content
            let last_row = self.state.lines.len().saturating_sub(1);
            if last_row < self.state.lines.len() {
                let last_col = self.state.lines.len_col(last_row).unwrap_or(0);
                self.state.cursor = edtui::Index2::new(last_row, last_col);
            }
        }
    }

    /// Handle a key event and return the result of the handling
    pub fn handle_key_event(&mut self, key: KeyEvent) -> HandleResult {
        // Special case for Enter in normal mode - submit the text
        if key.code == KeyCode::Enter && self.state.mode == EditorMode::Normal {
            // Get the current text and return a Submit result
            let content = self.get_text();
            return HandleResult::Submit(content);
        }

        // Handle Ctrl+P (previous), Ctrl+N (next), Ctrl+F (page down), Ctrl+B (page up)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('p') => {
                    self.history_up();
                    return HandleResult::Continue;
                }
                KeyCode::Char('n') => {
                    self.history_down();
                    return HandleResult::Continue;
                }
                KeyCode::Char('f') => {
                    self.page_down();
                    return HandleResult::Continue;
                }
                KeyCode::Char('b') => {
                    self.page_up();
                    return HandleResult::Continue;
                }
                _ => {}
            }
        }

        // Handle Ctrl+C to quit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return HandleResult::Abort;
        }

        // Handle Ctrl+V to paste from system clipboard
        if key.code == KeyCode::Char('v') && key.modifiers.contains(KeyModifiers::CONTROL) {
            // If not in insert mode, switch to insert mode first
            if self.state.mode != EditorMode::Insert {
                self.state.mode = EditorMode::Insert;
            }

            // Try to paste from clipboard
            match self.paste_from_clipboard() {
                Ok(_) => {
                    // Successfully pasted
                    ui_log("Pasted from clipboard successfully");
                }
                Err(e) => {
                    ui_log(&format!("Failed to paste from clipboard: {}", e));
                }
            }
            return HandleResult::Continue;
        }

        // Handle yanking in visual mode with 'y'
        if key.code == KeyCode::Char('y') && self.state.mode == EditorMode::Visual {
            if let Err(e) = self.yank_to_clipboard() {
                ui_log(&format!("Failed to yank to clipboard: {}", e));
            }
            // After yanking, return to normal mode and clear selection
            self.state.mode = EditorMode::Normal;
            self.state.selection = None; // Clear the selection to remove highlighting
            return HandleResult::Continue;
        }

        // Convert crossterm::event::KeyEvent to ratatui::crossterm::event::KeyEvent
        let ratatui_key = ratatui::crossterm::event::KeyEvent {
            code: convert_key_code(key.code),
            modifiers: convert_key_modifiers(key.modifiers),
            kind: ratatui::crossterm::event::KeyEventKind::Press,
            state: ratatui::crossterm::event::KeyEventState::NONE,
        };

        // Pass the key event to the edtui event handler
        self.event_handler
            .on_key_event(ratatui_key, &mut self.state);

        HandleResult::Continue
    }

    /// Copy selected text to system clipboard
    fn yank_to_clipboard(&self) -> Result<()> {
        // Check if we have a selection
        if let Some(selection) = self.get_selected_text() {
            // Create a new clipboard instance
            let mut clipboard = Clipboard::new()?;

            // Set the clipboard text
            clipboard.set_text(selection)?;

            // Return success
            Ok(())
        } else {
            // No selection, so nothing to copy
            Ok(())
        }
    }

    /// Get the currently selected text using a simplified approach
    fn get_selected_text(&self) -> Option<String> {
        // Check if we're in visual mode with a selection
        if self.state.mode == EditorMode::Visual {
            if let Some(selection) = &self.state.selection {
                // Get the current full text
                let full_text = self.get_text();

                // Convert the full text to lines for easier processing
                let lines: Vec<&str> = full_text.lines().collect();

                // Get start and end positions
                let (start_row, start_col) = (selection.start.row, selection.start.col);
                let (end_row, end_col) = (selection.end.row, selection.end.col);

                // Prepare to collect the selection
                let mut selected_text = String::new();

                // Process the selection line by line
                for row in start_row..=end_row {
                    if row < lines.len() {
                        let line = lines[row];

                        // Determine start and end columns for this row
                        let s_col = if row == start_row { start_col } else { 0 };
                        let e_col = if row == end_row {
                            // Include the character under the cursor by adding 1, but don't exceed line length
                            std::cmp::min(end_col + 1, line.len())
                        } else {
                            line.len()
                        };

                        // Extract text for this row if valid
                        if s_col <= e_col && s_col < line.len() {
                            // Get the substring safely
                            let start_byte = line
                                .char_indices()
                                .nth(s_col)
                                .map_or(line.len(), |(i, _)| i);

                            let end_byte = line
                                .char_indices()
                                .nth(e_col)
                                .map_or(line.len(), |(i, _)| i);

                            let row_text = &line[start_byte..end_byte];

                            // Add this row's text to result
                            if !selected_text.is_empty() {
                                selected_text.push('\n');
                            }
                            selected_text.push_str(row_text);
                        }
                    }
                }

                // Return the selected text if not empty
                if !selected_text.is_empty() {
                    return Some(selected_text);
                }
            }
        }

        None
    }

    /// Paste text from system clipboard
    fn paste_from_clipboard(&mut self) -> Result<()> {
        // Create a new clipboard instance
        let mut clipboard = Clipboard::new()?;

        // Try to get the text from the clipboard with better error handling
        match clipboard.get_text() {
            Ok(text) => {
                if !text.is_empty() {
                    // Insert the text at the current cursor position
                    self.insert_text(&text);

                    // Print a debug message
                    ui_log(&format!(
                        "Pasted text from clipboard: '{}' (len: {})",
                        if text.len() > 20 { &text[0..20] } else { &text },
                        text.len()
                    ));
                } else {
                    ui_log("Clipboard contains empty text");
                }
                Ok(())
            }
            Err(e) => {
                // Log the error for debugging
                ui_log(&format!("Error getting text from clipboard: {}", e));

                // Try a fallback approach
                #[cfg(target_os = "macos")]
                {
                    // On macOS, we can try to use pb_paste as a workaround
                    use std::process::Command;
                    match Command::new("pbpaste").output() {
                        Ok(output) => {
                            if let Ok(text) = String::from_utf8(output.stdout) {
                                if !text.is_empty() {
                                    self.insert_text(&text);
                                    ui_log(&format!(
                                        "Pasted text using pbpaste: '{}' (len: {})",
                                        if text.len() > 20 { &text[0..20] } else { &text },
                                        text.len()
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            ui_log(&format!("pbpaste fallback also failed: {}", e));
                        }
                    }
                }

                Ok(())
            }
        }
    }

    /// Insert text at current cursor position
    fn insert_text(&mut self, text: &str) {
        // Skip if text is empty
        if text.is_empty() {
            return;
        }

        // Ensure we're in insert mode
        self.state.mode = EditorMode::Insert;

        // Get current cursor position
        let row = self.state.cursor.row;
        let col = self.state.cursor.col;

        // Simplify the approach: directly create a new state from the modified text

        // Get current content
        let current_content = self.get_text();

        // Split into lines for easier manipulation
        let mut lines: Vec<String> = current_content.lines().map(String::from).collect();

        // Handle empty document or empty lines vector case
        if current_content.is_empty() || lines.is_empty() {
            // Just set the content to the pasted text
            self.state = EditorState::new(Lines::from(text));

            // Calculate final cursor position
            let text_lines: Vec<&str> = text.lines().collect();
            let new_row = if text_lines.is_empty() {
                0
            } else {
                text_lines.len().saturating_sub(1)
            };

            let new_col = if text_lines.is_empty() {
                0
            } else {
                text_lines[new_row].len()
            };

            // Set cursor position
            self.state.cursor = edtui::Index2::new(new_row, new_col);

            // Stay in insert mode
            self.state.mode = EditorMode::Insert;
            return;
        }

        // Make sure row index is valid
        let row_idx = std::cmp::min(row, lines.len().saturating_sub(1));

        // We need to obtain the current line as a string and drop the borrow before modifying
        let current_line = if row_idx < lines.len() {
            lines[row_idx].clone()
        } else {
            // If row is beyond end, use the last line
            if lines.is_empty() {
                String::new()
            } else {
                lines[lines.len() - 1].clone()
            }
        };

        // Make sure col index is valid
        let col_idx = std::cmp::min(col, current_line.len());

        // Split the pasted text into lines
        let paste_lines: Vec<&str> = text.lines().collect();

        if paste_lines.len() == 1 {
            // Simple case: single line paste
            let paste_text = paste_lines[0];

            // Insert within the current line
            if row_idx < lines.len() {
                let before = &current_line[..col_idx];
                let after = &current_line[col_idx..];

                lines[row_idx] = format!("{}{}{}", before, paste_text, after);

                // Set new content
                let new_content = lines.join("\n");
                self.state = EditorState::new(Lines::from(new_content));

                // Update cursor position
                self.state.cursor = edtui::Index2::new(row_idx, col_idx + paste_text.len());
            }
        } else {
            // Multi-line paste - more complex
            if row_idx < lines.len() {
                // Get parts of the current line before modifying 'lines'
                let before = current_line[..col_idx].to_string();
                let after = current_line[col_idx..].to_string();

                // Replace current line with first part + first line of paste
                lines[row_idx] = format!("{}{}", before, paste_lines[0]);

                // Insert remaining paste lines
                let mut insert_row = row_idx + 1;
                (1..paste_lines.len() - 1).for_each(|i| {
                    lines.insert(insert_row, paste_lines[i].to_string());
                    insert_row += 1;
                });

                // Add last paste line + remainder of original line
                if paste_lines.len() > 1 {
                    lines.insert(
                        insert_row,
                        format!("{}{}", paste_lines[paste_lines.len() - 1], after),
                    );
                }

                // Set new content
                let new_content = lines.join("\n");
                self.state = EditorState::new(Lines::from(new_content));

                // Calculate final cursor position (end of pasted text)
                let final_row = row_idx + paste_lines.len() - 1;
                let final_col = if paste_lines.len() > 1 {
                    paste_lines[paste_lines.len() - 1].len()
                } else {
                    col_idx + paste_lines[0].len()
                };

                // Set cursor position
                self.state.cursor = edtui::Index2::new(final_row, final_col);
            }
        }

        // Ensure we stay in insert mode
        self.state.mode = EditorMode::Insert;
    }

    /// Get the current text content
    pub fn get_text(&self) -> String {
        // Extract text from the Lines structure
        let mut result = String::new();

        // Lines in edtui has from_string/to_string functionality
        // But we need to manually convert to string by iterating ourselves
        let lines = &self.state.lines;

        // Create a string by iterating through the lines
        let mut first_line = true;
        for line in lines.iter_row() {
            if !first_line {
                result.push('\n');
            }
            first_line = false;

            for ch in line {
                result.push(*ch);
            }
        }

        result
    }

    ///// Set the text content
    // pub fn set_text(&mut self, text: String) {
    //     // Create new Lines from the text
    //     self.state = EditorState::new(Lines::from(text));
    // }

    /// Clear the editor content
    pub fn clear(&mut self) {
        // Reset to a blank editor
        self.state = EditorState::new(Lines::from(""));

        // Reset history navigation
        self.history_index = None;
    }

    /// Add current content to command history
    pub fn add_to_history(&mut self) {
        let current_text = self.get_text();

        // Only add non-empty text to history
        if !current_text.is_empty() {
            // Don't add duplicate consecutive entries
            if self.command_history.last() != Some(&current_text) {
                self.command_history.push(current_text);
            }

            // Reset history navigation index
            self.history_index = None;
        }
    }

    /// Get the command history (for debugging or display)
    pub fn get_history(&self) -> &[String] {
        &self.command_history
    }

    /// Get current history index if we're navigating history
    pub fn get_history_index(&self) -> Option<usize> {
        self.history_index
    }

    /// Set the title of the editor block
    pub fn title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    /// Scroll down by one page (approximately)
    pub fn page_down(&mut self) {
        // Get current cursor position
        let current_row = self.state.cursor.row;

        // Calculate a "page" as about 10 lines
        const PAGE_SIZE: usize = 10;

        // Calculate the target row (with bounds checking)
        let total_rows = self.state.lines.len();
        let target_row = (current_row + PAGE_SIZE).min(total_rows.saturating_sub(1));

        // Move cursor to target position
        if target_row < total_rows {
            // Keep the same column position when possible
            let target_col = self
                .state
                .cursor
                .col
                .min(self.state.lines.len_col(target_row).unwrap_or(0));

            self.state.cursor = edtui::Index2::new(target_row, target_col);
        }
    }

    /// Scroll up by one page (approximately)
    pub fn page_up(&mut self) {
        // Get current cursor position
        let current_row = self.state.cursor.row;

        // Calculate a "page" as about 10 lines
        const PAGE_SIZE: usize = 10;

        // Calculate the target row (with bounds checking)
        let target_row = current_row.saturating_sub(PAGE_SIZE);

        // Keep the same column position when possible
        let target_col = self
            .state
            .cursor
            .col
            .min(self.state.lines.len_col(target_row).unwrap_or(0));

        self.state.cursor = edtui::Index2::new(target_row, target_col);
    }

    /// Set the block for the editor
    pub fn block(mut self, block: Block<'static>) -> Self {
        self.block = Some(block);
        self
    }

    /// Apply a style to a range of text in a specific line
    pub fn apply_style(
        &mut self,
        line: usize,
        start: usize,
        end: usize,
        style: ratatui::style::Style,
        name: Option<String>,
    ) {
        let text_style = match name {
            Some(name_str) => TextStyle::with_name(style, name_str),
            None => TextStyle::new(style),
        };

        self.state
            .add_line_style_indices(line, start, end, text_style);
    }

    /// Apply a style to a range of text using a style range
    pub fn apply_style_range(
        &mut self,
        line: usize,
        range: std::ops::Range<usize>,
        style: ratatui::style::Style,
        name: Option<String>,
    ) {
        let text_style = match name {
            Some(name_str) => TextStyle::with_name(style, name_str),
            None => TextStyle::new(style),
        };

        self.state
            .add_line_style(line, StyleRange::new(range, text_style));
    }

    /// Clear all styles with a specific name
    pub fn clear_styles_by_name(&mut self, name: &str) {
        self.state.remove_line_styles_by_name(name);
    }

    /// Clear all styles for a specific line
    pub fn clear_styles_for_line(&mut self, line: usize) {
        self.state.remove_line_styles_for_line(line);
    }

    /// Clear all styles
    pub fn clear_all_styles(&mut self) {
        self.state.clear_line_styles();
    }

    // /// Set a placeholder for the editor (not implemented in edtui)
    // pub fn placeholder(self, _placeholder: String) -> Self {
    //     // Edtui doesn't have a built-in placeholder
    //     self
    // }
}

// Implement Widget for reference to EdtuiEditor to avoid cloning
impl Widget for &mut EdtuiEditor {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create the editor view with direct reference to our state
        let mut view = EditorView::new(&mut self.state);

        // Create theme with our block
        let mut theme = EditorTheme::default();
        if let Some(block) = &self.block {
            theme = theme.block(block.clone());
        } else {
            theme = theme.block(Block::default().title(self.title.clone()));
        }

        // Set theme and word wrap
        view = view.theme(theme).wrap(true);

        // Render the view
        view.render(area, buf);
    }
}

// Keep the original implementation for backward compatibility
impl Widget for EdtuiEditor {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a mutable copy of the state for rendering
        let mut state = self.state;

        // Create the editor view
        let mut view = EditorView::new(&mut state);

        // Create theme with our block
        let mut theme = EditorTheme::default();
        if let Some(block) = self.block {
            theme = theme.block(block);
        } else {
            theme = theme.block(Block::default().title(self.title));
        }

        // Set theme and word wrap
        view = view.theme(theme).wrap(true);

        // Render the view
        view.render(area, buf);
    }
}

/// Convert KeyCode from crossterm to ratatui::crossterm
pub fn convert_key_code(code: KeyCode) -> ratatui::crossterm::event::KeyCode {
    match code {
        KeyCode::Backspace => ratatui::crossterm::event::KeyCode::Backspace,
        KeyCode::Enter => ratatui::crossterm::event::KeyCode::Enter,
        KeyCode::Left => ratatui::crossterm::event::KeyCode::Left,
        KeyCode::Right => ratatui::crossterm::event::KeyCode::Right,
        KeyCode::Up => ratatui::crossterm::event::KeyCode::Up,
        KeyCode::Down => ratatui::crossterm::event::KeyCode::Down,
        KeyCode::Home => ratatui::crossterm::event::KeyCode::Home,
        KeyCode::End => ratatui::crossterm::event::KeyCode::End,
        KeyCode::PageUp => ratatui::crossterm::event::KeyCode::PageUp,
        KeyCode::PageDown => ratatui::crossterm::event::KeyCode::PageDown,
        KeyCode::Tab => ratatui::crossterm::event::KeyCode::Tab,
        KeyCode::BackTab => ratatui::crossterm::event::KeyCode::BackTab,
        KeyCode::Delete => ratatui::crossterm::event::KeyCode::Delete,
        KeyCode::Insert => ratatui::crossterm::event::KeyCode::Insert,
        KeyCode::F(n) => ratatui::crossterm::event::KeyCode::F(n),
        KeyCode::Char(c) => ratatui::crossterm::event::KeyCode::Char(c),
        KeyCode::Null => ratatui::crossterm::event::KeyCode::Null,
        KeyCode::Esc => ratatui::crossterm::event::KeyCode::Esc,
        _ => ratatui::crossterm::event::KeyCode::Null,
    }
}

/// Convert KeyModifiers from crossterm to ratatui::crossterm
pub fn convert_key_modifiers(modifiers: KeyModifiers) -> ratatui::crossterm::event::KeyModifiers {
    let mut result = ratatui::crossterm::event::KeyModifiers::empty();

    if modifiers.contains(KeyModifiers::SHIFT) {
        result.insert(ratatui::crossterm::event::KeyModifiers::SHIFT);
    }
    if modifiers.contains(KeyModifiers::CONTROL) {
        result.insert(ratatui::crossterm::event::KeyModifiers::CONTROL);
    }
    if modifiers.contains(KeyModifiers::ALT) {
        result.insert(ratatui::crossterm::event::KeyModifiers::ALT);
    }
    if modifiers.contains(KeyModifiers::SUPER) {
        result.insert(ratatui::crossterm::event::KeyModifiers::SUPER);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use edtui::EditorMode;
    use ratatui::style::{Color, Modifier, Style};

    /// Create a new EdtuiEditor for testing
    fn create_test_editor() -> EdtuiEditor {
        EdtuiEditor::new()
    }

    #[test]
    fn test_apply_styles() {
        let mut editor = create_test_editor();

        // Enter insert mode
        let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
        editor.handle_key_event(i_key);

        // Type "styled text example"
        for ch in "styled text example".chars() {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty());
            editor.handle_key_event(key);
        }

        // Apply styles
        let bold_red = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
        let italic_green = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::ITALIC);

        // Style "styled" in bold red
        editor.apply_style(0, 0, 6, bold_red, Some("keyword".to_string()));

        // Style "example" in italic green
        editor.apply_style(0, 12, 19, italic_green, Some("value".to_string()));

        // Clear styles by name
        editor.clear_styles_by_name("value");

        // Verify the text wasn't changed
        assert_eq!(editor.get_text(), "styled text example");
    }

    #[test]
    fn test_editor_initial_state() {
        let editor = create_test_editor();

        // Check that a new editor starts with empty text
        assert_eq!(editor.get_text(), "");

        // Check that the editor starts in normal mode
        assert_eq!(editor.state.mode, EditorMode::Normal);
    }

    #[test]
    fn test_handle_key_event_enter_normal_mode() {
        let mut editor = create_test_editor();

        // When Enter is pressed in normal mode, it should submit the text
        let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        let result = editor.handle_key_event(enter_key);

        // Verify that Enter in normal mode results in submission
        match result {
            HandleResult::Submit(text) => {
                assert_eq!(text, ""); // Should be empty since we didn't add any text
            }
            _ => panic!("Expected HandleResult::Submit but got a different result"),
        }
    }

    #[test]
    fn test_handle_key_event_abort() {
        let mut editor = create_test_editor();

        // When Ctrl+C is pressed, it should abort
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let result = editor.handle_key_event(ctrl_c);

        // Verify that Ctrl+C results in aborting
        match result {
            HandleResult::Abort => {}
            _ => panic!("Expected HandleResult::Abort but got a different result"),
        }
    }

    #[test]
    fn test_enter_insert_mode_then_type() {
        let mut editor = create_test_editor();

        // First, press 'i' to enter insert mode
        let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
        editor.handle_key_event(i_key);

        // Verify the editor switched to insert mode
        assert_eq!(editor.state.mode, EditorMode::Insert);

        // Type "hello"
        for ch in "hello".chars() {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty());
            editor.handle_key_event(key);
        }

        // Verify the text was added
        assert_eq!(editor.get_text(), "hello");

        // Press Escape to go back to normal mode
        let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
        editor.handle_key_event(esc_key);

        // Verify we're back in normal mode
        assert_eq!(editor.state.mode, EditorMode::Normal);
    }

    #[test]
    fn test_enter_key_in_insert_mode() {
        let mut editor = create_test_editor();

        // First, press 'i' to enter insert mode
        let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
        editor.handle_key_event(i_key);

        // Type "line1"
        for ch in "line1".chars() {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty());
            editor.handle_key_event(key);
        }

        // Press Enter to create a new line
        let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        editor.handle_key_event(enter_key);

        // Type "line2"
        for ch in "line2".chars() {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty());
            editor.handle_key_event(key);
        }

        // Verify we have two lines
        assert_eq!(editor.get_text(), "line1\nline2");

        // Verify Enter in insert mode doesn't submit (we're still in the editor)
        assert_eq!(editor.state.mode, EditorMode::Insert);
    }

    #[test]
    fn test_clear() {
        let mut editor = create_test_editor();

        // Enter insert mode and add text
        let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
        editor.handle_key_event(i_key);

        for ch in "text to clear".chars() {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty());
            editor.handle_key_event(key);
        }

        // Verify text was added
        assert_eq!(editor.get_text(), "text to clear");

        // Clear the editor
        editor.clear();

        // Verify text was cleared
        assert_eq!(editor.get_text(), "");
    }
}
