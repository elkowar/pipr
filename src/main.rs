#![feature(vec_remove_item)]
#[macro_use]
extern crate num_derive;
extern crate num_traits;
use std::io;
use std::io::Write;

use tui::{backend::CrosstermBackend, Terminal};

use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub mod app;
pub mod bookmark;
pub mod command_evaluation;
pub mod lineeditor;
pub mod ui;

pub use app::*;
pub use bookmark::BookmarkList;
pub use command_evaluation::*;
pub use lineeditor as le;

fn main() -> Result<(), failure::Error> {
    let bubblewrap_available = which::which("bwrap").is_ok();
    let execution_mode = if std::env::args().any(|arg| arg == "--no-isolation") {
        ExecutionMode::UNSAFE
    } else {
        ExecutionMode::ISOLATED
    };

    if !bubblewrap_available && execution_mode == ExecutionMode::ISOLATED {
        println!("bubblewrap installation not found. Please make sure you have `bwrap` on your path, or supply --no-isolation to disable safe-mode");
        std::process::exit(1);
    }
    let executor = Executor::start_executor(execution_mode);
    let app = App::new(executor);
    run_app(app)
}

fn run_app(mut app: App) -> Result<(), failure::Error> {
    let bookmarks = bookmark::load_file().unwrap_or(BookmarkList::new());
    app.bookmarks = bookmarks;

    let mut stdout = io::stdout();
    #[allow(deprecated)]
    execute!(stdout, EnterAlternateScreen)?;
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    while !app.should_quit {
        ui::draw_app(&mut terminal, &mut app)?;

        loop {
            if let Some(cmd_output) = app.executor.poll_output() {
                app.apply_cmd_output(cmd_output);
                break;
            }

            if let Ok(true) = event::poll(std::time::Duration::from_millis(100)) {
                match event::read()? {
                    CEvent::Key(KeyEvent { code, modifiers }) => {
                        match code {
                            KeyCode::Esc => app.should_quit = true,
                            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.should_quit = true,
                            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => app.should_quit = true,
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
    }
    disable_raw_mode()?;
    #[allow(deprecated)]
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    std::io::Write::flush(&mut terminal.backend_mut())?;
    Ok(())
}
