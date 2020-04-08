use itertools::Itertools;
use unicode_segmentation::*;
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

    pub fn set_content(&mut self, new_content: &str) {
        //self.content = UnicodeSegmentation::graphemes(new_content, true)
        //.map(|x| x.to_string())
        //.collect::<Vec<String>>();
        //self.cursor_col = new_content.len();
        unimplemented!();
    }

    pub fn content_str(&self) -> String { self.content.to_owned() }

    pub fn displayed_cursor_column(&self) -> usize {
        self.content
            .chars()
            .take(self.cursor_col)
            .filter_map(unicode_width::UnicodeWidthChar::width)
            .sum::<usize>()
    }

    pub fn apply_event(&mut self, event: EditorEvent) {
        match event {
            EditorEvent::NewCharacter(c) => {
                let c_string = c.to_string();
                let mut grapheme_vec: Vec<&str> = self.content.graphemes(true).collect_vec();
                grapheme_vec.insert(self.cursor_col, &c_string);
                self.content = grapheme_vec.join("");
                self.cursor_col += 1;
            }
            EditorEvent::Backspace if self.cursor_col > 0 => {
                let mut grapheme_vec: Vec<&str> = self.content.graphemes(true).collect_vec();
                let deleted_grapheme_len = grapheme_vec[self.cursor_col - 1].len();
                grapheme_vec.remove(self.cursor_col - 1);
                self.content = grapheme_vec.join("");
                self.cursor_col -= deleted_grapheme_len;
            }
            EditorEvent::Delete if self.cursor_col < self.content.len() => {
                let mut grapheme_vec: Vec<&str> = self.content.graphemes(true).collect_vec();
                let deleted_grapheme_len = self
                    .content
                    .chars()
                    .skip(self.cursor_col)
                    .collect::<String>()
                    .graphemes(true)
                    .take(1)
                    .join("")
                    .len();
                self.content = grapheme_vec.join("");
                self.cursor_col -= deleted_grapheme_len;
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
                //let mut i = self.content.len() - 1;
                //while let Some(c) = self.content.clone().get(i) {
                //self.content.remove(i);
                //self.cursor_col -= 1;
                //if c == " " || c == "/" || c == "\\" || c == ":" || c == "_" || c == "-" || i == 0 {
                //break;
                //};
                //i -= 1;
                //}
                unimplemented!()
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
    pub fn test_lineeditor_umlaut() {
        let mut le = EditorState::new();
        assert_eq!(le.content_str(), "");

        le.apply_event(EditorEvent::NewCharacter('ä'));
        assert_eq!(le.content_str(), "ä");
        dbg!(le.content_str().graphemes(true).collect_vec());
        le.apply_event(EditorEvent::NewCharacter('ä'));
        assert_eq!(le.content_str(), "ää");
        assert_eq!(le.displayed_cursor_column(), 2);
    }
}
