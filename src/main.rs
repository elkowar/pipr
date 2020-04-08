#[macro_use]
extern crate num_derive;
extern crate num_traits;

use num_traits::FromPrimitive;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::env;
use std::io;
use std::io::Write;
use std::process::Command;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Row, Table, Text, Widget};
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
    Output,
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
}

impl App {
    fn new() -> App {
        App {
            selected_area: UIArea::CommandInput,
            input_state: le::EditorState::new(),
            command_output: "".into(),
            command_error: None,
            autoeval_mode: false,
        }
    }

    fn apply_command_result(&mut self, res: (String, Option<String>)) {
        let (stdout, stderr) = res;
        if stderr == None {
            self.command_output = stdout;
        }
        self.command_error = stderr;
    }

    fn apply_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if code == KeyCode::Tab {
            self.selected_area = self.selected_area.next_area();
            return;
        }
        match self.selected_area {
            UIArea::CommandInput => {
                let previous_content = self.input_state.content_str();
                if modifiers.contains(KeyModifiers::CONTROL) {
                    match code {
                        KeyCode::Char('w') => self.input_state.apply_event(le::EditorEvent::KillWordBack),
                        _ => {}
                    }
                } else {
                    match code {
                        KeyCode::Up => {
                            self.autoeval_mode = !self.autoeval_mode;
                        }
                        KeyCode::Char(c) => {
                            self.input_state.apply_event(le::EditorEvent::NewCharacter(c.to_string()));
                        }
                        KeyCode::Backspace => {
                            self.input_state.apply_event(le::EditorEvent::Backspace);
                        }
                        KeyCode::Delete => {
                            self.input_state.apply_event(le::EditorEvent::Delete);
                        }
                        KeyCode::Left => {
                            self.input_state.apply_event(le::EditorEvent::GoLeft);
                        }
                        KeyCode::Right => {
                            self.input_state.apply_event(le::EditorEvent::GoRight);
                        }
                        KeyCode::Enter => {
                            self.apply_command_result(evaluate_command(&self.input_state.content_str()));
                        }
                        _ => {}
                    }
                }
                if previous_content != self.input_state.content_str() && self.autoeval_mode {
                    self.apply_command_result(evaluate_command(&self.input_state.content_str()));
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), failure::Error> {
    let mut app = App::new();

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut running = true;
    while running {
        let mut input_field_rect = tui::layout::Rect::new(0, 0, 0, 0);

        terminal.draw(|mut f| {
            use Constraint::*;
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Length(3), Percentage(50), Percentage(50)].as_ref())
                .split(f.size());

            input_field_rect = chunks[0];

            let input_text = [Text::raw(format!("{}", &app.input_state.content_str()))];

            Paragraph::new(input_text.iter())
                .block(make_default_block("Command", app.selected_area == UIArea::CommandInput))
                .render(&mut f, chunks[0]);

            let output_text = [Text::raw(format!("{}", &app.command_output))];
            Paragraph::new(output_text.iter())
                .block(make_default_block("Output", app.selected_area == UIArea::Output))
                .render(&mut f, chunks[1]);

            if let Some(error) = &app.command_error {
                let error_text = [Text::raw(format!("{}", error))];
                Paragraph::new(error_text.iter())
                    .block(make_default_block("Stderr", app.selected_area == UIArea::Output))
                    .render(&mut f, chunks[2]);
            }
        })?;

        terminal.backend_mut().write(
            format!(
                "{}",
                cursor::MoveTo(
                    input_field_rect.x + 1 + app.input_state.cursor_col as u16,
                    input_field_rect.y + 1
                )
            )
            .as_bytes(),
        )?;

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
