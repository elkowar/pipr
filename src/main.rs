#![feature(vec_remove_item)]
#[macro_use]
extern crate num_derive;
extern crate clap;
extern crate lazy_static;
extern crate num_traits;

use clap::Arg;
use itertools::Itertools;
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
    let matches = clap::App::new("Pipr")
        .arg(Arg::with_name("stdin-default").long("stdin-default"))
        .arg(Arg::with_name("no-isolation").long("no-isolation"))
        .arg(Arg::with_name("default").long("default").default_value("").takes_value(true))
        .get_matches();

    if matches.is_present("stdin-default") {
        let mut supplied_input = String::new();
        io::stdin().read_to_string(&mut supplied_input).unwrap();
        let current_exe = std::env::current_exe()?;
        dbg!(&current_exe);
        let mut child_command = Command::new(current_exe);
        child_command.arg("--default").arg(supplied_input);
        for arg in std::env::args().skip(1).filter(|x| x != "--stdin-default") {
            child_command.arg(arg);
        }
        child_command.spawn()?.wait_with_output()?;
        return Ok(());
    }

    let bubblewrap_available = which::which("bwrap").is_ok();
    let execution_mode = match matches.is_present("no-isolation") {
        true => ExecutionMode::UNSAFE,
        false => ExecutionMode::ISOLATED,
    };

    if !bubblewrap_available && execution_mode == ExecutionMode::ISOLATED {
        println!("bubblewrap installation not found. Please make sure you have `bwrap` on your path, or supply --no-isolation to disable safe-mode");
        std::process::exit(1);
    }

    let config = PiprConfig::load_from_file();

    let executor = Executor::start_executor(execution_mode);
    let mut app = App::new(executor, config.clone());
    if let Some(default_value) = matches.value_of("default") {
        app.input_state.set_content(&default_value.lines().map_into().collect());
    }
    run_app(&mut app)?;

    if let Some(finish_hook) = config.finish_hook {
        let finish_hook = finish_hook.split(" ").collect::<Vec<&str>>();
        if let Some(cmd) = finish_hook.first() {
            let mut command = Command::new(cmd);
            for arg in finish_hook.iter().dropping(1) {
                command.arg(arg);
            }
            let mut child = command.stdin(Stdio::piped()).spawn()?;
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
