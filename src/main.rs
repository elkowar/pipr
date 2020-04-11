#![feature(vec_remove_item)]
#[macro_use]
extern crate num_derive;
extern crate lazy_static;
extern crate num_traits;
use itertools::Itertools;
use std::io::Stdin;
use std::io::Write;
use std::io::{self, Read};
use std::process::{Command, Stdio};

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
pub mod pipr_config;
pub mod ui;

pub use app::*;
pub use bookmark::BookmarkList;
pub use command_evaluation::*;
pub use lineeditor as le;
pub use pipr_config::*;

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

    let has_stdin_default = std::env::args().any(|arg| arg == "--stdin-default");
    let default_content = if has_stdin_default {
        let mut supplied_input = String::new();
        io::stdin().read_to_string(&mut supplied_input).unwrap();
        supplied_input
    } else {
        "".into()
    };

    let config = PiprConfig::load_from_file();

    let executor = Executor::start_executor(execution_mode);
    let mut app = App::new(executor, config.clone());
    if !default_content.is_empty() {
        app.input_state
            .set_content(&default_content.lines().map(|x| x.into()).collect());
    }
    run_app(&mut app)?;

    if let Some(finish_hook) = config.finish_hook {
        let finish_hook = finish_hook.split(" ").collect::<Vec<&str>>();
        if let Some(cmd) = finish_hook.first() {
            let mut command = Command::new(cmd);
            for arg in finish_hook.iter().dropping(1) {
                command.arg(arg);
            }
            command.stdin(Stdio::piped());

            let mut child = command.spawn()?;
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(&app.input_state.content_str().as_bytes())?;
            child.wait()?;
        }
    }

    println!("{}", app.input_state.content_str());

    Ok(())
}

fn run_app(mut app: &mut App) -> Result<(), failure::Error> {
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
                app.on_cmd_output(cmd_output);
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
