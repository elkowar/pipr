use crate::commandlist::CommandEntry;
use crossterm::event::KeyCode;

pub struct CommandListState {
    pub list: Vec<CommandEntry>,
    pub selected_idx: Option<usize>,
    recently_deleted: Vec<CommandEntry>,
}

impl CommandListState {
    pub fn new(list: Vec<CommandEntry>, selected_idx: Option<usize>) -> CommandListState {
        CommandListState {
            selected_idx: selected_idx.or(if list.is_empty() { None } else { Some(list.len() - 1) }),
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
                KeyCode::PageDown | KeyCode::Char('G') if !self.list.is_empty() => {
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
                    if let Some(entry) = self.recently_deleted.pop() {
                        self.list.push(entry);
                    }
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
