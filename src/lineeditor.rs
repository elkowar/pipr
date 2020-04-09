use super::bookmark::*;

#[derive(Debug, Clone)]
pub struct EditorState {
    content: Vec<String>,
    pub cursor_col: usize,
}
pub enum EditorEvent {
    NewCharacter(char),
    Backspace,
    Delete,
    GoLeft,
    GoRight,
    Home,
    End,
    KillWordBack,
}

impl EditorState {
    pub fn new() -> EditorState {
        EditorState {
            content: Vec::new(),
            cursor_col: 0,
        }
    }

    pub fn content_to_bookmark(&self) -> Bookmark { Bookmark::new(self.content.to_owned()) }

    pub fn load_bookmark(&mut self, bookmark: Bookmark) {
        self.content = bookmark.content;
        self.cursor_col = self.content.len();
    }

    pub fn content_str(&self) -> String { self.content.join("") }
    pub fn displayed_cursor_column(&self) -> usize {
        self.content
            .iter()
            .take(self.cursor_col)
            .map(|elem| elem.clone())
            .collect::<Vec<String>>()
            .join("")
            .chars()
            .filter_map(unicode_width::UnicodeWidthChar::width)
            .sum::<usize>()
    }

    fn next_char_index(&self) -> usize {
        let mut next_cursor = self.cursor_col;
        while let None = self.content_str().get(next_cursor..next_cursor + 1) {
            next_cursor += 1;
        }
        next_cursor
    }

    pub fn apply_event(&mut self, event: EditorEvent) {
        match event {
            EditorEvent::NewCharacter(c) => {
                self.content.insert(self.cursor_col, c.to_string());
                self.cursor_col += 1
            }
            EditorEvent::Backspace if self.cursor_col > 0 => {
                self.content.remove(self.cursor_col - 1);
                self.cursor_col -= 1
            }
            EditorEvent::Delete if self.cursor_col < self.content.len() => {
                self.content.remove(self.cursor_col);
            }
            EditorEvent::GoLeft if self.cursor_col > 0 => {
                self.cursor_col -= 1;
            }
            EditorEvent::GoRight if self.cursor_col < self.content.len() => {
                self.cursor_col += 1;
            }
            EditorEvent::Home => {
                self.cursor_col = 0;
            }
            EditorEvent::End => {
                self.cursor_col = self.content.len();
            }
            EditorEvent::KillWordBack if !self.content.is_empty() => {
                let mut i = self.content.len() - 1;
                while let Some(c) = self.content.clone().get(i) {
                    self.content.remove(i);
                    self.cursor_col -= 1;
                    if c == " " || c == "/" || c == "\\" || c == ":" || c == "_" || c == "-" || i == 0 {
                        break;
                    };
                    i -= 1;
                }
            }
            _ => {}
        }
    }
}
