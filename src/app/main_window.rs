use super::app::*;
use super::key_select_menu::KeySelectMenu;
use super::{lineeditor::*, Path};
use crossterm::event::{KeyCode, KeyModifiers};
use std::path::PathBuf;

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
    fn selected(&self) -> &str {
        &self.options[self.current_idx]
    }
}

impl App {
    pub fn handle_key_select_menu_event(&mut self, key_select_menu: KeySelectMenu<KeySelectMenuType>, c: char) {
        match key_select_menu.menu_type {
            KeySelectMenuType::Snippets => {
                if let Some(snippet) = self.config.snippets.get(&c) {
                    self.input_state.insert_at_cursor(&snippet.text);
                    self.input_state.cursor_col += snippet.cursor_offset;
                }
            }
            KeySelectMenuType::OpenWordIn(word) => {
                if let Some(help_viewer) = self.config.help_viewers.get(&c) {
                    self.should_jump_to_other_cmd = Some(help_viewer.resolve_to_command(&word));
                }
            }
        }
    }

    pub fn handle_main_window_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let control_pressed = modifiers.contains(KeyModifiers::CONTROL);

        if let Some(autocomplete_state) = self.autocomplete_state.as_mut() {
            match code {
                KeyCode::Tab => {
                    autocomplete_state.cycle_selected();
                    return;
                }
                KeyCode::Enter => {
                    let chosen_completion = autocomplete_state.selected();
                    let completed_value = chosen_completion.trim_start_matches(&autocomplete_state.original_prompt);
                    self.input_state.insert_at_cursor(completed_value);
                    self.input_state.cursor_col += completed_value.len();
                    self.autocomplete_state = None;
                    return;
                }
                _ => {
                    self.autocomplete_state = None;
                }
            }
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

            KeyCode::Tab => {
                let current_line = self.input_state.current_line().to_string();
                let hovered_word = word_under_cursor(&current_line, self.input_state.cursor_col);

                if let Some(hovered_word) = hovered_word {
                    if let Some(completions) = provide_path_autocomplete(hovered_word) {
                        if completions.len() == 1 {
                            let completed_value = completions.first().unwrap();
                            let completed_value = completed_value.trim_start_matches(hovered_word);
                            self.input_state.insert_at_cursor(completed_value);
                            self.input_state.cursor_col += completed_value.len();
                        } else if completions.len() > 1 {
                            self.autocomplete_state = AutocompleteState::from_options(hovered_word.to_string(), completions);
                        }
                    }
                }
            }

            KeyCode::F(5) => {
                let hovered_word = word_under_cursor(self.input_state.current_line(), self.input_state.cursor_col);
                if let Some(word) = hovered_word {
                    let help_viewers = &self.config.help_viewers;
                    let options = help_viewers.iter().map(|(&k, v)| (k, v.resolve(word))).collect();
                    let key_select_menu = KeySelectMenu::new(options, KeySelectMenuType::OpenWordIn(word.into()));
                    self.opened_key_select_menu = Some(key_select_menu);
                }
            }

            KeyCode::Char('s') if control_pressed => self.bookmarks.toggle_entry(self.input_state.content_to_commandentry()),
            KeyCode::Char('p') if control_pressed => self.apply_history_prev(),
            KeyCode::Char('n') if control_pressed => self.apply_history_next(),
            KeyCode::Char('x') if control_pressed => {
                self.history.push(self.input_state.content_to_commandentry());
                self.input_state.apply_event(EditorEvent::Clear);
            }

            KeyCode::Char('v') if control_pressed => {
                self.opened_key_select_menu = Some(KeySelectMenu::new(
                    self.config.snippets.iter().map(|(&c, v)| (c, v.to_string())).collect(),
                    KeySelectMenuType::Snippets,
                ));
            }
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

                    if self.autoeval_mode && previous_content != self.input_state.content_str() {
                        self.execute_content();
                    }
                }
            }
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
}

fn provide_path_autocomplete(word: &str) -> Option<Vec<String>> {
    let mut path = PathBuf::new();
    path.push(word);

    let possible_children: Vec<_> = if let Ok(entries) = path.read_dir() {
        entries.filter_map(|entry| entry.ok()).collect()
    } else {
        let started_subfile_name = path.file_name().unwrap().to_string_lossy().to_string();
        let parent_path = path.parent().unwrap_or(Path::new("./"));
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
        .map(|entry| entry.path().display().to_string())
        .collect::<Vec<_>>();
    if completions.is_empty() {
        None
    } else {
        Some(completions)
    }
}
