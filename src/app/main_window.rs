use crate::app::key_select_menu::KeySelectMenu;
use crate::app::{App, CachedCommandPart, KeySelectMenuType};
use crate::command_evaluation;
use crate::lineeditor::{convert_keyevent_to_editorevent, EditorEvent};
use crate::util::{StringExt, VecStringExt};
use crate::CmdOutput;
use crate::Stdio;

use crossterm::event::{KeyCode, KeyModifiers};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// a state holder for the autocomplete menu
#[derive(Debug)]
pub struct AutocompleteState {
    pub original_prompt: String,
    pub options: Vec<String>,
    pub current_idx: usize,
}

impl AutocompleteState {
    fn from_options(original_prompt: String, options: Vec<String>) -> Option<AutocompleteState> {
        if options.is_empty() {
            None
        } else {
            Some(AutocompleteState {
                current_idx: 0,
                original_prompt,
                options,
            })
        }
    }
    fn cycle_selected(&mut self) {
        self.current_idx = (self.current_idx + 1) % self.options.len();
    }
    fn cycle_selected_backwards(&mut self) {
        if self.current_idx == 0 {
            self.current_idx = self.options.len() - 1;
        } else {
            self.current_idx -= 1;
        }
    }
    fn selected(&self) -> &str {
        &self.options[self.current_idx]
    }
}

impl App {
    pub fn handle_key_select_menu_event(&mut self, key_select_menu: KeySelectMenu<KeySelectMenuType>, c: char) {
        match key_select_menu.menu_type {
            KeySelectMenuType::Snippets => {
                if let Some(snippet) = self.config.snippets.get(&c) {
                    self.input_state.insert_at_cursor(&snippet.text, false);
                    self.input_state.cursor_col += snippet.cursor_offset;
                }
            }
            KeySelectMenuType::OpenWordIn(word) => {
                if let Some(help_viewer) = self.config.help_viewers.get(&c) {
                    self.should_jump_to_other_cmd = Some((None, help_viewer.resolve_to_command(&word)));
                }
            }
            KeySelectMenuType::OpenOutputIn(output) => {
                if let Some(output_viewer) = self.config.output_viewers.get(&c) {
                    let mut command = Command::new(output_viewer);
                    command.stdin(Stdio::piped());
                    self.should_jump_to_other_cmd = Some((Some(output), command));
                }
            }
        }
    }

    fn do_cache_command_part(&mut self) {
        if self.input_state.hovered_char() != Some("|") {
            return;
        }
        let mut command_to_cache = self
            .input_state
            .content_lines()
            .split_strings_at_offset(self.input_state.cursor_line, self.input_state.cursor_col + 1)
            .0;

        command_to_cache.last_mut().map(|x| x.pop());
        if command_to_cache.last() == Some(&"".to_owned()) {
            command_to_cache.pop();
        }

        let command_result = command_evaluation::execute_command_blocking(
            &self.execution_handler.shell_command, 
            &command_to_cache.join(" "),
            self.execution_handler.execution_mode
        );

        if self.input_state.cursor_col < self.input_state.current_line().len() {
            match command_result {
                Ok(output) => {
                    self.cached_command_part = Some(CachedCommandPart::new(
                        self.input_state.cursor_line,
                        self.input_state.cursor_col + 1,
                        output,
                    ));
                }
                Err(err) => {
                    self.input_state.cursor_col = self.input_state.prev_char_index();
                    self.on_cmd_output(CmdOutput::NotOk(format!("could not run command to cache: {}", err)));
                }
            }
        }
    }

    fn open_autocomplete_menu(&mut self) {
        let current_line = self.input_state.current_line().to_string();
        let hovered_word = current_line.word_at_idx(self.input_state.cursor_col);
        let hovered_char = self.input_state.hovered_char();
        if hovered_char.is_none() || hovered_char == Some(" ") || hovered_char == Some("") {
            let hovered_word = hovered_word.unwrap_or_default();
            if let Some(completions) = provide_path_autocomplete(hovered_word) {
                if completions.len() == 1 {
                    let completed_value = completions.first().unwrap().trim_start_matches(hovered_word);
                    self.input_state.insert_at_cursor(completed_value, true);
                } else if completions.len() > 1 {
                    self.autocomplete_state = AutocompleteState::from_options(hovered_word.to_string(), completions);
                }
            }
        }
    }

    fn open_helpviewer(&mut self) {
        let current_line = self.input_state.current_line();
        let hovered_word = current_line.word_at_idx(self.input_state.cursor_col);
        if let Some(word) = hovered_word {
            let help_viewers = &self.config.help_viewers;
            let options = help_viewers.iter().map(|(&k, v)| (k, v.resolve(word))).collect();
            let key_select_menu = KeySelectMenu::new(options, KeySelectMenuType::OpenWordIn(word.into()));
            self.opened_key_select_menu = Some(key_select_menu);
        }
    }

    fn open_outputviewer(&mut self) {
        let current_output = self.command_output.to_owned();
        let output_viewers = &self.config.output_viewers;
        let options = output_viewers.iter().map(|(&k, v)| (k, v.to_owned())).collect();
        let key_select_menu = KeySelectMenu::new(options, KeySelectMenuType::OpenOutputIn(current_output));
        self.opened_key_select_menu = Some(key_select_menu);
    }

    pub fn handle_main_window_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let control_pressed = modifiers.contains(KeyModifiers::CONTROL);

        if let Some(autocomplete_state) = self.autocomplete_state.as_mut() {
            match code {
                KeyCode::Tab | KeyCode::Down => autocomplete_state.cycle_selected(),
                KeyCode::BackTab | KeyCode::Up => autocomplete_state.cycle_selected_backwards(),
                KeyCode::Enter => {
                    let completed_value = autocomplete_state
                        .selected()
                        .trim_start_matches(&autocomplete_state.original_prompt);
                    self.input_state.insert_at_cursor(completed_value, true);
                    self.autocomplete_state = None;
                }
                _ => self.autocomplete_state = None,
            }
            return;
        }

        if let Some(key_select_menu) = self.opened_key_select_menu.take() {
            if let KeyCode::Char(c) = code {
                self.handle_key_select_menu_event(key_select_menu, c);
            }
            return;
        }

        match code {
            KeyCode::Esc => self.set_should_quit(),
            KeyCode::Char('q') | KeyCode::Char('c') if control_pressed => self.set_should_quit(),
            KeyCode::F(2) => self.autoeval_mode = !self.autoeval_mode,
            KeyCode::F(3) => self.paranoid_history_mode = !self.paranoid_history_mode,
            KeyCode::Tab => self.open_autocomplete_menu(),
            KeyCode::F(5) => self.open_helpviewer(),
            KeyCode::F(6) => self.open_outputviewer(),
            KeyCode::F(7) => self.do_cache_command_part(),

            KeyCode::Char('s') if control_pressed => self.bookmarks.toggle_entry(self.input_state.content_to_commandentry()),
            KeyCode::Char('p') if control_pressed => self.apply_history_prev(),
            KeyCode::Char('n') if control_pressed => self.apply_history_next(),
            KeyCode::Char('x') if control_pressed => {
                self.history.push(self.input_state.content_to_commandentry());
                self.history_idx = None;
                self.input_state.apply_event(EditorEvent::Clear);
            }

            KeyCode::Char('v') if control_pressed => {
                self.opened_key_select_menu = Some(KeySelectMenu::new(
                    self.config.snippets.iter().map(|(&c, v)| (c, v.to_string())).collect(),
                    KeySelectMenuType::Snippets,
                ));
            }
            KeyCode::Enter if !modifiers.contains(KeyModifiers::ALT) => {
                self.history.push(self.input_state.content_to_commandentry());
                self.execute_content();
            }

            _ => {
                if let Some(editor_event) = convert_keyevent_to_editorevent(code, modifiers) {
                    let previous_content = self.input_state.content_lines().clone();
                    self.history_idx = None;
                    self.input_state.apply_event(editor_event);

                    let new_content = self.input_state.content_lines();

                    if let Some(CachedCommandPart { end_line, end_col, .. }) = self.cached_command_part {
                        if previous_content.split_strings_at_offset(end_line, end_col).0
                            != new_content.split_strings_at_offset(end_line, end_col).0
                        {
                            self.cached_command_part = None;
                        }
                    }

                    if self.autoeval_mode && previous_content != *new_content {
                        self.execute_content();
                    }
                }
            }
        }
    }

    fn apply_history_prev(&mut self) {
        self.cached_command_part = None;
        if let Some(idx) = self.history_idx {
            if idx > 0 {
                self.history_idx = Some(idx - 1);
                self.input_state.load_commandentry(self.history.get_at(idx - 1).unwrap());
            }
        } else if self.history.len() > 0 {
            let new_idx = self.history.len() - 1;
            self.history_idx = Some(new_idx);
            self.history.push(self.input_state.content_to_commandentry());
            self.input_state.load_commandentry(self.history.get_at(new_idx).unwrap());
        }
    }

    fn apply_history_next(&mut self) {
        self.cached_command_part = None;
        if let Some(idx) = self.history_idx {
            let new_idx = idx + 1;
            if new_idx < self.history.len() - 1 {
                self.history_idx = Some(new_idx);
                self.input_state.load_commandentry(self.history.get_at(new_idx).unwrap());
            } else {
                self.history_idx = None;
                self.input_state.set_content(vec![String::new()]);
            }
        }
    }
}

fn provide_path_autocomplete(word: &str) -> Option<Vec<String>> {
    let home_dir = std::env::var("HOME")
        .map(|p| Path::new(&p).to_path_buf())
        .expect("$HOME not set");

    let path = if word.trim().is_empty() {
        PathBuf::from("./")
    } else if word.starts_with('~') {
        let mut dir = home_dir.clone();
        dir.push(word.trim_start_matches('~').trim_start_matches('/').trim_start_matches('\\'));
        dir
    } else {
        PathBuf::from(word)
    };

    let possible_children: Vec<_> = if let Ok(entries) = path.read_dir() {
        entries.filter_map(|entry| entry.ok()).collect()
    } else {
        let started_subfile_name = path.file_name().unwrap().to_string_lossy().to_string();
        let parent_path = path.parent().unwrap_or_else(|| Path::new("./"));
        if let Ok(parent_entries) = parent_path.read_dir() {
            parent_entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(&started_subfile_name))
                .collect()
        } else {
            Vec::default()
        }
    };

    let completions = possible_children
        .iter()
        .map(|entry| entry.path().display().to_string().replace(home_dir.to_str().unwrap(), "~"))
        .collect::<Vec<_>>();
    if completions.is_empty() {
        None
    } else {
        Some(completions)
    }
}
