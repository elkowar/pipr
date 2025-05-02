use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use crossbeam_channel::{unbounded, Receiver, Sender};

const BUBBLEWRAP_ARGS: &str = "--ro-bind / / --tmpfs /tmp --dev /dev --proc /proc --die-with-parent --share-net --unshare-pid";

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
        CommandExecutionRequest { command, stdin }
    }
}

pub struct CommandExecutionHandler {
    pub execution_mode: ExecutionMode,
    pub eval_environment: Vec<String>,
    pub cmd_out_receive: Receiver<CmdOutput>,
    cmd_in_send: Sender<CommandExecutionRequest>,
    stop_send: Sender<()>,
}

pub enum CmdOutput {
    Ok(String),
    NotOk(String),
}

impl CommandExecutionHandler {
    /// start a CommandExecutionHandler thread.
    pub fn start(cmd_timeout: Duration, execution_mode: ExecutionMode, eval_environment: Vec<String>) -> CommandExecutionHandler {
        let (cmd_in_send, cmd_in_receive) = unbounded::<CommandExecutionRequest>();
        let (cmd_out_send, cmd_out_receive) = unbounded::<CmdOutput>();
        let (stop_send, stop_receive) = unbounded::<()>();

        let executor = CommandExecutionHandler {
            eval_environment: eval_environment.clone(),
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
                if let Some(child) = &mut current_child {
                    if let Some((start_time, timeout_duration)) = current_timeout {
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
                            
                            let stdout_reader = BufReader::new(stdout);
                            let stderr_reader = BufReader::new(stderr);
                            
                            let out_lines: String = stdout_reader.lines()
                                .filter_map(Result::ok)
                                .collect::<Vec<String>>()
                                .join("\n") + "\n";
                                
                            let err_lines: String = stderr_reader.lines()
                                .filter_map(Result::ok)
                                .collect::<Vec<String>>()
                                .join("\n") + "\n";
                            
                            if status.success() {
                                cmd_out_send.send(CmdOutput::Ok(out_lines)).unwrap();
                            } else {
                                cmd_out_send.send(CmdOutput::NotOk(err_lines)).unwrap();
                            }
                            
                            current_child = None;
                            current_timeout = None;
                        }
                    }
                }
                
                // Check for new commands or stop signals
                crossbeam_channel::select! {
                    recv(cmd_in_receive) -> msg => {
                        if let Ok(new_cmd) = msg {
                            let child = execution_mode.run_cmd_std(&eval_environment, &new_cmd.command);
                            match child {
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
                                Err(err) => cmd_out_send.send(CmdOutput::NotOk(err)).unwrap(),
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

impl ExecutionMode {
    /// spawn a child process using this executionMode, returning Err if something went wrong while spawning.
    /// the command has stdout, stderr and stdin as `Stdio::piped()`, so all are available.
    fn run_cmd_std(&self, eval_environment: &[String], cmd: &str) -> Result<Child, String> {
        match self {
            ExecutionMode::ISOLATED => Command::new("bwrap")
                .args(BUBBLEWRAP_ARGS.split(' '))
                .args(eval_environment.iter())
                .arg(cmd)
                .stdout(Stdio::piped())
                .stdin(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|_| "Unable to spawn command".to_string()),

            ExecutionMode::UNSAFE => {
                if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("dd ") {
                    return Err("Will not run this command, it's for your own good. Believe me.".to_string());
                }
                let mut eval_environment = eval_environment.iter();
                Command::new(eval_environment.next().expect("eval_environment is empty"))
                    .args(eval_environment)
                    .arg(cmd)
                    .stdout(Stdio::piped())
                    .stdin(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|_| "Unable to spawn command".to_string())
            }
        }
    }

    /// blockingly run a command using this executionmode, ignoring it's stderr.
    /// return's the stdout if everything went well, or an error message if there was a problem.
    pub fn run_cmd_blocking(&self, eval_environment: &[String], cmd: &str) -> Result<Vec<String>, String> {
        // TODO respect stderr, check exit code and clean up
        match self {
            ExecutionMode::ISOLATED => std::process::Command::new("bwrap")
                .args(BUBBLEWRAP_ARGS.split(' '))
                .args(eval_environment.iter())
                .arg(cmd)
                .stdout(Stdio::piped())
                .stdin(Stdio::null()) // stdin is unused
                .stderr(Stdio::null()) // stderr is ignored
                .spawn()
                .and_then(|mut child| {
                    let stdout = std::io::BufReader::new(child.stdout.as_mut().unwrap()).lines().collect();
                    if child.wait()?.success() {
                        stdout
                    } else {
                        Err(std::io::Error::new(std::io::ErrorKind::Other, "Non-zero exit code"))
                    }
                })
                .map_err(|err| format!("{}", err)),

            ExecutionMode::UNSAFE => {
                if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("dd ") {
                    return Err("Will not run this command, it's for your own good. Believe me.".to_string());
                }
                let mut eval_environment = eval_environment.iter();
                std::process::Command::new(eval_environment.next().expect("eval_environment is empty"))
                    .args(eval_environment)
                    .arg(cmd)
                    .stdout(Stdio::piped())
                    .stdin(Stdio::null()) // stdin is unused
                    .stderr(Stdio::null()) // stderr is ignored
                    .spawn()
                    .and_then(|mut child| {
                        let stdout = std::io::BufReader::new(child.stdout.as_mut().unwrap()).lines().collect();
                        if child.wait()?.success() {
                            stdout
                        } else {
                            Err(std::io::Error::new(std::io::ErrorKind::Other, "Non-zero exit code"))
                        }
                    })
                    .map_err(|err| format!("{}", err))
            }
        }
    }
}