#![feature(vec_remove_item)]
#[macro_use]
extern crate num_derive;
extern crate num_traits;

use num_traits::FromPrimitive;
use std::env;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::PathBuf;
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

pub mod bookmark;
pub mod lineeditor;
pub use bookmark::BookmarkList;
use lineeditor as le;

#[derive(PartialEq, PartialOrd, Eq, Ord, FromPrimitive, Clone, Copy, Debug)]
enum UIArea {
    CommandInput,
    Config,
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
    bookmarks: BookmarkList,
    last_unsaved: Option<String>,
    selected_bookmark_idx: Option<usize>,
    unsafe_mode: bool,
}

impl App {
    fn new(unsafe_mode: bool) -> App {
        App {
            selected_area: UIArea::CommandInput,
            input_state: le::EditorState::new(),
            command_output: "".into(),
            command_error: None,
            autoeval_mode: false,
            bookmarks: BookmarkList::new(),
            last_unsaved: None,
            selected_bookmark_idx: None,
            unsafe_mode,
        }
    }

    fn eval_input(&mut self) {
        let (stdout, stderr) = evaluate_command(self.unsafe_mode, &self.input_state.content_str());
        if stderr == None {
            self.command_output = stdout;
        }
        self.command_error = stderr;
    }

    fn toggle_bookmarked(&mut self) { self.bookmarks.toggle_bookmark(self.input_state.content_to_bookmark()); }

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
                    KeyCode::Char('z') if modifiers.contains(KeyModifiers::CONTROL) => {
                        self.last_unsaved.clone().map(|x| self.input_state.set_content(&x));
                    }

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
                        self.selected_bookmark_idx = Some((idx - 1).max(0) as usize);
                    } else {
                        self.last_unsaved = Some(self.input_state.content_str());
                        self.selected_bookmark_idx = Some(0);
                    }
                }
                KeyCode::Enter => {
                    if let Some(bookmark) = self
                        .selected_bookmark_idx
                        .and_then(|idx| self.bookmarks.bookmark_at(idx))
                        .cloned()
                    {
                        self.input_state.load_bookmark(bookmark);
                    }
                }
                _ => {}
            },
        }
    }
}

fn main() -> Result<(), failure::Error> {
    let bubblewrap_available = which::which("bwrap").is_ok();
    let unsafe_mode = std::env::args().any(|arg| arg == "--no-isolation");

    if !bubblewrap_available && !unsafe_mode {
        println!("bubblewrap installation not found. Please make sure you have `bwrap` on your path, or supply --no-isolation to disable safe-mode");
        std::process::exit(1);
    }

    let mut app = App::new(unsafe_mode);

    let bookmarks = bookmark::load_file().unwrap_or(BookmarkList::new());
    app.bookmarks = bookmarks;

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

            let bookmark_items: Vec<String> = app.bookmarks.as_strings();

            SelectableList::default()
                .block(make_default_block("Bookmarks", app.selected_area == UIArea::BookmarkList))
                .items(bookmark_items.as_slice())
                .select(if app.selected_area == UIArea::BookmarkList {
                    app.selected_bookmark_idx
                } else {
                    None
                })
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
                .style(if app.autoeval_mode {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                })
                .render(&mut f, input_field_rect);

            let output_text = [Text::raw(format!("{}", &app.command_output))];
            Paragraph::new(output_text.iter())
                .block(make_default_block("Output", false))
                .render(&mut f, exec_chunks[2]);

            if let Some(error) = &app.command_error {
                let error_text = [Text::raw(format!("{}", error))];
                Paragraph::new(error_text.iter())
                    .block(make_default_block("Stderr", false))
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
    println!("{}", app.input_state.content_str());
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

fn evaluate_command(unsafe_mode: bool, cmd: &str) -> (String, Option<String>) {
    if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("-i") || cmd.contains("dd ") {
        return (
            "".into(),
            Some("Will not evaluate this command. it's for your own safety, believe me....".into()),
        );
    }

    let output = if unsafe_mode {
        let args = "--ro-bind /usr /usr --symlink usr/lib64 /lib64 --tmpfs /tmp --proc /proc --dev /dev --ro-bind /etc /etc --die-with-parent --share-net --unshare-pid";
        let mut command = Command::new("bwrap");
        for arg in args.split(" ") {
            command.arg(arg);
        }
        command
            .arg("bash")
            .arg("-c")
            .arg(cmd)
            .output()
            .expect("Failed to execute process in bwrap. this might be a bwrap problem,... or not")
    } else {
        Command::new("bash")
            .arg("bash")
            .arg("-c")
            .arg("cmd")
            .output()
            .expect("failed to execute process")
    };
    let stdout = std::str::from_utf8(&output.stdout).unwrap().to_owned();
    let stderr = std::str::from_utf8(&output.stderr).unwrap().to_owned();
    (stdout, if stderr.is_empty() { None } else { Some(stderr) })
}
