#[derive(Debug, Clone)]
pub struct EditorState {
    content: String,
    cursor_col: usize,
    pub display_cursor_col: usize,
}
pub enum EditorEvent {
    NewCharacter(char),
    Backspace,
    Delete,
    GoLeft,
    GoRight,
    KillWordBack,
}

impl EditorState {
    pub fn new() -> EditorState {
        EditorState {
            content: String::new(),
            cursor_col: 0,
            display_cursor_col: 0,
        }
    }

    pub fn content_str(&self) -> String { self.content.clone() }

    pub fn apply_event(&mut self, event: EditorEvent) {
        match event {
            EditorEvent::NewCharacter(c) => {
                self.content.insert(self.cursor_col, c);
                self.cursor_col = next_char(&self.content, self.cursor_col);
                self.display_cursor_col += 1
            }
            EditorEvent::GoLeft if self.cursor_col > 0 => {
                self.cursor_col = prev_char(&self.content, self.cursor_col);
                self.display_cursor_col -= 1
            }
            EditorEvent::GoRight if self.cursor_col < self.content.len() => {
                self.cursor_col = next_char(&self.content, self.cursor_col);
                self.display_cursor_col += 1
            }
            EditorEvent::Backspace if self.cursor_col > 0 => {
                self.cursor_col = prev_char(&self.content, self.cursor_col);
                self.content.remove(self.cursor_col);
                self.display_cursor_col -= 1
            }
            EditorEvent::Delete if self.cursor_col < self.content.len() => {
                self.content.remove(self.cursor_col);
            }
            EditorEvent::KillWordBack if !self.content.is_empty() => {
                //let mut i = self.content.len() - 1;
                //while let Some(c) = self.content.clone().get(i) {
                //self.content.remove(i);
                //self.cursor_col -= 1;
                //if c == " " || i == 0 {
                //break;
                //};
                //i -= 1;
                //}
            }
            _ => {}
        }
    }
}

fn next_char(s: &str, i: usize) -> usize {
    let s = s.as_bytes();
    if s[i] <= 0x7f {
        i + 1
    } else if s[i] >> 5 == 0b110 {
        i + 2
    } else if s[i] >> 4 == 0b1110 {
        i + 3
    } else {
        i + 4
    }
}

fn prev_char(s: &str, i: usize) -> usize {
    let s = s.as_bytes();
    let mut i = i;
    i -= 1;
    while s[i] & 0b1100_0000 == 0b1000_0000 {
        i -= 1
    }
    i
}
