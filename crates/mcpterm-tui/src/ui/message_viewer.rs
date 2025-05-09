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
use tracing::{debug, warn};

use crate::state::{Message, MessageType};

/// Result type for message viewer key handling
pub enum HandleResult {
    /// Continue processing
    Continue,
    /// Copy selected text to clipboard
    Copy(String),
    /// Abort/Cancel the operation
    Abort,
}

/// A read-only message viewer component based on edtui
#[derive(Clone)]
pub struct MessageViewer {
    pub state: EditorState,
    event_handler: EditorEventHandler,
    pub title: String,
    pub block: Option<Block<'static>>,
    // Track if we've styled this content already
    styled_content_id: Option<String>,
}

impl Default for MessageViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageViewer {
    /// Create a new MessageViewer
    pub fn new() -> Self {
        let mut state = EditorState::default();
        let event_handler = EditorEventHandler::default();
        
        // Always start in normal mode
        state.mode = EditorMode::Normal;

        Self {
            state,
            event_handler,
            title: "Messages".to_string(),
            block: None,
            styled_content_id: None,
        }
    }
    
    /// Set the editor mode
    pub fn set_mode(&mut self, mode: EditorMode) {
        self.state.mode = mode;
    }

    /// Set the content of the viewer from messages
    pub fn set_content(&mut self, messages: &[Message]) {
        // Check if we have the same content already
        // This is a simple optimization to avoid unnecessary re-rendering
        let first_id = messages.first().map(|m| m.id.clone());
        let last_id = messages.last().map(|m| m.id.clone());
        let content_id = match (first_id, last_id) {
            (Some(first), Some(last)) => format!("{}:{}", first, last),
            _ => "empty".to_string(),
        };

        let new_content = if !messages.is_empty() {
            // Format all messages into a single string
            let mut content = String::new();
            for message in messages {
                // Add message header based on type
                let header = match message.message_type {
                    MessageType::User => "You: ",
                    MessageType::Assistant => "Assistant: ",
                    MessageType::System => "System: ",
                    MessageType::Tool => "Tool: ",
                    MessageType::Error => "Error: ",
                };

                // Format timestamp
                let timestamp = message.timestamp.format("%H:%M:%S");

                // Add formatted message with header and timestamp
                if !content.is_empty() {
                    content.push_str("\n\n");
                }
                
                content.push_str(&format!("[{}] {}\n", timestamp, header));
                content.push_str(&message.content);
            }
            content
        } else {
            String::new()
        };

        // If content has changed, update the editor state
        if self.styled_content_id.as_ref() != Some(&content_id) {
            self.state = EditorState::new(Lines::from(new_content));
            
            // Start in normal mode (read-only is handled through key event handling)
            self.state.mode = EditorMode::Normal;
            
            // Style the content
            self.apply_message_styles(messages);
            
            // Track that we've styled this content
            self.styled_content_id = Some(content_id);

            // Set cursor to beginning
            self.state.cursor = edtui::Index2::new(0, 0);
        }
    }

    /// Apply styles to messages based on their type
    fn apply_message_styles(&mut self, messages: &[Message]) {
        self.clear_all_styles();
        
        // If there are no messages, nothing to style
        if messages.is_empty() {
            return;
        }

        // Get all lines for easier processing
        let text = self.get_text();
        let lines: Vec<&str> = text.lines().collect();
        
        // Keep track of line numbers for styling
        let mut current_line = 0;
        
        for message in messages {
            // Style message headers differently based on message type
            let header_style = match message.message_type {
                MessageType::User => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                MessageType::Assistant => Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                MessageType::System => Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                MessageType::Tool => Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
                MessageType::Error => Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            };

            // Apply header style to the header line
            if current_line < lines.len() {
                let line = lines[current_line];
                let timestamp_end = line.find(']').unwrap_or(0) + 1;
                
                // Style timestamp part
                self.apply_style(
                    current_line,
                    0,
                    timestamp_end,
                    Style::default().fg(Color::DarkGray),
                    Some("timestamp".to_string()),
                );
                
                // Style the rest of the header
                if timestamp_end < line.len() {
                    self.apply_style(
                        current_line,
                        timestamp_end,
                        line.len(),
                        header_style,
                        Some("header".to_string()),
                    );
                }
            }
            
            // Move to message content (starts on next line)
            current_line += 1;
            
            // Style message content based on type
            let content_lines = message.content.lines().count();
            let content_style = match message.message_type {
                MessageType::User => Style::default().fg(Color::Yellow),
                MessageType::Assistant => Style::default().fg(Color::Green),
                MessageType::System => Style::default().fg(Color::Blue),
                MessageType::Tool => Style::default().fg(Color::Magenta),
                MessageType::Error => Style::default().fg(Color::Red),
            };
            
            // Apply style to each content line
            for i in 0..content_lines {
                if current_line + i < lines.len() {
                    let line = lines[current_line + i];
                    self.apply_style(
                        current_line + i,
                        0,
                        line.len(),
                        content_style,
                        Some(format!("content_{}", message.message_type as u8)),
                    );
                }
            }
            
            // Move past this message's content lines plus the blank line
            current_line += content_lines + 1;
        }
    }

    /// Handle a key event and return the result of the handling
    pub fn handle_key_event(&mut self, key: KeyEvent) -> HandleResult {
        // Special case for Ctrl+C to quit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return HandleResult::Abort;
        }
        
        // Visual mode and 'y' for yanking (copying) selected text
        if key.code == KeyCode::Char('y') && self.state.mode == EditorMode::Visual {
            if let Some(text) = self.get_selected_text() {
                if let Err(e) = self.yank_to_clipboard(&text) {
                    warn!("Failed to copy to clipboard: {}", e);
                }
                // Return to normal mode and clear selection
                self.state.mode = EditorMode::Normal;
                self.state.selection = None;
                return HandleResult::Copy(text);
            }
            // If no selection, clear and continue
            self.state.mode = EditorMode::Normal;
            self.state.selection = None;
            return HandleResult::Continue;
        }

        // Handle Ctrl+F (page down), Ctrl+B (page up)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
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

        // Convert crossterm::event::KeyEvent to ratatui::crossterm::event::KeyEvent
        let ratatui_key = ratatui::crossterm::event::KeyEvent {
            code: convert_key_code(key.code),
            modifiers: convert_key_modifiers(key.modifiers),
            kind: ratatui::crossterm::event::KeyEventKind::Press,
            state: ratatui::crossterm::event::KeyEventState::NONE,
        };

        // Pass the key event to the edtui event handler
        self.event_handler.on_key_event(ratatui_key, &mut self.state);

        HandleResult::Continue
    }

    /// Copy selected text to system clipboard
    fn yank_to_clipboard(&self, text: &str) -> Result<()> {
        // Create a new clipboard instance
        let mut clipboard = Clipboard::new()?;

        // Set the clipboard text
        clipboard.set_text(text)?;

        Ok(())
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

    /// Set the title of the viewer block
    pub fn title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    /// Set the block for the viewer
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
}

// Implement Widget for reference to MessageViewer to avoid cloning
impl Widget for &mut MessageViewer {
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
impl Widget for MessageViewer {
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