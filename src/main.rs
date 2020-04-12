#![feature(vec_remove_item)]
extern crate clap;
extern crate lazy_static;
extern crate num_derive;
extern crate num_traits;

use clap::Arg;
use itertools::Itertools;
use std::env;
use std::io::Write;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use tui::{backend::CrosstermBackend, Terminal};

use crossterm::{
    event::{self, Event as CEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub mod app;
pub mod command_evaluation;
pub mod commandlist;
pub mod lineeditor;
pub mod pipr_config;
pub mod ui;

pub use app::*;
pub use command_evaluation::*;
pub use commandlist::CommandList;
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

    let config = PiprConfig::load_from_file();

    let bubblewrap_available = which::which("bwrap").is_ok();
    let execution_mode = match matches.is_present("no-isolation") {
        true => ExecutionMode::UNSAFE,
        false => ExecutionMode::ISOLATED(config.isolation_mounts_readonly.clone()),
    };

    if !bubblewrap_available && execution_mode != ExecutionMode::UNSAFE {
        println!("bubblewrap installation not found. Please make sure you have `bwrap` on your path, or supply --no-isolation to disable safe-mode");
        std::process::exit(1);
    }

    let executor = Executor::start_executor(execution_mode);

    pub const CONFIG_DIR_RELATIVE_TO_HOME: &'static str = ".config/pipr/";
    let home_path = env::var("HOME").unwrap();
    let config_path = Path::new(&home_path).join(CONFIG_DIR_RELATIVE_TO_HOME);

    let bookmarks = CommandList::load_from_file(config_path.join("bookmarks"), None);
    let history = CommandList::load_from_file(config_path.join("history"), Some(config.history_size));

    let mut app = App::new(executor, config.clone(), bookmarks, history);
    if let Some(default_value) = matches.value_of("default") {
        app.input_state.set_content(&default_value.lines().map_into().collect());
    }

    run_app(&mut app)?;

    after_finish(&app)?;

    Ok(())
}

fn after_finish(app: &App) -> Result<(), failure::Error> {
    if let Some(finish_hook) = &app.config.finish_hook {
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
                    CEvent::Resize(_, _) => break,
                    CEvent::Key(evt) => {
                        app.on_tui_event(evt.code, evt.modifiers);
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
