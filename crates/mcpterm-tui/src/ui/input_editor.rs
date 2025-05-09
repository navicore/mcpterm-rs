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
    style::{Color, Modifier, Style},
    widgets::{Block, Widget},
};
use tracing::{debug, info, warn};

/// Result type for editor key handling
pub enum HandleResult {
    /// Continue processing
    Continue,
    /// Submit the current content
    Submit(String),
    /// Use previous history item
    PreviousHistory,
    /// Use next history item
    NextHistory,
    /// Abort/Cancel the operation
    Abort,
}

/// EdtuiEditor is a wrapper around edtui's Editor
/// for our mcpterm application.
#[derive(Clone)]
pub struct InputEditor {
    state: EditorState,
    event_handler: EditorEventHandler,
    pub title: String,
    pub block: Option<Block<'static>>,
}

impl Default for InputEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl InputEditor {
    /// Create a new InputEditor
    pub fn new() -> Self {
        let state = EditorState::default();
        let event_handler = EditorEventHandler::default();

        Self {
            state,
            event_handler,
            title: "Input".to_string(),
            block: None,
        }
    }

    /// Set the content of the editor
    pub fn set_content(&mut self, content: &str) {
        self.state = EditorState::new(Lines::from(content));
        // Put cursor at the end of the content
        let last_row = self.state.lines.len().saturating_sub(1);
        if last_row < self.state.lines.len() {
            let last_col = self.state.lines.len_col(last_row).unwrap_or(0);
            self.state.cursor = edtui::Index2::new(last_row, last_col);
        }
    }

    /// Set the editor mode
    pub fn set_mode(&mut self, mode: EditorMode) {
        self.state.mode = mode;
    }

    /// Handle a key event and return the result of the handling
    pub fn handle_key_event(&mut self, key: KeyEvent) -> HandleResult {
        // Debug output to help diagnose issues
        info!("Input editor handling key: {:?} in mode: {:?}", key, self.state.mode);
        
        // Special case for Enter in normal mode - submit the text
        if key.code == KeyCode::Enter && self.state.mode == EditorMode::Normal {
            // Get the current text and return a Submit result
            let content = self.get_text();
            info!("Submitting content: '{}'", content);
            return HandleResult::Submit(content);
        }

        // Handle Ctrl+C to quit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            info!("Ctrl+C pressed - aborting");
            return HandleResult::Abort;
        }

        // Handle Ctrl+P/Ctrl+N for history navigation
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('p') => {
                    return HandleResult::PreviousHistory;
                }
                KeyCode::Char('n') => {
                    return HandleResult::NextHistory;
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
                    debug!("Pasted from clipboard successfully");
                }
                Err(e) => {
                    warn!("Failed to paste from clipboard: {}", e);
                }
            }
            return HandleResult::Continue;
        }

        // Handle yanking in visual mode with 'y'
        if key.code == KeyCode::Char('y') && self.state.mode == EditorMode::Visual {
            if let Err(e) = self.yank_to_clipboard() {
                warn!("Failed to yank to clipboard: {}", e);
            }
            // After yanking, return to normal mode and clear selection
            self.state.mode = EditorMode::Normal;
            self.state.selection = None; // Clear the selection to remove highlighting
            return HandleResult::Continue;
        }

        // Special handling for character input in insert mode
        if self.state.mode == EditorMode::Insert && matches!(key.code, KeyCode::Char(_)) {
            if let KeyCode::Char(c) = key.code {
                info!("Direct handling of character '{}' in insert mode", c);
                
                // Get current cursor position
                let cursor_row = self.state.cursor.row;
                let cursor_col = self.state.cursor.col;
                
                // Insert character directly at cursor position
                let mut content = self.get_text();
                if cursor_col >= content.len() {
                    // Append to the end
                    content.push(c);
                } else {
                    // Insert in middle
                    content.insert(cursor_col, c);
                }
                
                // Set updated content
                self.set_content(&content);
                
                // Move cursor forward
                self.state.cursor.col += 1;
                
                info!("Updated content to: '{}', cursor now at {}", content, self.state.cursor.col);
                return HandleResult::Continue;
            }
        }
        
        // For all other keys, pass to the edtui handler
        info!("Passing key event to edtui: {:?}", key);
        
        // Convert crossterm::event::KeyEvent to ratatui::crossterm::event::KeyEvent
        let ratatui_key = ratatui::crossterm::event::KeyEvent {
            code: convert_key_code(key.code),
            modifiers: convert_key_modifiers(key.modifiers),
            kind: ratatui::crossterm::event::KeyEventKind::Press,
            state: ratatui::crossterm::event::KeyEventState::NONE,
        };

        // Pass the key event to the edtui event handler
        let result = self.event_handler.on_key_event(ratatui_key, &mut self.state);
        info!("edtui event handler result: {:?}", result);

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

    /// Get the currently selected text
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

        // Try to get the text from the clipboard
        match clipboard.get_text() {
            Ok(text) => {
                if !text.is_empty() {
                    // Insert the text at the current cursor position
                    self.insert_text(&text);
                    debug!("Pasted text from clipboard: {} chars", text.len());
                } else {
                    debug!("Clipboard contains empty text");
                }
                Ok(())
            }
            Err(e) => {
                // Log the error for debugging
                warn!("Error getting text from clipboard: {}", e);

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
                                    debug!("Pasted text using pbpaste: {} chars", text.len());
                                }
                            }
                        }
                        Err(e) => {
                            warn!("pbpaste fallback also failed: {}", e);
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

        // Get current content
        let current_content = self.get_text();

        // Handle empty document case
        if current_content.is_empty() {
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
            return;
        }

        // Split into lines for easier manipulation
        let mut lines: Vec<String> = current_content.lines().map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }

        // Make sure row index is valid
        let row_idx = std::cmp::min(row, lines.len().saturating_sub(1));

        // Get current line
        let current_line = if row_idx < lines.len() {
            lines[row_idx].clone()
        } else {
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
                // Get parts of the current line
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

        // Create a string by iterating through the lines
        let lines = &self.state.lines;
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

    /// Clear the editor content
    pub fn clear(&mut self) {
        // Reset to a blank editor
        self.state = EditorState::new(Lines::from(""));
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

    /// Set the title of the editor block
    pub fn title(mut self, title: String) -> Self {
        self.title = title;
        self
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

        self.state.add_line_style_indices(line, start, end, text_style);
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

        self.state.add_line_style(line, StyleRange::new(range, text_style));
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
}

// Implement Widget for reference to InputEditor to avoid cloning
impl Widget for &mut InputEditor {
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
impl Widget for InputEditor {
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
pub fn convert_key_modifiers(
    modifiers: KeyModifiers,
) -> ratatui::crossterm::event::KeyModifiers {
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