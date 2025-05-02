extern crate crossterm;

use crate::app::command_list_window::CommandListState;
use crate::app::key_select_menu::KeySelectMenu;
use crate::app::main_window::AutocompleteState;
use crate::commandlist::CommandList;
use crate::lineeditor::EditorState;
use crate::util::VecStringExt;
use crate::{CmdOutput, CommandExecutionHandler, CommandExecutionRequest, PiprConfig};

use crossterm::event::{KeyCode, KeyModifiers};

pub mod command_list_window;
pub mod key_select_menu;
pub mod main_window;

pub const HELP_TEXT: &str = "\
F1         Show/hide help
F2         Toggle autoeval
F3         Toggle Paranoid history (fills up history in autoeval)
F4         Show/hide history
Ctrl+B     Show/hide bookmarks
F5         Open helpviewer
F6         Open outputviewer
F7         When the cursor is on a `|` symbol, cache the output of everything before that |
Ctrl+S     Save bookmark
Alt+Return Newline
Ctrl+U     Clear Command
Ctrl+P     Previous in history
Ctrl+N     Next in history
Ctrl+V     Insert snippet (press corresponding key to choose)

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

pub enum KeySelectMenuType {
    Snippets,
    OpenWordIn(String), // stores the word that should be opened in the selected help
    OpenOutputIn(String),
}

#[derive(Debug)]
pub struct CachedCommandPart {
    /// the line where the cached command part ends (must be within the bounds of the input_state)
    pub end_line: usize,
    /// the column where the cached command part ends
    pub end_col: usize,
    /// the cached output of the command
    pub cached_output: Vec<String>,
}

impl CachedCommandPart {
    pub fn new(end_line: usize, end_col: usize, cached_output: Vec<String>) -> CachedCommandPart {
        CachedCommandPart {
            end_line,
            end_col,
            cached_output,
        }
    }
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
    pub execution_handler: CommandExecutionHandler,
    pub config: PiprConfig,
    pub should_quit: bool,
    pub opened_key_select_menu: Option<KeySelectMenu<KeySelectMenuType>>,
    pub raw_mode: bool,
    pub autocomplete_state: Option<AutocompleteState>,

    /// Part of a command can be cached, so it will not be reevluated on every execution.
    pub cached_command_part: Option<CachedCommandPart>,

    /// number from 0-4 showing an animation that shows some process being executed
    pub is_processing_state: Option<u8>,

    /// A (stdin, command) that should be executed in the main screen.
    /// this will be taken ( and thus reset ) and handled by the ui module.
    pub should_jump_to_other_cmd: Option<(Option<String>, std::process::Command)>,
}

impl App {
    pub fn new(
        execution_handler: CommandExecutionHandler,
        raw_mode: bool,
        config: PiprConfig,
        bookmarks: CommandList,
        history: CommandList,
    ) -> App {
        App {
            autocomplete_state: None,
            window_state: WindowState::Main,
            input_state: EditorState::new(),
            command_output: "".into(),
            command_error: "".into(),
            last_executed_cmd: "".into(),
            autoeval_mode: config.autoeval_mode_default,
            paranoid_history_mode: config.paranoid_history_mode_default,
            should_quit: false,
            is_processing_state: None,
            history_idx: None,
            cached_command_part: None,
            opened_key_select_menu: None,
            should_jump_to_other_cmd: None,
            execution_handler,
            raw_mode,
            config,
            bookmarks,
            history,
        }
    }

    pub fn on_cmd_output(&mut self, process_result: CmdOutput) {
        self.is_processing_state = None;
        match process_result {
            CmdOutput::Ok(stdout) => {
                if self.paranoid_history_mode {
                    self.history.push(self.input_state.content_to_commandentry());
                }
                self.command_output = stdout;
                self.command_error = String::new();
            }
            CmdOutput::NotOk(stderr) => self.command_error = stderr,
        }
    }

    pub fn set_should_quit(&mut self) {
        self.should_quit = true;
        self.history.push(self.input_state.content_to_commandentry());
    }

    pub fn execute_content(&mut self) {
        let lines = self.input_state.content_lines().clone();
        let lines = match self.cached_command_part {
            Some(CachedCommandPart { end_line, end_col, .. }) => lines.split_strings_at_offset(end_line, end_col).1,
            _ => lines,
        };

        let command = lines
            .iter()
            .filter(|line| !line.starts_with('#'))
            .cloned()
            .collect::<Vec<String>>();
        let command = if self.raw_mode {
            command.join("\n")
        } else {
            command.join(" ")
        };

        let execution_request =
            CommandExecutionRequest::new(command, self.cached_command_part.as_ref().map(|x| x.cached_output.to_owned()));
        self.execution_handler.execute(execution_request);
        self.is_processing_state = Some(0);
        self.last_executed_cmd = self.input_state.content_str();
    }

    fn toggle_history_list(&mut self) {
        match self.window_state {
            WindowState::HistoryList(_) => self.window_state = WindowState::Main,
            _ => {
                self.history.push(self.input_state.content_to_commandentry());
                let entries = self.history.entries().clone();
                self.window_state = WindowState::HistoryList(CommandListState::new(entries, self.history_idx));
            }
        }
    }

    fn toggle_bookmark_list(&mut self) {
        match self.window_state {
            WindowState::BookmarkList(_) => self.window_state = WindowState::Main,
            _ => {
                self.history.push(self.input_state.content_to_commandentry());
                let entries = self.bookmarks.entries().clone();
                self.window_state = WindowState::BookmarkList(CommandListState::new(entries, None));
            }
        }
    }

    fn toggle_help_window(&mut self) {
        match self.window_state {
            WindowState::TextView(_, _) => self.window_state = WindowState::Main,
            _ => self.window_state = WindowState::TextView("Help".to_string(), HELP_TEXT.to_string()),
        }
    }

    pub fn on_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let control_pressed = modifiers.contains(KeyModifiers::CONTROL);
        match code {
            KeyCode::F(1) => self.toggle_help_window(),
            KeyCode::Char('b') if control_pressed => self.toggle_bookmark_list(),
            KeyCode::F(4) => self.toggle_history_list(),
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
                    self.bookmarks.set_entries(state.list.clone());
                    self.window_state = WindowState::Main;
                }
                KeyCode::Enter => {
                    if let Some(entry) = state.selected_entry() {
                        self.input_state.load_commandentry(entry);
                        self.cached_command_part = None;
                    }
                    self.bookmarks.set_entries(state.list.clone());
                    self.window_state = WindowState::Main;
                }
                _ => state.apply_event(code),
            },
            WindowState::HistoryList(state) => match code {
                KeyCode::Esc => {
                    self.history.set_entries(state.list.clone());
                    self.window_state = WindowState::Main;
                }
                KeyCode::Enter => {
                    if let Some(entry) = state.selected_idx.and_then(|idx| state.list.get(idx)) {
                        self.input_state.load_commandentry(entry);
                        self.cached_command_part = None;
                    }
                    self.history.set_entries(state.list.clone());
                    self.history_idx = state.selected_idx;
                    self.window_state = WindowState::Main;
                }
                _ => state.apply_event(code),
            },
        }
    }

    pub fn on_tick(&mut self) {
        self.is_processing_state = self.is_processing_state.map(|x| (x + 1) % 6)
    }
}
