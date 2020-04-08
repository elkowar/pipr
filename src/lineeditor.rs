use itertools::Itertools;

#[derive(Debug)]
pub struct EditorState {
    content: Vec<String>,
    pub cursor_col: usize,
}

pub enum EditorEvent {
    NewCharacter(String),
    Backspace,
    Delete,
    GoLeft,
    GoRight,
    KillWordBack,
}

impl EditorState {
    pub fn new() -> EditorState {
        EditorState {
            content: Vec::new(),
            cursor_col: 0,
        }
    }

    pub fn content_str(&self) -> String { self.content.join("") }

    pub fn apply_event(&mut self, event: EditorEvent) {
        match event {
            EditorEvent::NewCharacter(input) => {
                self.content.insert(self.cursor_col, input);
                self.cursor_col += 1;
            }
            EditorEvent::GoLeft if self.cursor_col > 0 => {
                self.cursor_col -= 1;
            }
            EditorEvent::GoRight if self.cursor_col < self.content.len() => {
                self.cursor_col += 1;
            }
            EditorEvent::Backspace if self.cursor_col > 0 => {
                self.content.remove(self.cursor_col - 1);
                self.cursor_col -= 1;
            }
            EditorEvent::Delete if self.cursor_col < self.content.len() => {
                self.content.remove(self.cursor_col);
            }
            EditorEvent::KillWordBack if !self.content.is_empty() => {
                let mut i = self.content.len() - 1;
                while let Some(c) = self.content.clone().get(i) {
                    self.content.remove(i);
                    self.cursor_col -= 1;
                    if c == " " || i == 0 {
                        break;
                    };
                    i -= 1;
                }
            }
            _ => {}
        }
    }
}
