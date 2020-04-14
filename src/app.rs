use super::command_evaluation::*;
use super::commandlist::{CommandEntry, CommandList};
use super::lineeditor::*;
use super::pipr_config::*;
use crossterm::event::{KeyCode, KeyModifiers};

pub const HELP_TEXT: &str = "\
F1         Show/hide help
F2         Toggle autoeval
F3         Toggle Paranoid history (fills up history in autoeval)
F4         Show/hide history
Ctrl+B     Show/hide bookmarks
Ctrl+S     Save bookmark
Alt+Return Newline
Ctrl+X     Clear Command
Ctrl+P     Previous in history
Ctrl+N     Next in history
Ctrl+V     Start snippet mode (press the key for your Snippet to choose)

disable a line by starting it with a #
this will simply exclude the line from the executed command.

Config file is in
~/.config/pipr/pipr.toml";

pub struct CommandListState {
    pub list: Vec<CommandEntry>,
    pub selected_idx: Option<usize>,
    recently_deleted: Vec<CommandEntry>,
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
    pub last_executed_cmd: String,
    pub paranoid_history_mode: bool,
    pub window_state: WindowState,
    pub bookmarks: CommandList,
    pub history: CommandList,
    pub history_idx: Option<usize>,
    pub executor: Executor,
    pub config: PiprConfig,
    pub should_quit: bool,
    pub snippet_mode: bool,
}

impl App {
    pub fn new(executor: Executor, config: PiprConfig, bookmarks: CommandList, history: CommandList) -> App {
        App {
            window_state: WindowState::Main,
            input_state: EditorState::new(),
            command_output: "".into(),
            command_error: "".into(),
            last_executed_cmd: "".into(),
            autoeval_mode: config.autoeval_mode_default,
            paranoid_history_mode: config.paranoid_history_mode_default,
            should_quit: false,
            history_idx: None,
            snippet_mode: false,
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
                self.input_state.set_content(vec![String::new()]);
            }
        }
    }

    pub fn on_cmd_output(&mut self, process_result: ProcessResult) {
        match process_result {
            ProcessResult::Ok(stdout) => {
                if self.paranoid_history_mode {
                    self.history.push(self.input_state.content_to_commandentry());
                }
                self.command_output = stdout;
                self.command_error = String::new();
            }
            ProcessResult::NotOk(stderr) => {
                self.command_error = stderr;
            }
        }
    }

    pub fn set_should_quit(&mut self) {
        self.should_quit = true;
        self.history.push(self.input_state.content_to_commandentry());
    }

    pub fn execute_content(&mut self) {
        let command = self.input_state.content_lines();
        let command = command
            .iter()
            .filter(|line| !line.starts_with("#"))
            .map(|x| x.to_owned())
            .collect::<Vec<String>>()
            .join(" ");

        self.executor.execute(&command);
        self.last_executed_cmd = self.input_state.content_str();
    }

    pub fn main_window_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let control_pressed = modifiers.contains(KeyModifiers::CONTROL);
        if self.snippet_mode {
            if let KeyCode::Char(c) = code {
                // TODO check for pipes and use without_pipe, also normalize spacing
                if let Some(snippet) = self.config.snippets.get(&c) {
                    self.input_state.insert_at_cursor(&snippet.text);
                    self.input_state.cursor_col += snippet.cursor_offset;
                }
            }
            self.snippet_mode = false;
        } else {
            match code {
                KeyCode::Esc => self.set_should_quit(),
                KeyCode::Char('q') | KeyCode::Char('c') if control_pressed => self.set_should_quit(),
                KeyCode::F(2) => self.autoeval_mode = !self.autoeval_mode,
                KeyCode::F(3) => self.paranoid_history_mode = !self.paranoid_history_mode,

                KeyCode::Char('s') if control_pressed => self.bookmarks.toggle_entry(self.input_state.content_to_commandentry()),
                KeyCode::Char('p') if control_pressed => self.apply_history_prev(),
                KeyCode::Char('n') if control_pressed => self.apply_history_next(),
                KeyCode::Char('x') if control_pressed => {
                    self.history.push(self.input_state.content_to_commandentry());
                    self.input_state.apply_event(EditorEvent::Clear);
                }

                KeyCode::Char('v') if control_pressed => self.snippet_mode = true,
                KeyCode::Enter => {
                    if !self.input_state.content_str().is_empty() {
                        self.history.push(self.input_state.content_to_commandentry());
                    }
                    self.execute_content();
                }

                _ => {
                    if let Some(editor_event) = convert_keyevent_to_editorevent(code, modifiers) {
                        let previous_content = self.input_state.content_str();
                        self.input_state.apply_event(editor_event);

                        if previous_content != self.input_state.content_str() && self.autoeval_mode {
                            self.execute_content();
                        }
                    }
                }
            }
        }
    }

    pub fn on_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let control_pressed = modifiers.contains(KeyModifiers::CONTROL);
        match code {
            KeyCode::F(1) => match self.window_state {
                WindowState::TextView(_, _) => self.window_state = WindowState::Main,
                _ => self.window_state = WindowState::TextView("Help".to_string(), HELP_TEXT.to_string()),
            },
            KeyCode::Char('b') if control_pressed => match self.window_state {
                WindowState::BookmarkList(_) => self.window_state = WindowState::Main,
                _ => {
                    self.history.push(self.input_state.content_to_commandentry());

                    let entries = self.bookmarks.entries.clone();
                    self.window_state = WindowState::BookmarkList(CommandListState::new(entries, None));
                }
            },
            KeyCode::F(4) => match self.window_state {
                WindowState::HistoryList(_) => self.window_state = WindowState::Main,
                _ => {
                    self.history.push(self.input_state.content_to_commandentry());

                    let entries = self.history.entries.clone();
                    self.window_state = WindowState::HistoryList(CommandListState::new(entries, self.history_idx));
                }
            },
            _ => {
                let window_state = &mut self.window_state;
                match window_state {
                    WindowState::Main => self.main_window_tui_event(code, modifiers),
                    WindowState::TextView(_, _) => self.window_state = WindowState::Main,
                    WindowState::BookmarkList(state) => match code {
                        KeyCode::Esc => {
                            self.bookmarks.entries = state.list.clone();
                            self.window_state = WindowState::Main;
                        }
                        KeyCode::Enter => {
                            if let Some(entry) = state.selected_entry() {
                                self.input_state.load_commandentry(entry);
                            }
                            self.bookmarks.entries = state.list.clone();
                            self.window_state = WindowState::Main;
                        }
                        _ => state.apply_event(code),
                    },
                    WindowState::HistoryList(state) => match code {
                        KeyCode::Esc => {
                            self.history.entries = state.list.clone();
                            self.window_state = WindowState::Main;
                        }
                        KeyCode::Enter => {
                            if let Some(entry) = state.selected_idx.and_then(|idx| state.list.get(idx)) {
                                self.input_state.load_commandentry(entry);
                            }
                            self.history.entries = state.list.clone();
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
    pub fn new(list: Vec<CommandEntry>, selected_idx: Option<usize>) -> CommandListState {
        CommandListState {
            selected_idx: selected_idx.or(if list.len() == 0 { None } else { Some(list.len() - 1) }),
            list,
            recently_deleted: Vec::new(),
        }
    }
    pub fn selected_entry(&self) -> Option<&CommandEntry> {
        self.selected_idx.and_then(|idx| self.list.get(idx))
    }

    pub fn apply_event(&mut self, code: KeyCode) {
        if let Some(selected_idx) = self.selected_idx {
            match code {
                KeyCode::PageUp | KeyCode::Char('g') => {
                    self.selected_idx = if selected_idx >= 5 { Some(selected_idx - 5) } else { Some(0) };
                }
                KeyCode::PageDown | KeyCode::Char('G') if self.list.len() > 0 => {
                    self.selected_idx = if (selected_idx as isize) < (self.list.len() as isize - 5) {
                        Some(selected_idx + 5)
                    } else {
                        Some(self.list.len() - 1)
                    };
                }

                KeyCode::Up | KeyCode::Char('k') if selected_idx > 0 => self.selected_idx = Some(selected_idx - 1),
                KeyCode::Down | KeyCode::Char('j') if selected_idx < self.list.len() - 1 => {
                    self.selected_idx = Some(selected_idx + 1)
                }
                KeyCode::Char('u') => {
                    self.recently_deleted.pop().map(|entry| self.list.push(entry));
                    self.selected_idx = Some(self.list.len() - 1);
                }
                KeyCode::Delete | KeyCode::Backspace => {
                    let deleted_entry = self.list.remove(selected_idx);
                    self.recently_deleted.push(deleted_entry);
                    if self.list.is_empty() {
                        self.selected_idx = None;
                    } else if self.list.get(selected_idx).is_none() {
                        self.selected_idx = Some(selected_idx - 1);
                    }
                }

                _ => {}
            }
        }
    }
}
