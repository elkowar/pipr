use super::lineeditor::*;
use super::{command_list_window::CommandListState, pipr_config::*};
use crate::command_evaluation::*;
use crate::commandlist::CommandList;
use crossterm::event::{KeyCode, KeyModifiers};

pub const HELP_TEXT: &str = "\
F1         Show/hide help
F2         Toggle autoeval
F3         Toggle Paranoid history (fills up history in autoeval)
F4         Show/hide history
Ctrl+B     Show/hide bookmarks
F5         Open man page of command under cursor
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
    pub opened_manpage: Option<String>,
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
            opened_manpage: None,
            should_quit: false,
            history_idx: None,
            snippet_mode: false,
            executor,
            config,
            bookmarks,
            history,
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

    pub fn on_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let control_pressed = modifiers.contains(KeyModifiers::CONTROL);
        match code {
            KeyCode::F(1) => match self.window_state {
                WindowState::TextView(_, _) => self.window_state = WindowState::Main,
                _ => self.window_state = WindowState::TextView("Help".to_string(), HELP_TEXT.to_string()),
            },
            KeyCode::Char('b') if control_pressed => match self.window_state {
                WindowState::BookmarkList(_) => {
                    self.window_state = WindowState::Main;
                }
                _ => {
                    self.history.push(self.input_state.content_to_commandentry());

                    let entries = self.bookmarks.entries.clone();
                    self.window_state = WindowState::BookmarkList(CommandListState::new(entries, None));
                }
            },
            KeyCode::F(4) => match self.window_state {
                WindowState::HistoryList(_) => {
                    self.window_state = WindowState::Main;
                }
                _ => {
                    self.history.push(self.input_state.content_to_commandentry());

                    let entries = self.history.entries.clone();
                    self.window_state = WindowState::HistoryList(CommandListState::new(entries, self.history_idx));
                }
            },
            _ => self.handle_window_specific_event(code, modifiers),
        }
    }

    pub fn handle_window_specific_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let window_state = &mut self.window_state;
        match window_state {
            WindowState::Main => self.handle_main_window_tui_event(code, modifiers),
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

/// returns the word at the given byte index.
pub fn word_under_cursor(line: &str, cursor_col: usize) -> Option<&str> {
    let words = line.split_whitespace().collect::<Vec<&str>>();
    let mut hovered_word = None;
    if words.len() == 1 {
        hovered_word = Some(line);
    }
    for idx in 0..words.len() {
        let len = words.clone().into_iter().take(idx + 1).collect::<Vec<&str>>().join(" ").len();
        if len > cursor_col {
            hovered_word = words.get(idx).cloned();
            break;
        }
    }
    hovered_word
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_word_under_cursor() {
        assert_eq!(word_under_cursor("abc def ghi", 5), Some("def"));
        assert_eq!(word_under_cursor("abc def ghi", 2), Some("abc"));
        assert_eq!(word_under_cursor("abc def ghi", 0), Some("abc"));
        assert_eq!(word_under_cursor("abc def ghi", 10), Some("ghi"));
        assert_eq!(word_under_cursor("abc def ghi", 11), None);
        assert_eq!(word_under_cursor("", 0), None);
        assert_eq!(word_under_cursor("", 2), None);
        assert_eq!(word_under_cursor("abc", 0), Some("abc"));
        assert_eq!(word_under_cursor("abc", 3), Some("abc"));
    }
}
