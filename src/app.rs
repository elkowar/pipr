use super::bookmark::BookmarkList;
use super::command_evaluation::*;
use super::lineeditor as le;
use num_traits::FromPrimitive;

use crossterm::event::{KeyCode, KeyModifiers};

#[derive(PartialEq, PartialOrd, Eq, Ord, FromPrimitive, Clone, Copy, Debug)]
pub enum UIArea {
    CommandInput,
    Config,
    BookmarkList,
}

impl UIArea {
    fn next_area(&self) -> UIArea {
        match FromPrimitive::from_u8(*self as u8 + 1) {
            Some(next) => next,
            None => FromPrimitive::from_u8(0).unwrap(),
        }
    }
    fn prev_area(&self) -> UIArea {
        if *self as u8 == 0 {
            FromPrimitive::from_u8(2).unwrap()
        } else {
            FromPrimitive::from_u8(*self as u8 - 1).unwrap()
        }
    }
}

pub struct App {
    pub selected_area: UIArea,
    pub input_state: le::EditorState,
    pub command_output: String,
    pub command_error: String,
    pub autoeval_mode: bool,
    pub bookmarks: BookmarkList,
    pub last_unsaved: Option<String>,
    pub selected_bookmark_idx: Option<usize>,
    pub executor: Executor,
    pub should_quit: bool,
}

impl App {
    pub fn new(executor: Executor) -> App {
        App {
            selected_area: UIArea::CommandInput,
            input_state: le::EditorState::new(),
            command_output: "".into(),
            command_error: "".into(),
            autoeval_mode: false,
            bookmarks: BookmarkList::new(),
            last_unsaved: None,
            selected_bookmark_idx: None,
            executor,
            should_quit: false,
        }
    }

    fn eval_input(&mut self) {
        self.executor.execute(&self.input_state.content_str());
    }

    fn toggle_bookmarked(&mut self) {
        self.bookmarks.toggle_bookmark(self.input_state.content_to_bookmark());
    }

    fn command_input_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let previous_content = self.input_state.content_str().clone();
        match code {
            KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => self.toggle_bookmarked(),
            KeyCode::Char('z') if modifiers.contains(KeyModifiers::CONTROL) => {
                //self.last_unsaved.clone().map(|x| self.input_state.set_content(&x));
            }

            KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.input_state.apply_event(le::EditorEvent::KillWordBack)
            }
            KeyCode::Char('\r') | KeyCode::Char('\n') if modifiers.contains(KeyModifiers::ALT) => {
                self.input_state.apply_event(le::EditorEvent::NewLine)
            }
            KeyCode::Char(c) => self.input_state.apply_event(le::EditorEvent::NewCharacter(c)),
            KeyCode::Backspace => self.input_state.apply_event(le::EditorEvent::Backspace),
            KeyCode::Delete => self.input_state.apply_event(le::EditorEvent::Delete),

            KeyCode::Left => self.input_state.apply_event(le::EditorEvent::GoLeft),
            KeyCode::Right => self.input_state.apply_event(le::EditorEvent::GoRight),
            KeyCode::Up => self.input_state.apply_event(le::EditorEvent::GoUp),
            KeyCode::Down => self.input_state.apply_event(le::EditorEvent::GoDown),
            KeyCode::Home => self.input_state.apply_event(le::EditorEvent::Home),
            KeyCode::End => self.input_state.apply_event(le::EditorEvent::End),
            KeyCode::Enter => self.eval_input(),
            _ => {}
        }

        if previous_content != self.input_state.content_str() && self.autoeval_mode {
            self.eval_input();
        }
    }

    fn config_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Enter => self.autoeval_mode = !self.autoeval_mode,
            _ => {}
        }
    }

    fn bookmarklist_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(idx) = self.selected_bookmark_idx {
                    self.selected_bookmark_idx = Some((idx + 1) % self.bookmarks.len() as usize);
                } else {
                    self.last_unsaved = Some(self.input_state.content_str());
                    self.selected_bookmark_idx = Some(0);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(idx) = self.selected_bookmark_idx {
                    self.selected_bookmark_idx = Some((idx - 1).max(0) as usize);
                } else {
                    self.last_unsaved = Some(self.input_state.content_str());
                    self.selected_bookmark_idx = Some(0);
                }
            }
            KeyCode::Enter => {
                if let Some(bookmark) = self
                    .selected_bookmark_idx
                    .and_then(|idx| self.bookmarks.bookmark_at(idx))
                    .cloned()
                {
                    self.input_state.load_bookmark(&bookmark);
                }
            }
            _ => {}
        }
    }

    pub fn apply_cmd_output(&mut self, (stdout, stderr): (String, String)) {
        if stderr.is_empty() {
            self.command_output = stdout;
        }
        self.command_error = stderr;
    }

    pub fn apply_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if code == KeyCode::Tab {
            self.selected_area = self.selected_area.next_area();
        } else if code == KeyCode::BackTab {
            self.selected_area = self.selected_area.prev_area();
        } else {
            match self.selected_area {
                UIArea::CommandInput => self.command_input_event(code, modifiers),
                UIArea::Config => self.config_event(code),
                UIArea::BookmarkList => self.bookmarklist_event(code),
            }
        }
    }
}
