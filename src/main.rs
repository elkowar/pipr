use atty::Stream;
use crossbeam_channel::{select, unbounded, Receiver};
use getopts::Options;
use itertools::Itertools;
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use ratatui::{backend::CrosstermBackend, Terminal};

use crossterm::{
    event::{self, Event as CEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

mod app;
mod command_evaluation;
mod command_template;
mod commandlist;
mod lineeditor;
mod pipr_config;
mod snippets;
pub mod ui;
mod util;

use app::App;
use command_evaluation::*;
use commandlist::CommandList;
use pipr_config::*;

pub struct CliArgs {
    default_content: Option<String>,
    output_file: Option<String>,
    input_file: Option<String>,
    unsafe_mode: bool,
    raw_mode: bool,
}

fn main() -> anyhow::Result<()> {
    let args = handle_cli_arguments();
    let home_path = env::var("HOME").expect("$HOME not set");
    let config_path = &env::var("XDG_CONFIG_HOME")
        .map(|dir| Path::new(&dir).to_path_buf())
        .unwrap_or(Path::new(&home_path).join(".config"))
        .join("pipr");

    let config = PiprConfig::load_from_file(&config_path.join("pipr.toml"));

    let execution_mode = if args.unsafe_mode {
        ExecutionMode::Unsafe
    } else {
        ExecutionMode::Isolated
    };

    let bubblewrap_available = which::which("bwrap").is_ok();

    if !bubblewrap_available && execution_mode != ExecutionMode::Unsafe {
        println!("bubblewrap installation not found. Please make sure you have `bwrap` on your path, or supply --no-isolation to disable safe-mode");
        std::process::exit(1);
    }

    let execution_handler = CommandExecutionHandler::start(config.cmd_timeout, execution_mode, config.eval_environment.clone());

    let bookmarks = CommandList::load_from_file(config_path.join("bookmarks"), None);
    let history = CommandList::load_from_file(config_path.join("history"), Some(config.history_size));

    // create app and set default
    let mut app = App::new(execution_handler, args.raw_mode, config.clone(), bookmarks, history);

    if let Some(default_value) = args.default_content {
        app.input_state.set_content(default_value.lines().map_into().collect());
    }
    if let Some(input_file) = args.input_file {
        let mut buffer = String::new();
        File::open(input_file)?.read_to_string(&mut buffer)?;
        app.input_state.set_content(buffer.lines().map_into().collect());
    }

    // render on stdout if output is not piped into something. if it is, use stderr.
    if atty::is(Stream::Stdout) {
        run_app(&mut app, io::stdout())?;
    } else {
        run_app(&mut app, io::stderr())?;
    }

    after_finish(&app, args.output_file)?;

    Ok(())
}

/// parses the arguments, handles printing help and config-reference if requested
/// and otherwise returns an CliArgs instance.
fn handle_cli_arguments() -> CliArgs {
    let cli_args: Vec<String> = env::args().collect();
    let program = cli_args[0].clone();

    // arguments
    let mut opts = Options::new();
    opts.optopt("d", "default", "text inserted into the textfield on startup", "TEXT");
    opts.optopt("o", "out-file", "write final command to file", "FILE");
    opts.optopt("", "in-file", "read initial command from file", "FILE");
    opts.optflag("", "config-reference", "print out the default configuration file");
    opts.optflag("r", "raw-mode", "keep linebreaks in finished command when closing");
    opts.optflag(
        "",
        "no-isolation",
        "disable isolation. This will run the commands directly on your system, without protection. Take care.",
    );
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(&cli_args[1..]) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}: {}", program, e);
            std::process::exit(1);
        }
    };

    if matches.opt_present("help") {
        let brief = format!("Usage: {} [options]", program);
        print!("{}", opts.usage(&brief));
        std::process::exit(0);
    } else if matches.opt_present("config-reference") {
        println!("{}", pipr_config::DEFAULT_CONFIG);
        std::process::exit(0);
    }

    CliArgs {
        default_content: matches.opt_str("default"),
        output_file: matches.opt_str("out-file"),
        input_file: matches.opt_str("in-file"),
        unsafe_mode: matches.opt_present("no-isolation"),
        raw_mode: matches.opt_present("raw-mode"),
    }
}

/// executed after the program has been closed.
/// optionally given out_file, a path to a file that the
/// final command will be written to (mostly for scripting stuff)
fn after_finish(app: &App, out_file: Option<String>) -> anyhow::Result<()> {
    let finished_command = if app.raw_mode {
        app.input_state.content_lines().join("\n")
    } else {
        app.input_state.content_str()
    };

    if let Some(finish_hook) = &app.config.finish_hook {
        let mut finish_hook = finish_hook.split(' ');
        if let Some(cmd) = finish_hook.next() {
            let mut child = Command::new(cmd).args(finish_hook).stdin(Stdio::piped()).spawn()?;
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(finished_command.as_bytes())?;
            child.wait()?;
        }
    }

    println!("{}", finished_command);
    if let Some(out_file) = out_file {
        File::create(out_file)?.write_all(finished_command.as_bytes())?;
    }
    Ok(())
}

// Start a thread that reads events from crossterm and sends them through a channel
fn spawn_event_reader_thread() -> Receiver<CEvent> {
    let (sender, receiver) = unbounded();

    thread::spawn(move || {
        while let Ok(event) = event::read() {
            // If sending fails, the channel is closed, so exit the thread
            if sender.send(event).is_err() {
                break;
            }
        }
    });

    receiver
}

fn run_app<W: Write>(app: &mut App, mut output_stream: W) -> anyhow::Result<()> {
    execute!(output_stream, EnterAlternateScreen)?;
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(output_stream);
    let mut terminal = Terminal::new(backend)?;

    std::panic::set_hook(Box::new(|data| {
        disable_raw_mode().unwrap();
        execute!(io::stdout(), LeaveAlternateScreen).unwrap();
        execute!(io::stderr(), LeaveAlternateScreen).unwrap();
        eprintln!("{}", data);
        std::process::exit(1);
    }));

    let mut all_errors = Vec::new();

    // Create a tick channel
    let (tick_sender, tick_receiver) = unbounded();
    thread::spawn(move || {
        let tick_interval = Duration::from_millis(100);
        loop {
            thread::sleep(tick_interval);
            if tick_sender.send(()).is_err() {
                break;
            }
        }
    });

    // Create an event reader thread
    let event_receiver = spawn_event_reader_thread();

    while !app.should_quit {
        let draw_result = ui::draw_app(&mut terminal, app);
        if let Err(err) = draw_result {
            all_errors.push(format!("{}", err));
        }

        select! {
            recv(app.execution_handler.cmd_out_receive) -> msg => {
                if let Ok(cmd_output) = msg {
                    app.on_cmd_output(cmd_output);
                }
            },
            recv(tick_receiver) -> _ => {
                app.on_tick();
            },
            recv(event_receiver) -> msg => {
                if let Ok(CEvent::Key(key_evt)) = msg {
                    app.on_tui_event(key_evt.code, key_evt.modifiers);
                }
            }
        }
    }

    app.execution_handler.stop();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    std::io::Write::flush(&mut terminal.backend_mut())?;
    if !all_errors.is_empty() {
        eprintln!("{}", all_errors.join("\n"));
    }
    Ok(())
}
