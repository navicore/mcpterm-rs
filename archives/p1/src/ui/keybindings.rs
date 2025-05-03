use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

#[derive(PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Quit,
    ToggleMode,
    Submit,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorToStart,
    MoveCursorToEnd,
    DeleteChar,
    ClearInput,
    ScrollUp,
    ScrollDown,
}

#[allow(dead_code)]
pub struct KeyBindings {
    normal_bindings: HashMap<KeyEvent, Action>,
    insert_bindings: HashMap<KeyEvent, Action>,
}

#[allow(dead_code)]
impl KeyBindings {
    pub fn new() -> Self {
        let mut normal_bindings = HashMap::new();
        let mut insert_bindings = HashMap::new();

        // Normal mode (Vi-like) bindings
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            Action::Quit,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            Action::ToggleMode,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            Action::ToggleMode,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            Action::MoveCursorLeft,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
            Action::MoveCursorRight,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
            Action::MoveCursorToStart,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
            Action::MoveCursorToEnd,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
            Action::DeleteChar,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            Action::ClearInput,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            Action::ScrollUp,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            Action::ScrollDown,
        );
        normal_bindings.insert(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Action::Submit,
        );

        // Insert mode bindings
        insert_bindings.insert(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            Action::ToggleMode,
        );
        insert_bindings.insert(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            Action::DeleteChar,
        );
        insert_bindings.insert(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            Action::MoveCursorLeft,
        );
        insert_bindings.insert(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            Action::MoveCursorRight,
        );
        insert_bindings.insert(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            Action::MoveCursorToStart,
        );
        insert_bindings.insert(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            Action::MoveCursorToEnd,
        );
        insert_bindings.insert(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Action::Submit,
        );
        insert_bindings.insert(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Action::Quit,
        );

        Self {
            normal_bindings,
            insert_bindings,
        }
    }

    pub fn get_action(&self, key: KeyEvent, mode: Mode) -> Option<Action> {
        match mode {
            Mode::Normal => self.normal_bindings.get(&key).copied(),
            Mode::Insert => self.insert_bindings.get(&key).copied(),
        }
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::new()
    }
}
