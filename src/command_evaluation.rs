use anyhow::{bail, Context};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const BUBBLEWRAP_ARGS: &[&str] = &[
    "--ro-bind",
    "/",
    "/",
    "--tmpfs",
    "/tmp",
    "--dev",
    "/dev",
    "--proc",
    "/proc",
    "--die-with-parent",
    "--share-net",
    "--unshare-pid",
];
const UNSAFE_COMMANDS: [&str; 3] = ["rm ", "mv ", "dd "];
const UNSAFE_CMD_ERR: &str = "Will not run this command, it's for your own good. Believe me.";
const SPAWN_ERR: &str = "Unable to spawn command";

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ExecutionMode {
    UNSAFE,
    ISOLATED,
}

/// Represents a command that should be executed, and an optional stdin that should be piped into it
pub struct CommandExecutionRequest {
    pub command: String,
    pub stdin: Option<Vec<String>>,
}

impl CommandExecutionRequest {
    pub fn new(command: String, stdin: Option<Vec<String>>) -> Self {
        Self { command, stdin }
    }
}

pub enum CmdOutput {
    Ok(String),
    NotOk(String),
}

pub struct CommandExecutionHandler {
    pub execution_mode: ExecutionMode,
    pub shell_command: Vec<String>,
    pub cmd_out_receive: Receiver<CmdOutput>,
    cmd_in_send: Sender<CommandExecutionRequest>,
    stop_send: Sender<()>,
}

impl CommandExecutionHandler {
    /// start a CommandExecutionHandler thread.
    ///
    /// `cmd_timeout` is the maximum time a command is allowed to run before being killed.
    /// `execution_mode` is the mode in which commands are executed (ISOLATED or UNSAFE).
    /// `shell_command` is the shell (or other environment) that the command should be executed in. I.e.: `["bash", "-c"]`
    pub fn start(cmd_timeout: Duration, execution_mode: ExecutionMode, shell_command: Vec<String>) -> Self {
        let (cmd_in_send, cmd_in_receive) = unbounded::<CommandExecutionRequest>();
        let (cmd_out_send, cmd_out_receive) = unbounded::<CmdOutput>();
        let (stop_send, stop_receive) = unbounded::<()>();

        let executor = Self {
            shell_command: shell_command.clone(),
            execution_mode,
            cmd_in_send,
            cmd_out_receive,
            stop_send,
        };

        thread::spawn(move || {
            let mut current_child: Option<Child> = None;
            let mut current_timeout: Option<(Instant, Duration)> = None;

            loop {
                // Check if we need to poll the current process
                if let (Some(child), Some((start_time, timeout_duration))) = (&mut current_child, current_timeout) {
                    if start_time.elapsed() > timeout_duration {
                        // Command timed out
                        let _ = child.kill();
                        cmd_out_send.send(CmdOutput::NotOk("Command timed out".to_string())).unwrap();
                        current_child = None;
                        current_timeout = None;
                    } else if let Ok(Some(status)) = child.try_wait() {
                        // Process has completed
                        let stdout = child.stdout.take().unwrap();
                        let stderr = child.stderr.take().unwrap();

                        let out_lines = read_lines_to_string(BufReader::new(stdout));
                        let err_lines = read_lines_to_string(BufReader::new(stderr));

                        let output = if status.success() {
                            CmdOutput::Ok(out_lines)
                        } else {
                            CmdOutput::NotOk(err_lines)
                        };

                        cmd_out_send.send(output).unwrap();
                        current_child = None;
                        current_timeout = None;
                    }
                }

                // Check for new commands or stop signals
                crossbeam_channel::select! {
                    recv(cmd_in_receive) -> msg => {
                        if let Ok(new_cmd) = msg {
                            match execution_mode.run_cmd_std(&shell_command, &new_cmd.command) {
                                Ok(mut child) => {
                                    // Handle stdin if provided
                                    if let Some(stdin_content) = new_cmd.stdin {
                                        if let Some(stdin) = &mut child.stdin {
                                            for line in stdin_content {
                                                let _ = writeln!(stdin, "{}", line);
                                            }
                                        }
                                    }

                                    current_child = Some(child);
                                    current_timeout = Some((Instant::now(), cmd_timeout));
                                }
                                Err(err) => cmd_out_send.send(CmdOutput::NotOk(err.to_string())).unwrap(),
                            }
                        }
                    },
                    recv(stop_receive) -> _ => break,
                    default(std::time::Duration::from_millis(10)) => {} // Small sleep to prevent busy waiting
                }
            }
        });

        executor
    }

    /// execute a single command, returning it's output in this executors cmd_out channel
    pub fn execute(&mut self, cmd: CommandExecutionRequest) {
        self.cmd_in_send.send(cmd).unwrap();
    }

    /// stop the executor thread
    pub fn stop(&mut self) {
        self.stop_send.send(()).unwrap();
    }
}

fn is_unsafe_command(cmd: &str) -> bool {
    UNSAFE_COMMANDS.iter().any(|&unsafe_cmd| cmd.contains(unsafe_cmd))
}

impl ExecutionMode {
    /// spawn a child process using this ExecutionMode, returning Err if something went wrong while spawning.
    /// the command has stdout, stderr and stdin as `Stdio::piped()`, so all are available.
    fn run_cmd_std(&self, shell_command: &[String], cmd: &str) -> anyhow::Result<Child> {
        let mut command = match self {
            ExecutionMode::ISOLATED => {
                let mut command = Command::new("bwrap");
                command.args(BUBBLEWRAP_ARGS).args(shell_command.iter());
                command
            }
            ExecutionMode::UNSAFE => {
                if is_unsafe_command(cmd) {
                    bail!(UNSAFE_CMD_ERR);
                }
                let mut eval_iter = shell_command.iter();
                let shell = eval_iter.next().context("shell_command is empty")?;
                let mut command = Command::new(shell);
                command.args(eval_iter);
                command
            }
        };
        command
            .arg(cmd)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(SPAWN_ERR)
    }

    /// blockingly run a command using this ExecutionMode, ignoring it's stderr.
    /// return's the stdout if everything went well, or an error message if there was a problem.
    pub fn run_cmd_blocking(&self, shell_command: &[String], cmd: &str) -> anyhow::Result<Vec<String>> {
        let mut child = self.run_cmd_std(shell_command, cmd)?;
        let stdout = BufReader::new(child.stdout.take().context("No child stdout available")?);
        let lines: Vec<String> = stdout.lines().filter_map(Result::ok).collect();
        let status = child.wait()?;
        if status.success() {
            Ok(lines)
        } else {
            bail!("Non-zero exit code: {}", status.code().unwrap_or(-1))
        }
    }
}

/// Read lines from a BufRead into a single string, ignoring all lines where reading failed.
fn read_lines_to_string<R: BufRead>(reader: R) -> String {
    reader.lines().filter_map(Result::ok).collect::<Vec<String>>().join("\n") + "\n"
}
