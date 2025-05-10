use edtui::EditorMode;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

use crate::state::{AppState, FocusArea, ProcessingStatus};
use tracing::info;
use crate::ui::input_editor::InputEditor;
use crate::ui::message_viewer::MessageViewer;

pub mod input_editor;
pub mod message_viewer;

pub fn render(f: &mut ratatui::Frame, state: &mut AppState) {
    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    // Render message viewer
    render_messages(f, state, chunks[0]);

    // Render input editor
    render_input(f, state, chunks[1]);
}

/// Render the UI with provided editor instances
pub fn render_with_editors(
    f: &mut ratatui::Frame, 
    state: &mut AppState, 
    message_viewer: &mut MessageViewer,
    input_editor: &mut InputEditor
) {
    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    // Render message viewer
    render_messages_with_viewer(f, state, chunks[0], message_viewer);

    // Render input editor
    render_input_with_editor(f, state, chunks[1], input_editor);
}

fn render_messages(f: &mut ratatui::Frame, state: &mut AppState, area: Rect) {
    // Create a block with borders
    let auto_scroll_status = if state.auto_scroll { "AUTO" } else { "MANUAL" };
    let title = format!("Messages (scroll: {}/{} - {})", 
                      state.messages_scroll,
                      state.messages.len().saturating_sub(1),
                      auto_scroll_status);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if state.focus == FocusArea::Messages {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });

    // Get access to the message viewer
    static mut MESSAGE_VIEWER: Option<MessageViewer> = None;
    
    let message_viewer = unsafe {
        if MESSAGE_VIEWER.is_none() {
            info!("Initializing MESSAGE_VIEWER static");
            let mut viewer = MessageViewer::new();
            // Start in normal mode for navigation
            viewer.set_mode(EditorMode::Normal);
            MESSAGE_VIEWER = Some(viewer);
            info!("MESSAGE_VIEWER initialized");
        } else {
            info!("Using existing MESSAGE_VIEWER");
        }
        MESSAGE_VIEWER.as_mut().unwrap()
    };
    
    // Set block based on focus
    message_viewer.block = Some(block);
    
    // Apply messages with reverse order for scrolling (most recent at the bottom)
    let messages_offset = state.messages_scroll;
    let messages_to_show = if messages_offset >= state.messages.len() {
        &[]
    } else {
        &state.messages[0..state.messages.len() - messages_offset]
    };
    
    // Update content
    message_viewer.set_content(messages_to_show);
    
    // Debug info for troubleshooting
    info!("Rendering message viewer: mode={:?}, messages_count={}", 
         message_viewer.state.mode, messages_to_show.len());
         
    // Render the message viewer
    f.render_widget(message_viewer, area);
}

fn render_messages_with_viewer(f: &mut ratatui::Frame, state: &mut AppState, area: Rect, message_viewer: &mut MessageViewer) {
    // Create a block with borders
    let auto_scroll_status = if state.auto_scroll { "AUTO" } else { "MANUAL" };
    let title = format!("Messages (scroll: {}/{} - {})", 
                      state.messages_scroll,
                      state.messages.len().saturating_sub(1),
                      auto_scroll_status);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if state.focus == FocusArea::Messages {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });
    
    // Set block based on focus
    message_viewer.block = Some(block);
    
    // Apply messages with reverse order for scrolling (most recent at the bottom)
    let messages_offset = state.messages_scroll;
    let messages_to_show = if messages_offset >= state.messages.len() {
        &[]
    } else {
        &state.messages[0..state.messages.len() - messages_offset]
    };
    
    // Update content
    message_viewer.set_content(messages_to_show);
    
    // Debug info for troubleshooting
    info!("Rendering message viewer with local instance: mode={:?}, messages_count={}", 
         message_viewer.state.mode, messages_to_show.len());
         
    // Render the message viewer
    f.render_widget(message_viewer, area);
}

fn render_input(f: &mut ratatui::Frame, state: &mut AppState, area: Rect) {
    // Create more descriptive title based on processing status
    let title = match &state.processing {
        ProcessingStatus::Idle => {
            if state.editor_mode == crate::state::EditorMode::Normal {
                "Input (Normal Mode - press 'i' to type)".to_string()
            } else {
                "Input (Insert Mode - press Esc for normal mode)".to_string()
            }
        }
        ProcessingStatus::Connecting => "Input (Connecting...)".to_string(),
        ProcessingStatus::Processing { start_time, status } => {
            let elapsed = start_time.elapsed();
            format!(
                "Input ({} - {:?} elapsed)",
                status,
                std::time::Duration::from_secs(elapsed.as_secs())
            )
        }
        ProcessingStatus::Error(msg) => format!("Input (Error: {})", msg),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if state.focus == FocusArea::Input {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });

    // Get access to the input editor 
    static mut INPUT_EDITOR: Option<InputEditor> = None;
    
    let input_editor = unsafe {
        if INPUT_EDITOR.is_none() {
            info!("Initializing INPUT_EDITOR static");
            let mut editor = InputEditor::new();
            // Initialize properly
            editor.set_mode(EditorMode::Normal);
            INPUT_EDITOR = Some(editor);
            info!("INPUT_EDITOR initialized");
        } else {
            info!("Using existing INPUT_EDITOR");
        }
        INPUT_EDITOR.as_mut().unwrap()
    };
    
    // Set block based on focus
    input_editor.block = Some(block);
    
    // Set the current content if it's different from what's in the editor
    if input_editor.get_text() != state.input_content {
        input_editor.set_content(&state.input_content);
    }
    
    // Map our editor mode to edtui's editor mode and ensure it's set
    let edtui_mode = match state.editor_mode {
        crate::state::EditorMode::Normal => EditorMode::Normal,
        crate::state::EditorMode::Insert => EditorMode::Insert,
        crate::state::EditorMode::Visual => EditorMode::Visual,
    };
    input_editor.set_mode(edtui_mode);
    
    // Debug info for troubleshooting
    info!("Rendering input editor: mode={:?}, content={:?}", edtui_mode, state.input_content);
    
    // Render the input editor
    f.render_widget(input_editor, area);
}

fn render_input_with_editor(f: &mut ratatui::Frame, state: &mut AppState, area: Rect, input_editor: &mut InputEditor) {
    // Create more descriptive title based on processing status
    let title = match &state.processing {
        ProcessingStatus::Idle => {
            if state.editor_mode == crate::state::EditorMode::Normal {
                "Input (Normal Mode - press 'i' to type)".to_string()
            } else {
                "Input (Insert Mode - press Esc for normal mode)".to_string()
            }
        }
        ProcessingStatus::Connecting => "Input (Connecting...)".to_string(),
        ProcessingStatus::Processing { start_time, status } => {
            let elapsed = start_time.elapsed();
            format!(
                "Input ({} - {:?} elapsed)",
                status,
                std::time::Duration::from_secs(elapsed.as_secs())
            )
        }
        ProcessingStatus::Error(msg) => format!("Input (Error: {})", msg),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if state.focus == FocusArea::Input {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });
    
    // Set block based on focus
    input_editor.block = Some(block);
    
    // Set the current content if it's different from what's in the editor
    if input_editor.get_text() != state.input_content {
        input_editor.set_content(&state.input_content);
    }
    
    // Map our editor mode to edtui's editor mode and ensure it's set
    let edtui_mode = match state.editor_mode {
        crate::state::EditorMode::Normal => EditorMode::Normal,
        crate::state::EditorMode::Insert => EditorMode::Insert,
        crate::state::EditorMode::Visual => EditorMode::Visual,
    };
    input_editor.set_mode(edtui_mode);
    
    // Debug info for troubleshooting
    info!("Rendering input editor with local instance: mode={:?}, content={:?}", edtui_mode, state.input_content);
    
    // Render the input editor
    f.render_widget(input_editor, area);
}