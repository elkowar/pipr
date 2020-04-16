#[macro_use]
extern crate maplit;
extern crate getopts;
use getopts::Options;
use itertools::Itertools;
use std::env;
use std::io;
use std::io::Write;
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
pub mod command_template;
pub mod commandlist;
pub mod lineeditor;
pub mod pipr_config;
pub mod snippets;
pub mod ui;

pub use app::app::*;
pub use command_evaluation::*;
pub use commandlist::CommandList;
pub use lineeditor as le;
pub use pipr_config::*;

pub fn main() -> Result<(), failure::Error> {
    // arguments
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optopt("d", "default", "text inserted into the textfield on startup", "TEXT");
    opts.optflag(
        "",
        "no-isolation",
        "disable isolation. This will run the commands directly on your system, without protection. Take care.",
    );
    opts.optflag("h", "help", "print this help menu");
    opts.optflag(
        "",
        "config-reference",
        "print out the default configuration file, with comments",
    );

    let matches = opts.parse(&args[1..]).unwrap();

    let flag_help = matches.opt_present("help");
    let opt_default_input = matches.opt_str("default");
    let flag_no_isolation = matches.opt_present("no-isolation");
    let flag_config_reference = matches.opt_present("config-reference");

    if flag_help {
        let brief = format!("Usage: {} [options]", program);
        print!("{}", opts.usage(&brief));
        std::process::exit(0);
    } else if flag_config_reference {
        println!("{}", pipr_config::DEFAULT_CONFIG);
        std::process::exit(0);
    }

    // initialize

    pub const CONFIG_DIR_RELATIVE_TO_HOME: &'static str = ".config/pipr/";
    let config_path = Path::new(&env::var("HOME").unwrap()).join(CONFIG_DIR_RELATIVE_TO_HOME);

    let config = PiprConfig::load_from_file(&config_path.join("pipr.toml"));

    let execution_mode = if flag_no_isolation {
        ExecutionMode::UNSAFE
    } else {
        ExecutionMode::ISOLATED {
            additional_mounts: config.isolation_mounts_readonly.clone(),
            additional_path_entries: config.isolation_path_additions.clone(),
        }
    };

    let bubblewrap_available = which::which("bwrap").is_ok();

    if !bubblewrap_available && execution_mode != ExecutionMode::UNSAFE {
        println!("bubblewrap installation not found. Please make sure you have `bwrap` on your path, or supply --no-isolation to disable safe-mode");
        std::process::exit(1);
    }

    let executor = Executor::start_executor(execution_mode, config.eval_environment.clone());

    let bookmarks = CommandList::load_from_file(config_path.join("bookmarks"), None);
    let history = CommandList::load_from_file(config_path.join("history"), Some(config.history_size));

    // create app and set default

    let mut app = App::new(executor, config.clone(), bookmarks, history);

    if let Some(default_value) = opt_default_input {
        app.input_state.set_content(default_value.lines().map_into().collect());
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

    std::panic::set_hook(Box::new(|data| {
        disable_raw_mode().unwrap();
        #[allow(deprecated)]
        execute!(io::stdout(), LeaveAlternateScreen).unwrap();
        eprintln!("{}", data);
    }));

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
