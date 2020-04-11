#![feature(vec_remove_item)]
#[macro_use]
extern crate num_derive;
extern crate num_traits;
use std::io;
use std::io::Write;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Paragraph, SelectableList, Text, Widget};
use tui::{backend::CrosstermBackend, Terminal};

use crossterm::{
    cursor,
    event::{read, Event as CEvent, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub mod app;
pub mod bookmark;
pub mod command_evaluation;
pub mod lineeditor;

pub use app::*;
pub use bookmark::BookmarkList;
pub use command_evaluation::*;
pub use lineeditor as le;

fn main() -> Result<(), failure::Error> {
    let bubblewrap_available = which::which("bwrap").is_ok();
    let unsafe_mode = std::env::args().any(|arg| arg == "--no-isolation");

    if !bubblewrap_available && !unsafe_mode {
        println!("bubblewrap installation not found. Please make sure you have `bwrap` on your path, or supply --no-isolation to disable safe-mode");
        std::process::exit(1);
    }
    if unsafe_mode {
        run_app(App::new(UnsafeEnvironment::default()))
    } else {
        run_app(App::new(IsolatedEnvironment::default()))
    }
}

fn run_app<T>(mut app: App<T>) -> Result<(), failure::Error>
where
    T: ExecutionEnvironment,
{
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

            SelectableList::default()
                .block(make_default_block("Bookmarks", app.selected_area == UIArea::BookmarkList))
                .items(app.bookmarks.as_strings().as_slice())
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
                .constraints(
                    [
                        Length(2 + app.input_state.content_lines().len() as u16),
                        Length(3),
                        Percentage(100),
                    ]
                    .as_ref(),
                )
                .split(root_chunks[1]);

            input_field_rect = exec_chunks[0];

            {
                let command_input_style = if app.autoeval_mode {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                };

                List::new(app.input_state.content_lines().iter().map(|l| Text::raw(l)))
                    .block(make_default_block("Command", app.selected_area == UIArea::CommandInput).style(command_input_style))
                    .render(&mut f, input_field_rect);
            }

            {
                let output_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Percentage(50), Percentage(50)].as_ref())
                    .split(exec_chunks[2]);

                Paragraph::new([Text::raw(&app.command_output)].iter())
                    .block(make_default_block("Output", false))
                    .render(&mut f, output_chunks[0]);

                if let Some(error) = &app.command_error {
                    Paragraph::new([Text::raw(error)].iter())
                        .block(make_default_block("Stderr", false))
                        .render(&mut f, output_chunks[1]);
                }
            }

            {
                let config_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Percentage(50), Percentage(50)].as_ref())
                    .split(exec_chunks[1]);

                let immediate_eval_state = if app.autoeval_mode { "Active" } else { "Inactive" };
                Paragraph::new([Text::raw(immediate_eval_state)].iter())
                    .block(make_default_block("Immediate eval", app.selected_area == UIArea::Config))
                    .render(&mut f, config_chunks[0]);
            }
        })?;

        // move cursor to where it belongs.
        terminal.backend_mut().write(
            format!(
                "{}",
                cursor::MoveTo(
                    input_field_rect.x + 1 + app.input_state.displayed_cursor_column() as u16,
                    input_field_rect.y + 1 + app.input_state.cursor_line as u16,
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
                        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => running = false,
                        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => running = false,
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
