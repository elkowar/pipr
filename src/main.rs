#![feature(vec_remove_item)]
#[macro_use]
extern crate num_derive;
extern crate num_traits;

use num_traits::FromPrimitive;
use std::io;
use std::io::Write;
use std::process::Command;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, SelectableList, Text, Widget};
use tui::{backend::CrosstermBackend, Terminal};

use crossterm::{
    cursor,
    event::{read, Event as CEvent, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub mod lineeditor;
use lineeditor as le;

#[derive(PartialEq, PartialOrd, Eq, Ord, FromPrimitive, Clone, Copy, Debug)]
enum UIArea {
    CommandInput,
    Config,
    Output,
    BookmarkList,
}

impl UIArea {
    fn next_area(&self) -> UIArea {
        match FromPrimitive::from_u8(*self as u8 + 1) {
            Some(next) => next,
            None => FromPrimitive::from_u8(0).unwrap(),
        }
    }
}

struct App {
    selected_area: UIArea,
    input_state: le::EditorState,
    command_output: String,
    command_error: Option<String>,
    autoeval_mode: bool,
    bookmarks: Vec<String>,
    last_unsaved: Option<String>,
    selected_bookmark_idx: Option<usize>,
}

impl App {
    fn new() -> App {
        App {
            selected_area: UIArea::CommandInput,
            input_state: le::EditorState::new(),
            command_output: "".into(),
            command_error: None,
            autoeval_mode: false,
            bookmarks: Vec::new(),
            last_unsaved: None,
            selected_bookmark_idx: None,
        }
    }

    fn eval_input(&mut self) {
        let (stdout, stderr) = evaluate_command(&self.input_state.content_str());
        if stderr == None {
            self.command_output = stdout;
        }
        self.command_error = stderr;
    }

    fn toggle_bookmarked(&mut self) {
        let content = self.input_state.content_str();
        if self.bookmarks.contains(&content) {
            self.bookmarks.remove_item(&content);
        } else {
            self.bookmarks.push(content);
        }
    }

    fn apply_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if code == KeyCode::Tab {
            self.selected_area = self.selected_area.next_area();
            return;
        }
        match self.selected_area {
            UIArea::CommandInput => {
                let previous_content = self.input_state.content_str().clone();
                match code {
                    KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => self.toggle_bookmarked(),

                    KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input_state.apply_event(le::EditorEvent::KillWordBack)
                    }
                    KeyCode::Char(c) => self.input_state.apply_event(le::EditorEvent::NewCharacter(c)),
                    KeyCode::Backspace => self.input_state.apply_event(le::EditorEvent::Backspace),
                    KeyCode::Delete => self.input_state.apply_event(le::EditorEvent::Delete),

                    KeyCode::Left => self.input_state.apply_event(le::EditorEvent::GoLeft),
                    KeyCode::Right => self.input_state.apply_event(le::EditorEvent::GoRight),
                    KeyCode::Home => self.input_state.apply_event(le::EditorEvent::Home),
                    KeyCode::End => self.input_state.apply_event(le::EditorEvent::End),
                    KeyCode::Enter => self.eval_input(),
                    _ => {}
                }

                if previous_content != self.input_state.content_str() && self.autoeval_mode {
                    self.eval_input();
                }
            }
            UIArea::Config => match code {
                KeyCode::Enter => self.autoeval_mode = !self.autoeval_mode,
                _ => {}
            },
            UIArea::BookmarkList => match code {
                KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.selected_bookmark_idx.map(|idx| self.bookmarks.remove(idx));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(idx) = self.selected_bookmark_idx {
                        self.selected_bookmark_idx = Some((idx + 1) % self.bookmarks.len() as usize);
                    } else {
                        self.last_unsaved = Some(self.input_state.content_str());
                        self.selected_bookmark_idx = Some(0);
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(idx) = self.selected_bookmark_idx {
                        self.selected_bookmark_idx = Some((idx - 1) % self.bookmarks.len() as usize);
                    } else {
                        self.last_unsaved = Some(self.input_state.content_str());
                        self.selected_bookmark_idx = Some(0);
                    }
                }
                KeyCode::Enter => {
                    if let Some(bookmark) = self.selected_bookmark_idx.and_then(|idx| self.bookmarks.get(idx)).cloned() {
                        self.input_state.set_content(&bookmark);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn main() -> Result<(), failure::Error> {
    let mut app = App::new();

    let mut stdout = io::stdout();
    #[allow(deprecated)]
    execute!(stdout, EnterAlternateScreen)?;
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut running = true;
    while running {
        let mut input_field_rect = tui::layout::Rect::new(0, 0, 0, 0);

        terminal.draw(|mut f| {
            use Constraint::*;
            let root_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Percentage(20), Percentage(80)].as_ref())
                .margin(1)
                .split(f.size());

            let bookmark_items: &Vec<String> = &app.bookmarks;
            //let bookmark_items: Vec<String> = if let Some(last_unsaved) = &app.last_unsaved {
            //let mut items = bookmark_items.clone();
            //items.insert(0, last_unsaved.clone());
            //items
            //} else {
            //bookmark_items.clone()
            //};

            SelectableList::default()
                .block(make_default_block("Bookmarks", app.selected_area == UIArea::BookmarkList))
                .items(bookmark_items.as_slice())
                .select(app.selected_bookmark_idx)
                .highlight_style(Style::default().modifier(Modifier::ITALIC))
                .highlight_symbol(">>")
                .render(&mut f, root_chunks[0]);

            let exec_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Length(3), Length(3), Percentage(50), Percentage(50)].as_ref())
                .split(root_chunks[1]);

            input_field_rect = exec_chunks[0];

            let input_text = [Text::raw(format!("{}", &app.input_state.content_str()))];

            Paragraph::new(input_text.iter())
                .block(make_default_block("Command", app.selected_area == UIArea::CommandInput))
                .render(&mut f, exec_chunks[0]);

            let output_text = [Text::raw(format!("{}", &app.command_output))];
            Paragraph::new(output_text.iter())
                .block(make_default_block("Output", app.selected_area == UIArea::Output))
                .render(&mut f, exec_chunks[2]);

            if let Some(error) = &app.command_error {
                let error_text = [Text::raw(format!("{}", error))];
                Paragraph::new(error_text.iter())
                    .block(make_default_block("Stderr", app.selected_area == UIArea::Output))
                    .render(&mut f, exec_chunks[3]);
            }

            let config_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Percentage(50), Percentage(50)].as_ref())
                .split(exec_chunks[1]);

            let immediate_eval_state = if app.autoeval_mode { "Active" } else { "Inactive" };
            Paragraph::new([Text::raw(immediate_eval_state)].iter())
                .block(make_default_block("Immediate eval", app.selected_area == UIArea::Config))
                .render(&mut f, config_chunks[0]);
        })?;

        // move cursor to where it belongs.
        terminal.backend_mut().write(
            format!(
                "{}",
                cursor::MoveTo(
                    input_field_rect.x + 1 + app.input_state.displayed_cursor_column() as u16,
                    input_field_rect.y + 1
                )
            )
            .as_bytes(),
        )?;
        // immediately _show_ the moved cursor where it now should be
        io::stdout().flush().ok();

        loop {
            match read()? {
                CEvent::Key(KeyEvent { code, modifiers }) => {
                    match code {
                        KeyCode::Esc => running = false,
                        _ => app.apply_event(code, modifiers),
                    }
                    break;
                }
                CEvent::Resize(_, _) => {
                    break;
                }
                _ => {}
            }
        }
    }
    disable_raw_mode()?;
    #[allow(deprecated)]
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn make_default_block(title: &str, selected: bool) -> Block {
    let title_style = if selected {
        Style::default().fg(Color::Black).bg(Color::White)
    } else {
        Style::default().fg(Color::White).bg(Color::Black)
    };

    Block::default().title(title).borders(Borders::ALL).title_style(title_style)
}

fn evaluate_command(cmd: &str) -> (String, Option<String>) {
    if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("-i") {
        return (
            "".into(),
            Some("Will not evaluate this command. it's for your own safety, believe me....".into()),
        );
    }

    let output = Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .output()
        .expect("failed to execute process");
    let stdout = std::str::from_utf8(&output.stdout).unwrap().to_owned();
    let stderr = std::str::from_utf8(&output.stderr).unwrap().to_owned();
    (stdout, if stderr.is_empty() { None } else { Some(stderr) })
}
