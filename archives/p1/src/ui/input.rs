#[allow(dead_code)]
pub struct Input {
    pub content: String,
    pub cursor_position: usize,
}

#[allow(dead_code)]
impl Input {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_position, c);
        self.cursor_position += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.content.remove(self.cursor_position);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.content.len() {
            self.cursor_position += 1;
        }
    }

    pub fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    pub fn move_cursor_to_end(&mut self) {
        self.cursor_position = self.content.len();
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_position = 0;
    }

    pub fn submit(&mut self) -> String {
        let content = self.content.clone();
        self.clear();
        content
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
