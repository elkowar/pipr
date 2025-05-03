use anyhow::{bail, Context};
use crossbeam_channel::{unbounded, Receiver, Sender};
use libc::SIGKILL;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

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
    Unsafe,
    /// Run commands in a sandboxed environment
    Isolated,
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
            let mut active_command: Option<BackgroundChildHandle> = None;

            loop {
                crossbeam_channel::select! {
                    recv(cmd_in_receive) -> msg => {
                        let Ok(new_cmd) = msg else { break; };
                        match spawn_command(&shell_command, &new_cmd.command, execution_mode) {
                            Ok(mut child) => {
                                if let Some(stdin_content) = new_cmd.stdin {
                                    let _ = write_stdin_to_child(&mut child, stdin_content);
                                }
                                if let Some(old_command) = active_command.take() {
                                    old_command.kill();
                                }
                                active_command = Some(wait_for_child_and_send_output(child, cmd_timeout, cmd_out_send.clone()));
                            }
                            Err(err) => cmd_out_send.send(CmdOutput::NotOk(err.to_string())).unwrap(),
                        }
                    },
                    recv(stop_receive) -> _ => {
                        if let Some(handle) = active_command.take() {
                            handle.kill();
                        }
                        break;
                    },
                };
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
        ExecutionMode::Isolated => {
            let mut command = Command::new("bwrap");
            command.args(BUBBLEWRAP_ARGS).args(shell_command.iter());
            command
        }
        ExecutionMode::Unsafe => {
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
    let lines: Vec<String> = stdout
        .lines()
        .collect::<Result<Vec<String>, _>>()
        .unwrap_or_else(|e| vec![e.to_string()]);

    let status = child.wait()?;

    if status.success() {
        Ok(lines)
    } else {
        bail!("Non-zero exit code: {}", status.code().unwrap_or(-1))
    }
}

/// Read lines from a BufRead into a single string, stopping on the first error
fn read_lines_to_string<R: BufRead>(reader: R) -> String {
    reader
        .lines()
        .collect::<Result<Vec<String>, _>>()
        .map(|x| x.join("\n") + "\n")
        .unwrap_or_else(|e| e.to_string())
}

fn write_stdin_to_child(child: &mut Child, stdin_content: Vec<String>) -> anyhow::Result<()> {
    if let Some(stdin) = &mut child.stdin {
        for line in stdin_content {
            writeln!(stdin, "{}", line)?;
        }
    }
    Ok(())
}

struct BackgroundChildHandle {
    pid: u32,
    /// Whether the child has already ended.
    /// If the child has been killed through the [`BackgroundChildHandle`], we don't want to handle its output at all.
    /// If it has already finished normally and sent its output, we don't want to actually kill it on [`Self::kill()`].
    already_killed: Arc<AtomicBool>,
}

impl BackgroundChildHandle {
    fn kill(&self) {
        if self.already_killed.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        unsafe {
            libc::kill(self.pid as i32, SIGKILL);
        }
        self.already_killed.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Wait for a child process to finish and send its output through the provided channel.
fn wait_for_child_and_send_output(
    mut child: Child,
    timeout: std::time::Duration,
    finished_channel: crossbeam_channel::Sender<CmdOutput>,
) -> BackgroundChildHandle {
    let pid = child.id();
    let already_killed = Arc::new(AtomicBool::new(false));
    let child_handle = BackgroundChildHandle {
        pid,
        already_killed: already_killed.clone(),
    };
    std::thread::spawn(move || {
        let status = child.wait_timeout(timeout);
        if already_killed.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        match status {
            Ok(Some(status)) => {
                let out_lines = read_lines_to_string(BufReader::new(child.stdout.take().unwrap()));
                let err_lines = read_lines_to_string(BufReader::new(child.stderr.take().unwrap()));
                let output = if status.success() {
                    CmdOutput::Ok(out_lines)
                } else {
                    CmdOutput::NotOk(err_lines)
                };
                finished_channel.send(output).unwrap();
            }
            Ok(None) => {
                finished_channel
                    .send(CmdOutput::NotOk("Command timed out".to_string()))
                    .unwrap();
            }
            Err(err) => {
                finished_channel.send(CmdOutput::NotOk(err.to_string())).unwrap();
            }
        }
        already_killed.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    child_handle
}
