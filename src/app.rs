use super::command_evaluation::*;
use super::commandlist::{CommandEntry, CommandList};
use super::lineeditor::*;
use super::pipr_config::*;

use crossterm::event::{KeyCode, KeyModifiers};

pub const HELP_TEXT: &str = "\
F1         Show/hide help
F2         Toggle autoeval
Ctrl+B     Show/hide bookmarks
Ctrl+S     Save bookmark
Alt+Return Newline
Ctrl+X     Clear Command
Ctrl+P     Previous in history
Ctrl+N     Next in history

Config file is in
~/.config/pipr/pipr.toml";

pub struct CommandListState {
    pub list: Vec<CommandEntry>,
    pub selected_idx: Option<usize>,
}

pub enum WindowState {
    Main,
    TextView(String, String),
    BookmarkList(CommandListState),
    HistoryList(CommandListState),
}

pub struct App {
    pub input_state: EditorState,
    pub command_output: String,
    pub command_error: String,
    pub autoeval_mode: bool,
    pub window_state: WindowState,
    pub bookmarks: CommandList,
    pub history: CommandList,
    pub history_idx: Option<usize>,
    pub executor: Executor,
    pub config: PiprConfig,
    pub should_quit: bool,
}

impl App {
    pub fn new(executor: Executor, config: PiprConfig, bookmarks: CommandList, history: CommandList) -> App {
        App {
            window_state: WindowState::Main,
            input_state: EditorState::new(),
            command_output: "".into(),
            command_error: "".into(),
            autoeval_mode: false,
            should_quit: false,
            history_idx: None,
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
                //self.sidebar_content = SidebarContent::BookmarkList;
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

    pub fn main_window_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('q') | KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => self.should_quit = true,
            KeyCode::F(2) => self.autoeval_mode = !self.autoeval_mode,
            _ => self.command_input_event(code, modifiers),
        }
    }

    pub fn on_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::F(1) => self.window_state = WindowState::TextView("Help".to_string(), HELP_TEXT.to_string()),
            KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.history.push(self.input_state.content_to_commandentry());

                let entries = self.bookmarks.entries.clone();
                self.window_state = WindowState::BookmarkList(CommandListState {
                    selected_idx: if entries.len() == 0 { None } else { Some(0) },
                    list: entries,
                })
            }
            KeyCode::Char('h') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.history.push(self.input_state.content_to_commandentry());

                let entries = self.history.entries.clone();
                self.window_state = WindowState::HistoryList(CommandListState {
                    selected_idx: self.history_idx.or(if self.history.len() > 0 { Some(0) } else { None }),
                    list: entries,
                })
            }
            _ => {
                let window_state = &mut self.window_state;
                match window_state {
                    WindowState::Main => self.main_window_tui_event(code, modifiers),
                    WindowState::TextView(_, _) => self.window_state = WindowState::Main,
                    WindowState::BookmarkList(state) => match code {
                        KeyCode::Esc => self.window_state = WindowState::Main,
                        KeyCode::Enter => {
                            if let Some(entry) = state.selected_entry() {
                                self.input_state.load_commandentry(entry);
                            }
                            self.window_state = WindowState::Main;
                        }
                        _ => state.apply_event(code),
                    },
                    WindowState::HistoryList(state) => match code {
                        KeyCode::Esc => self.window_state = WindowState::Main,
                        KeyCode::Enter => {
                            if let Some(idx) = state.selected_idx {
                                if let Some(entry) = self.history.get_at(idx) {
                                    self.input_state.load_commandentry(entry);
                                }
                            }
                            self.history_idx = state.selected_idx;
                            self.window_state = WindowState::Main;
                        }
                        _ => state.apply_event(code),
                    },
                }
            }
        }
    }
}

impl CommandListState {
    pub fn selected_entry(&self) -> Option<&CommandEntry> {
        self.selected_idx.and_then(|idx| self.list.get(idx))
    }

    pub fn apply_event(&mut self, code: KeyCode) {
        if let Some(selected_idx) = self.selected_idx {
            match code {
                KeyCode::Up | KeyCode::Char('k') if selected_idx > 0 => self.selected_idx = Some(selected_idx - 1),
                KeyCode::Down | KeyCode::Char('j') if selected_idx < self.list.len() - 1 => {
                    self.selected_idx = Some(selected_idx + 1)
                }
                _ => {}
            }
        }
    }
}
