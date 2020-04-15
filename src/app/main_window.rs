use super::app::*;
use super::lineeditor::*;
use crossterm::event::{KeyCode, KeyModifiers};

impl App {
    pub fn handle_snippet_mode_event(&mut self, code: KeyCode) {
        if let KeyCode::Char(c) = code {
            // TODO check for pipes and use without_pipe, also normalize spacing
            if let Some(snippet) = self.config.snippets.get(&c) {
                self.input_state.insert_at_cursor(&snippet.text);
                self.input_state.cursor_col += snippet.cursor_offset;
            }
        }
        self.snippet_mode = false;
    }
    pub fn handle_main_window_tui_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let control_pressed = modifiers.contains(KeyModifiers::CONTROL);
        if self.snippet_mode {
            self.handle_snippet_mode_event(code);
            return;
        }

        match code {
            KeyCode::Esc => self.set_should_quit(),
            KeyCode::Char('q') | KeyCode::Char('c') if control_pressed => self.set_should_quit(),
            KeyCode::F(2) => self.autoeval_mode = !self.autoeval_mode,
            KeyCode::F(3) => self.paranoid_history_mode = !self.paranoid_history_mode,

            KeyCode::F(5) => {
                let hovered_word = word_under_cursor(self.input_state.current_line(), self.input_state.cursor_col);
                self.opened_manpage = hovered_word.map(|x| x.to_string());
            }

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
