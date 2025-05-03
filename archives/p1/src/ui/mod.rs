mod input;
mod keybindings;
mod message;
//mod terminal;

pub use input::Input;
pub use keybindings::{KeyBindings, Mode};
pub use message::{Message, MessageType};
//pub use terminal::Terminal;

#[allow(dead_code)]
pub struct App {
    pub input: Input,
    pub messages: Vec<Message>,
    pub key_bindings: KeyBindings,
    pub mode: Mode,
    pub running: bool,
}

#[allow(dead_code)]
impl App {
    pub fn new() -> Self {
        Self {
            input: Input::default(),
            messages: Vec::new(),
            key_bindings: KeyBindings::default(),
            mode: Mode::Normal,
            running: true,
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            Mode::Normal => Mode::Insert,
            Mode::Insert => Mode::Normal,
        };
    }

    pub fn add_message(&mut self, content: String, message_type: MessageType) {
        self.messages.push(Message::new(content, message_type));
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
