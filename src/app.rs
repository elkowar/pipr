use super::command_evaluation::*;
use super::commandlist::CommandList;
use super::lineeditor as le;
use super::pipr_config::*;
use num_traits::FromPrimitive;

use crossterm::event::{KeyCode, KeyModifiers};

#[derive(PartialEq, PartialOrd, Eq, Ord, FromPrimitive, Clone, Copy, Debug)]
pub enum UIArea {
    CommandInput,
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
    pub bookmarks: CommandList,
    pub selected_bookmark_idx: Option<usize>,
    pub executor: Executor,
    pub config: PiprConfig,
    pub should_quit: bool,
    pub history: CommandList,
    pub history_idx: Option<usize>,
    pub bookmarks_visible: bool,
}

impl App {
    pub fn new(executor: Executor, config: PiprConfig, bookmarks: CommandList, history: CommandList) -> App {
        App {
            selected_area: UIArea::CommandInput,
            input_state: le::EditorState::new(),
            command_output: "".into(),
            command_error: "".into(),
            autoeval_mode: false,
            selected_bookmark_idx: None,
            should_quit: false,
            history_idx: None,
            bookmarks_visible: true,
            executor,
            config,
            bookmarks,
            history,
        }
    }

    fn eval_input(&mut self) {
        self.executor.execute(&self.input_state.content_str());
    }

    fn toggle_bookmarked(&mut self) {
        self.bookmarks.toggle_entry(self.input_state.content_to_commandentry());
    }

    fn apply_history_prev(&mut self) {
        if let Some(idx) = self.history_idx {
            if idx > 0 {
                self.history_idx = Some(idx - 1);
                self.input_state.load_commandentry(&self.history.get_at(idx - 1).unwrap());
            }
        } else if self.history.len() > 0 {
            let new_idx = self.history.len() - 1;
            self.history_idx = Some(new_idx);
            self.history.push(self.input_state.content_to_commandentry());
            self.input_state.load_commandentry(&self.history.get_at(new_idx).unwrap());
        }
    }

    fn apply_history_next(&mut self) {
        if let Some(idx) = self.history_idx {
            let new_idx = idx + 1;
            if new_idx < self.history.len() - 1 {
                self.history_idx = Some(new_idx);
                self.input_state.load_commandentry(&self.history.get_at(new_idx).unwrap());
            } else {
                self.history_idx = None;
                self.input_state.set_content(&vec![String::new()]);
            }
        }
    }

    fn command_input_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let previous_content = self.input_state.content_str().clone();
        match code {
            KeyCode::F(1) => self.autoeval_mode = !self.autoeval_mode,
            KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => self.bookmarks_visible = !self.bookmarks_visible,
            KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => self.toggle_bookmarked(),
            KeyCode::Char('p') if modifiers.contains(KeyModifiers::CONTROL) => self.apply_history_prev(),
            KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => self.apply_history_next(),
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
            KeyCode::Enter => {
                if (self.history.len() == 0
                    || self.history.get_at(self.history.len() - 1) != Some(&self.input_state.content_to_commandentry()))
                    && !self.input_state.content_str().is_empty()
                {
                    self.history.push(self.input_state.content_to_commandentry());
                }
                self.eval_input();
            }
            _ => {}
        }

        if previous_content != self.input_state.content_str() && self.autoeval_mode {
            self.eval_input();
        }
    }

    fn bookmarklist_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Down | KeyCode::Char('j') if self.bookmarks.len() > 0 => {
                if let Some(idx) = self.selected_bookmark_idx {
                    self.selected_bookmark_idx = Some((idx + 1) % self.bookmarks.len() as usize);
                } else {
                    self.selected_bookmark_idx = Some(0);
                }
            }
            KeyCode::Up | KeyCode::Char('k') if self.bookmarks.len() > 0 => {
                if let Some(idx) = self.selected_bookmark_idx {
                    if idx > 0 {
                        self.selected_bookmark_idx = Some((idx - 1).max(0) as usize);
                    }
                } else {
                    self.selected_bookmark_idx = Some(0);
                }
            }
            KeyCode::Enter => {
                if let Some(bookmark) = self.selected_bookmark_idx.and_then(|idx| self.bookmarks.get_at(idx)).cloned() {
                    self.input_state.load_commandentry(&bookmark);
                }
            }
            KeyCode::Delete => {
                if let Some(idx) = self.selected_bookmark_idx {
                    self.bookmarks.remove_at(idx);
                    if self.bookmarks.len() == 0 {
                        self.selected_bookmark_idx = None;
                    } else {
                        if self.bookmarks.get_at(idx).is_none() {
                            self.selected_bookmark_idx = Some(idx - 1);
                        }
                    }
                }
            }

            _ => {}
        }
    }

    pub fn on_cmd_output(&mut self, process_result: ProcessResult) {
        match process_result {
            ProcessResult::Ok(stdout) => {
                self.command_output = stdout;
                self.command_error = String::new();
            }
            ProcessResult::NotOk(stderr) => {
                self.command_error = stderr;
            }
        }
    }

    pub fn apply_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::Tab => self.selected_area = self.selected_area.next_area(),
            KeyCode::BackTab => self.selected_area = self.selected_area.prev_area(),
            _ => match self.selected_area {
                UIArea::CommandInput => self.command_input_event(code, modifiers),
                UIArea::BookmarkList => self.bookmarklist_event(code),
            },
        }
    }
}
