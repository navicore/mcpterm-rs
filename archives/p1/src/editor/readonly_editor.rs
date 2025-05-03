use crate::mcp::ui_log;
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use edtui::{EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Lines};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::{Block, Widget},
};

use super::common::HandleResult;
use super::edtui_editor::{convert_key_code, convert_key_modifiers};

/// ReadOnlyEditor is a wrapper around edtui's Editor that prevents content modifications
/// but allows for navigation, selection, and copying.
/// Search is handled natively by the edtui crate.
#[derive(Clone)]
pub struct ReadOnlyEditor {
    state: EditorState,
    event_handler: EditorEventHandler,
    pub title: String,
    pub block: Option<Block<'static>>,
    pub highlight_color: Option<Color>,
    pub is_search_mode: bool, // Public flag to track search mode for external code
}

impl Default for ReadOnlyEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadOnlyEditor {
    /// Create a new ReadOnlyEditor
    pub fn new() -> Self {
        // Create the default state
        let mut state = EditorState::default();
        let event_handler = EditorEventHandler::default();

        // Enable word wrap - this is important for proper scrolling
        // since we can't access view directly, we use the EditorView to set it
        {
            // The wrap method mutates the state's view field
            // but also returns self for method chaining which we don't need here
            let _ = EditorView::new(&mut state).wrap(true);
        }

        Self {
            state,
            event_handler,
            title: "Messages".to_string(),
            block: None,
            highlight_color: None,
            is_search_mode: false,
        }
    }

    /// Append content to the editor at the end
    pub fn append_content(&mut self, content: String) {
        // Get current content
        let current_content = self.get_content();

        // Create new combined content
        let new_content = if current_content.is_empty() {
            content
        } else {
            format!("{}\n{}", current_content, content)
        };

        // Save cursor mode only (we don't need position)
        let cursor_mode = self.state.mode;

        // Create a new state with the content
        self.state = EditorState::new(Lines::from(new_content));

        // Restore cursor mode
        self.state.mode = cursor_mode;

        // Very important: Position cursor at the end to ensure scrolling works properly
        self.scroll_to_bottom();
    }

    /// Append content with standardized prefixes and styling
    pub fn append_styled_content(
        &mut self,
        content: String,
        message_type: super::super::ui::MessageType,
    ) {
        // Define prefixes - use simple text rather than trying to apply styling
        // to part of the content in the editor
        let prefix = match message_type {
            super::super::ui::MessageType::System => "\nSYSTEM:",
            super::super::ui::MessageType::User => "\nYOU:",
            super::super::ui::MessageType::Assistant => "\nMCP:",
        };

        // Format the message with the prefix and a space
        let formatted_message = format!("{} {}", prefix, content);

        // Append the content without any styling attempts
        self.append_content(formatted_message);

        // Ensure we're not in search mode after content changes
        self.is_search_mode = false;
    }

    /// Get the current content of the editor
    pub fn get_content(&self) -> String {
        let mut result = String::new();
        let mut first_line = true;

        for line in self.state.lines.iter_row() {
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

    // Removed style methods that caused performance issues

    /// Manually scroll to the bottom of the content
    pub fn scroll_to_bottom(&mut self) {
        // Move cursor to end of content
        let last_row = self.state.lines.len().saturating_sub(1);
        if last_row > 0 {
            let last_col = self.state.lines.len_col(last_row).unwrap_or(0);
            self.state.cursor = edtui::Index2::new(last_row, last_col);
        }
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

    /// Debug function (now a placeholder since we're not using custom styling)
    pub fn debug_styles(&self) {
        // No styling to debug in this implementation
    }

    // Search is now handled directly by the edtui crate
    // No custom search implementation required

    /// Clear all content from the editor
    pub fn clear(&mut self) {
        // Create a new empty state
        let mut new_state = EditorState::new(Lines::from(""));

        // Preserve the existing mode
        new_state.mode = self.state.mode;

        // Replace the current state with the empty one
        self.state = new_state;

        // Reset search mode flag
        self.is_search_mode = false;

        // Restore normal title if there's a block
        if let Some(block) = &mut self.block {
            *block = block.clone().title(self.title.clone());
        }
    }

    /// Handle a key event in a read-only manner
    pub fn handle_key_event(&mut self, key: KeyEvent) -> HandleResult {
        // Special case for control+c to abort
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return HandleResult::Abort;
        }

        // Special case for Ctrl+V to paste from system clipboard
        if key.code == KeyCode::Char('v') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let Err(e) = self.paste_from_clipboard() {
                ui_log(&format!("Failed to paste from clipboard: {}", e));
            }
            return HandleResult::Continue;
        }

        // Add Ctrl+F for Page Down
        if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.page_down();
            return HandleResult::Continue;
        }

        // Add Ctrl+B for Page Up
        if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.page_up();
            return HandleResult::Continue;
        }

        // Note: Search functionality is handled by the edtui crate

        // Handle 'y' in Visual mode to yank (copy) selected text to clipboard
        if key.code == KeyCode::Char('y') && self.state.mode == EditorMode::Visual {
            if let Err(e) = self.yank_to_clipboard() {
                ui_log(&format!("Failed to yank to clipboard: {}", e));
            }

            // Exit visual mode after yanking
            self.state.mode = EditorMode::Normal;
            self.state.selection = None; // Clear the selection to remove highlighting
            return HandleResult::Continue;
        }

        // Filter out keys that would modify content in normal mode
        if self.state.mode == EditorMode::Normal
            && matches!(
                key.code,
                KeyCode::Char('i')
                    | KeyCode::Char('a')
                    | KeyCode::Char('o')
                    | KeyCode::Char('I')
                    | KeyCode::Char('A')
                    | KeyCode::Char('O')
                    | KeyCode::Char('c')
                    | KeyCode::Char('C')
                    | KeyCode::Char('s')
                    | KeyCode::Char('S')
                    | KeyCode::Char('d')
                    | KeyCode::Char('D')
                    | KeyCode::Delete
            )
        {
            return HandleResult::Continue;
        }

        // Special case for 'G' - move to end of content
        if key.code == KeyCode::Char('G') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            self.scroll_to_bottom();
            return HandleResult::Continue;
        }

        // Special case for 'gg' pattern - we just handle the last 'g' here
        if key.code == KeyCode::Char('g') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            // The actual 'gg' sequence is handled internally by edtui, but we ensure it actually works
            self.state.cursor = edtui::Index2::new(0, 0);
            return HandleResult::Continue;
        }

        // Convert the key to ratatui-compatible key
        let ratatui_key = ratatui::crossterm::event::KeyEvent {
            code: convert_key_code(key.code),
            modifiers: convert_key_modifiers(key.modifiers),
            kind: ratatui::crossterm::event::KeyEventKind::Press,
            state: ratatui::crossterm::event::KeyEventState::NONE,
        };

        // Pass the key to the editor handler
        self.event_handler
            .on_key_event(ratatui_key, &mut self.state);

        // Force the editor to stay in appropriate modes for read-only operation
        // Allow Normal, Visual, and Search modes, but prevent Insert or other editing modes
        let is_search_mode = format!("{:?}", self.state.mode) == "Search";

        // Update the public flag to track search mode
        self.is_search_mode = is_search_mode;

        if self.state.mode != EditorMode::Normal
            && self.state.mode != EditorMode::Visual
            && !is_search_mode
        {
            self.state.mode = EditorMode::Normal;
        }

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
                let full_text = self.get_content();

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

    /// Paste text from system clipboard (read-only mode, so this is disabled)
    fn paste_from_clipboard(&self) -> Result<()> {
        // In read-only mode, we don't support pasting
        // This function exists for API completeness
        Ok(())
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
}

// Implement Widget for a reference to ReadOnlyEditor
// This avoids cloning the editor and losing state
impl Widget for &mut ReadOnlyEditor {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a temp block based on the current block or title
        let theme_block = match &self.block {
            Some(block) => block.clone(),
            None => Block::default().title(self.title.clone()),
        };

        // Create the editor view with a mutable reference to our state
        let mut view = EditorView::new(&mut self.state);

        // Create the theme
        let theme = EditorTheme::default().block(theme_block);

        // Set theme options and word wrap
        view = view.theme(theme).wrap(true);

        // Render the view - this will update the viewport and handle scrolling
        view.render(area, buf);
    }
}
