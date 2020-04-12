use super::command_evaluation::*;
use super::commandlist::CommandList;
use super::lineeditor::*;
use super::pipr_config::*;

use crossterm::event::{KeyCode, KeyModifiers};

#[derive(PartialEq, PartialOrd, Eq, Ord, FromPrimitive, Clone, Copy, Debug)]
pub enum UIArea {
    CommandInput,
    BookmarkList,
}

#[derive(PartialEq)]
pub enum SidebarContent {
    BookmarkList,
    Help,
    Nothing,
}

pub struct App {
    pub selected_area: UIArea,
    pub input_state: EditorState,
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
    pub sidebar_content: SidebarContent,
}

impl App {
    pub fn new(executor: Executor, config: PiprConfig, bookmarks: CommandList, history: CommandList) -> App {
        App {
            selected_area: UIArea::CommandInput,
            input_state: EditorState::new(),
            command_output: "".into(),
            command_error: "".into(),
            autoeval_mode: false,
            selected_bookmark_idx: None,
            should_quit: false,
            history_idx: None,
            sidebar_content: if config.show_help {
                SidebarContent::Help
            } else {
                SidebarContent::Nothing
            },
            executor,
            config,
            bookmarks,
            history,
        }
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
            KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.sidebar_content = SidebarContent::BookmarkList;
                self.bookmarks.toggle_entry(self.input_state.content_to_commandentry());
            }
            KeyCode::Char('p') if modifiers.contains(KeyModifiers::CONTROL) => self.apply_history_prev(),
            KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => self.apply_history_next(),

            KeyCode::Left => self.input_state.apply_event(EditorEvent::GoLeft),
            KeyCode::Right => self.input_state.apply_event(EditorEvent::GoRight),
            KeyCode::Up => self.input_state.apply_event(EditorEvent::GoUp),
            KeyCode::Down => self.input_state.apply_event(EditorEvent::GoDown),
            KeyCode::Home => self.input_state.apply_event(EditorEvent::Home),
            KeyCode::End => self.input_state.apply_event(EditorEvent::End),
            KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => self.input_state.apply_event(EditorEvent::Home),
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => self.input_state.apply_event(EditorEvent::End),

            KeyCode::Char('x') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.history.push(self.input_state.content_to_commandentry());
                self.input_state.apply_event(EditorEvent::Clear);
            }
            KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.input_state.apply_event(EditorEvent::KillWordBack)
            }
            KeyCode::Char('\r') | KeyCode::Char('\n') if modifiers.contains(KeyModifiers::ALT) => {
                self.input_state.apply_event(EditorEvent::NewLine)
            }

            KeyCode::Char(c) => self.input_state.apply_event(EditorEvent::NewCharacter(c)),
            KeyCode::Backspace => self.input_state.apply_event(EditorEvent::Backspace),
            KeyCode::Delete => self.input_state.apply_event(EditorEvent::Delete),

            KeyCode::Enter => {
                if (self.history.len() == 0
                    || self.history.get_at(self.history.len() - 1) != Some(&self.input_state.content_to_commandentry()))
                    && !self.input_state.content_str().is_empty()
                {
                    self.history.push(self.input_state.content_to_commandentry());
                }
                self.executor.execute(&self.input_state.content_str());
            }
            _ => {}
        }

        if previous_content != self.input_state.content_str() && self.autoeval_mode {
            self.executor.execute(&self.input_state.content_str());
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

    pub fn on_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('q') | KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => self.should_quit = true,

            KeyCode::Tab | KeyCode::BackTab => {
                self.selected_area = match self.selected_area {
                    UIArea::CommandInput if self.sidebar_content == SidebarContent::BookmarkList => UIArea::BookmarkList,
                    _ => UIArea::CommandInput,
                }
            }
            KeyCode::F(2) => self.autoeval_mode = !self.autoeval_mode,
            KeyCode::F(1) => {
                self.sidebar_content = match self.sidebar_content {
                    SidebarContent::Help => SidebarContent::Nothing,
                    _ => {
                        self.selected_bookmark_idx = None;
                        SidebarContent::Help
                    }
                }
            }
            KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.sidebar_content = match self.sidebar_content {
                    SidebarContent::BookmarkList => {
                        self.selected_area = UIArea::CommandInput;
                        self.selected_bookmark_idx = None;
                        SidebarContent::Nothing
                    }
                    _ => {
                        self.selected_area = UIArea::BookmarkList;
                        SidebarContent::BookmarkList
                    }
                }
            }

            _ => match self.selected_area {
                UIArea::CommandInput => self.command_input_event(code, modifiers),
                UIArea::BookmarkList => self.bookmarklist_event(code),
            },
        }
    }
}
