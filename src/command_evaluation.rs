use anyhow::{bail, Context};
use crossbeam_channel::{unbounded, Receiver, RecvError, Sender};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

// Constants for command execution
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

/// Execution mode for commands
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ExecutionMode {
    /// Run commands directly without isolation (potentially dangerous)
    UNSAFE,
    /// Run commands in a sandboxed environment
    ISOLATED,
}

/// Represents a command that should be executed, with optional stdin
pub struct CommandExecutionRequest {
    pub command: String,
    pub stdin: Option<Vec<String>>,
}

impl CommandExecutionRequest {
    /// Create a new command execution request
    pub fn new(command: String, stdin: Option<Vec<String>>) -> Self {
        Self { command, stdin }
    }
}

/// Output from an executed command
pub enum CmdOutput {
    /// Command executed successfully with output
    Ok(String),
    /// Command failed with error message
    NotOk(String),
}

/// Handles command execution in a separate thread
pub struct CommandExecutionHandler {
    pub execution_mode: ExecutionMode,
    pub shell_command: Vec<String>,
    pub cmd_out_receive: Receiver<CmdOutput>,
    cmd_in_send: Sender<CommandExecutionRequest>,
    stop_send: Sender<()>,
}

impl CommandExecutionHandler {
    /// Start a CommandExecutionHandler thread.
    ///
    /// # Arguments
    /// * `cmd_timeout` - Maximum time a command is allowed to run before being killed
    /// * `execution_mode` - Mode in which commands are executed (ISOLATED or UNSAFE)
    /// * `shell_command` - Shell command to execute commands with (e.g., `["bash", "-c"]`)
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
            let mut active_command: Option<(Child, Instant, Duration)> = None;

            loop {
                enum Event {
                    CommandExecutionRequest(Result<CommandExecutionRequest, RecvError>),
                    StopReceived,
                    RecheckCommandOutput,
                }

                // Wait for messages, with or without a timeout depending on whether we have an active command
                let select_result = if active_command.is_some() {
                    // We have an active command - wait with timeout so we can check its status regularly
                    crossbeam_channel::select! {
                        recv(cmd_in_receive) -> msg => Event::CommandExecutionRequest(msg),
                        recv(stop_receive) -> _ => Event::StopReceived,
                        default(Duration::from_millis(100)) => Event::RecheckCommandOutput // Just a timeout to check process status
                    }
                } else {
                    // No active command - wait indefinitely for a new command or stop signal
                    crossbeam_channel::select! {
                        recv(cmd_in_receive) -> msg => Event::CommandExecutionRequest(msg),
                        recv(stop_receive) -> _ => Event::StopReceived
                    }
                };

                match select_result {
                    Event::CommandExecutionRequest(Ok(new_cmd)) => {
                        // Got a new command request
                        match spawn_command(&shell_command, &new_cmd.command, execution_mode) {
                            Ok(mut child) => {
                                // Handle stdin if provided
                                if let Some(stdin_content) = new_cmd.stdin {
                                    if let Some(stdin) = &mut child.stdin {
                                        for line in stdin_content {
                                            let _ = writeln!(stdin, "{}", line);
                                        }
                                    }
                                }

                                // Set up the command with its execution timeout
                                active_command = Some((child, Instant::now(), cmd_timeout));
                            }
                            Err(err) => cmd_out_send.send(CmdOutput::NotOk(err.to_string())).unwrap(),
                        }
                    }
                    Event::StopReceived => break,
                    Event::RecheckCommandOutput | Event::CommandExecutionRequest(Err(_)) => {
                        if let Some((mut child, start_time, timeout)) = active_command.take() {
                            // Check if command has timed out
                            if start_time.elapsed() >= timeout {
                                let _ = child.kill();
                                cmd_out_send.send(CmdOutput::NotOk("Command timed out".to_string())).unwrap();
                                active_command = None;
                            } else {
                                // Use wait_timeout to efficiently wait for process or timeout
                                match child.try_wait() {
                                    Ok(Some(status)) => {
                                        // Process has completed
                                        let out_lines = read_lines_to_string(BufReader::new(child.stdout.take().unwrap()));
                                        let err_lines = read_lines_to_string(BufReader::new(child.stderr.take().unwrap()));
                                        let output = if status.success() {
                                            CmdOutput::Ok(out_lines)
                                        } else {
                                            CmdOutput::NotOk(err_lines)
                                        };
                                        cmd_out_send.send(output).unwrap();
                                    }
                                    Ok(None) => {
                                        // Process is still running, put it back
                                        active_command = Some((child, start_time, timeout));
                                    }
                                    Err(e) => {
                                        // Error checking status
                                        cmd_out_send
                                            .send(CmdOutput::NotOk(format!("Error waiting for process: {}", e)))
                                            .unwrap();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        executor
    }

    /// Execute a single command, sending its output to this executor's cmd_out channel
    pub fn execute(&mut self, cmd: CommandExecutionRequest) {
        self.cmd_in_send.send(cmd).unwrap();
    }

    /// Stop the executor thread
    pub fn stop(&mut self) {
        self.stop_send.send(()).unwrap();
    }
}

/// Check if a command contains potentially unsafe operations
fn is_unsafe_command(cmd: &str) -> bool {
    UNSAFE_COMMANDS.iter().any(|&unsafe_cmd| cmd.contains(unsafe_cmd))
}

/// Spawn a child process with the given command, using the specified execution mode
///
/// Returns a Child process with piped stdin, stdout, and stderr
pub fn spawn_command(shell_command: &[String], cmd: &str, mode: ExecutionMode) -> anyhow::Result<Child> {
    let mut command = match mode {
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

/// Execute a command and block until it completes
///
/// Returns the command output as a vector of strings, or an error if execution fails
pub fn execute_command_blocking(shell_command: &[String], cmd: &str, mode: ExecutionMode) -> anyhow::Result<Vec<String>> {
    let mut child = spawn_command(shell_command, cmd, mode)?;
    let stdout = BufReader::new(child.stdout.take().context("No child stdout available")?);
    let lines: Vec<String> = stdout.lines().filter_map(Result::ok).collect();
    let status = child.wait()?;

    if status.success() {
        Ok(lines)
    } else {
        bail!("Non-zero exit code: {}", status.code().unwrap_or(-1))
    }
}

/// Read lines from a BufRead into a single string, ignoring all lines with read errors
fn read_lines_to_string<R: BufRead>(reader: R) -> String {
    reader.lines().filter_map(Result::ok).collect::<Vec<String>>().join("\n") + "\n"
}
