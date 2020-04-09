use super::bookmark::*;
use unicode_width::*;

#[derive(Debug, Clone)]
pub struct EditorState {
    content: String,
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
            content: String::new(),
            cursor_col: 0,
        }
    }

    pub fn content_to_bookmark(&self) -> Bookmark { Bookmark::new(&self.content) }

    pub fn load_bookmark(&mut self, bookmark: Bookmark) { self.content = bookmark.content; }

    pub fn set_content(&mut self, new_content: &str) {
        self.content = new_content.to_owned();
        self.cursor_col = self.content.len();
    }

    pub fn content_str(&self) -> String { self.content.to_owned() }
    pub fn displayed_cursor_column(&self) -> usize { UnicodeWidthStr::width(&self.content[..self.cursor_col]) }

    fn next_char_index(&self) -> usize {
        if self.cursor_col == self.content.len() {
            return self.cursor_col;
        }
        let mut new_cursor = self.cursor_col + 1;
        while let None = self.content_str().get(new_cursor..) {
            new_cursor += 1;
        }
        new_cursor
    }

    fn prev_char_index(&self) -> usize {
        if self.cursor_col == 0 {
            return 0;
        }
        let mut new_cursor = self.cursor_col - 1;
        while let None = self.content_str().get(new_cursor..) {
            new_cursor -= 1;
        }
        new_cursor
    }

    pub fn apply_event(&mut self, event: EditorEvent) {
        match event {
            EditorEvent::NewCharacter(c) => {
                self.content.insert(self.cursor_col, c);
                self.cursor_col = self.next_char_index();
            }
            EditorEvent::Backspace if self.cursor_col > 0 => {
                let new_cursor = self.prev_char_index();
                self.content.remove(new_cursor);
                self.cursor_col = new_cursor;
            }
            EditorEvent::Delete if self.cursor_col < self.content.len() => {
                self.content.remove(self.cursor_col);
            }
            EditorEvent::GoLeft if self.cursor_col > 0 => {
                self.cursor_col = self.prev_char_index();
            }
            EditorEvent::GoRight if self.cursor_col < self.content.len() => {
                self.cursor_col = self.next_char_index();
            }
            EditorEvent::Home => {
                self.cursor_col = 0;
            }
            EditorEvent::End => {
                self.cursor_col = self.content.len();
            }
            EditorEvent::KillWordBack if !self.content.is_empty() => {
                while let Some(c) = self.content.to_owned().get(self.prev_char_index()..self.cursor_col) {
                    self.cursor_col = self.prev_char_index();
                    self.content.remove(self.cursor_col);
                    if c == " " || c == "/" || c == "\\" || c == ":" || c == "_" || c == "-" || self.cursor_col == 0 {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}

pub mod test {
    #[allow(unused_imports)]
    use super::*;
    #[test]
    pub fn test_lineeditor_ascii() {
        let mut le = EditorState::new();
        assert_eq!(le.content_str(), "");

        le.apply_event(EditorEvent::NewCharacter('a'));
        assert_eq!(le.content_str(), "a");

        le.apply_event(EditorEvent::NewCharacter('a'));
        assert_eq!(le.content_str(), "aa");
        assert_eq!(le.displayed_cursor_column(), 2);

        le.apply_event(EditorEvent::Backspace);
        assert_eq!(le.content_str(), "a");
        assert_eq!(le.displayed_cursor_column(), 1);

        le.apply_event(EditorEvent::Backspace);
        assert_eq!(le.content_str(), "");
        assert_eq!(le.displayed_cursor_column(), 0);

        le.apply_event(EditorEvent::Backspace);
        assert_eq!(le.content_str(), "");
        assert_eq!(le.displayed_cursor_column(), 0);

        le.apply_event(EditorEvent::NewCharacter('a'));
        assert_eq!(le.content_str(), "a");
        assert_eq!(le.displayed_cursor_column(), 1);

        le.apply_event(EditorEvent::GoLeft);
        assert_eq!(le.displayed_cursor_column(), 0);

        le.apply_event(EditorEvent::Delete);
        assert_eq!(le.content_str(), "");
        assert_eq!(le.displayed_cursor_column(), 0);

        le.apply_event(EditorEvent::Delete);
        assert_eq!(le.content_str(), "");
        assert_eq!(le.displayed_cursor_column(), 0);
    }

    #[test]
    pub fn test_advanced() {
        let mut le = EditorState::new();
        le.set_content("as");
        assert_eq!(le.content_str(), "as");
        assert_eq!(le.displayed_cursor_column(), 2 as usize);

        le.apply_event(EditorEvent::KillWordBack);
        assert_eq!(le.content_str(), "");
        assert_eq!(le.displayed_cursor_column(), 0 as usize);

        le.set_content("as as as");
        assert_eq!(le.content_str(), "as as as");
        assert_eq!(le.displayed_cursor_column(), 8 as usize);

        le.apply_event(EditorEvent::KillWordBack);
        assert_eq!(le.content_str(), "as as");
        assert_eq!(le.displayed_cursor_column(), 5 as usize);
    }

    #[test]
    pub fn test_lineeditor_umlaut() {
        let mut le = EditorState::new();
        assert_eq!(le.content_str(), "");

        le.apply_event(EditorEvent::NewCharacter('ä'));
        assert_eq!(le.content_str(), "ä");
        assert_eq!(le.displayed_cursor_column(), 1);
        le.apply_event(EditorEvent::NewCharacter('ä'));
        assert_eq!(le.content_str(), "ää");
        assert_eq!(le.displayed_cursor_column(), 2);

        le.apply_event(EditorEvent::GoLeft);
        assert_eq!(le.displayed_cursor_column(), 1);
    }
}
