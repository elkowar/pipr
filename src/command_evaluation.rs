use futures::future::Either::*;
use futures::stream::StreamExt;
use std::process::Stdio;
use std::{io::BufRead, str, time::Duration};
use tokio::io::{self, AsyncBufReadExt};
use tokio::prelude::*;
use tokio::process::{Child, Command};
use tokio::sync::mpsc::{self, Receiver, Sender};

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
    pub cmd_timeout: Duration,
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
        let (cmd_in_send, mut cmd_in_receive) = mpsc::channel::<CommandExecutionRequest>(10);
        let (mut cmd_out_send, cmd_out_receive) = mpsc::channel::<CmdOutput>(10);
        let (stop_send, mut stop_receive) = mpsc::channel::<()>(10);

        let executor = CommandExecutionHandler {
            eval_environment: eval_environment.clone(),
            cmd_timeout,
            execution_mode,
            cmd_in_send,
            cmd_out_receive,
            stop_send,
        };

        tokio::spawn(async move {
            let mut handle = Left(futures::future::pending());
            let mut out_lines_stream = Left(futures::stream::pending());
            let mut err_lines_stream = Left(futures::stream::pending());
            let mut out_lines = String::new();
            let mut err_lines = String::new();

            loop {
                tokio::select! {
                    Some(new_cmd) = cmd_in_receive.recv() => {
                        let child = execution_mode.run_cmd_tokio(&eval_environment, &new_cmd.command);
                        match child {
                            Ok(mut child) =>  {

                                // this need's improving or at least some more testing
                                if let Some(stdin_content) = new_cmd.stdin {
                                    let mut stdin = child.stdin.take().unwrap();
                                    tokio::spawn(async move {
                                        for line in stdin_content {
                                            let _ = stdin.write_all(format!("{}\n", line).as_bytes()).await;
                                        }
                                    });
                                }

                                out_lines_stream = Right(io::BufReader::new(child.stdout.take().unwrap()).lines());
                                err_lines_stream = Right(io::BufReader::new(child.stderr.take().unwrap()).lines());
                                out_lines = String::new();
                                err_lines = String::new();
                                handle = Right(tokio::time::timeout(cmd_timeout, child));
                            }
                            Err(err) => cmd_out_send.send(CmdOutput::NotOk(err)).await.ok().unwrap(),
                        }
                    }

                    Some(line) = out_lines_stream.next() => {
                        match line {
                            Ok(line) => out_lines.push_str(&(line + "\n")),
                            Err(err) => {
                                cmd_out_send.send(CmdOutput::NotOk(format!("Error: {}", err))).await.ok().unwrap();
                                handle = Left(futures::future::pending());
                            }
                        }
                    }
                    Some(line) = err_lines_stream.next() => {
                        match line {
                            Ok(line) => err_lines.push_str(&(line + "\n")),
                            Err(err) => {
                                cmd_out_send.send(CmdOutput::NotOk(format!("Error: {}", err))).await.ok().unwrap();
                                handle = Left(futures::future::pending());
                            }
                        }
                    }

                    result = &mut handle => {
                        // resulting_output contains the command's output if everything went well,
                        // stderr if it exited non-zero, and if any other error occurred information about that.
                        let resulting_output = match result {
                            Ok(Ok(result)) => {
                                if result.success() {
                                    if let Right(stream) = out_lines_stream {
                                        let results = stream.collect::<Vec<Result<_, _>>>().await.into_iter().collect::<Result<Vec<_>, _>>();
                                        match results {
                                            Ok(pending_lines) => {
                                                out_lines.push_str(&pending_lines.join("\n"));
                                                CmdOutput::Ok(out_lines)
                                            }
                                            Err(err) => CmdOutput::NotOk(format!("{}", err)),
                                        }
                                    } else {
                                        CmdOutput::Ok(out_lines)
                                    }
                                } else {
                                    if let Right(stream) = err_lines_stream {
                                        let results = stream.collect::<Vec<Result<_, _>>>().await.into_iter().collect::<Result<Vec<_>, _>>();
                                        match results {
                                            Ok(pending_lines) => {
                                                err_lines.push_str(&pending_lines.join("\n"));
                                                CmdOutput::NotOk(err_lines)
                                            }
                                            Err(err) => CmdOutput::NotOk(format!("{}", err)),
                                        }
                                    } else {
                                        CmdOutput::NotOk(err_lines)
                                    }
                                }
                            },

                            Err(_) => CmdOutput::NotOk("Command timed out".to_string()),
                            Ok(Err(err)) => CmdOutput::NotOk(format!("Error running command: {}", err)),
                        };

                        cmd_out_send.send(resulting_output).await.ok().unwrap();

                        handle = Left(futures::future::pending());
                        out_lines_stream = Left(futures::stream::pending());
                        err_lines_stream = Left(futures::stream::pending());
                        out_lines = String::new();
                        err_lines = String::new();
                    }
                    Some(_) = stop_receive.recv() => break,
                };
            }
        });
        executor
    }

    /// execute a single command, returning it's output in this executors cmd_out channel
    pub async fn execute(&mut self, cmd: CommandExecutionRequest) {
        self.cmd_in_send.send(cmd).await.ok().unwrap();
    }

    /// stop the executor thread
    pub async fn stop(&mut self) {
        self.stop_send.send(()).await.unwrap();
    }
}

impl ExecutionMode {
    /// spawn an asynchronously running child using this executionMode, returning Err if something went wrong while spawning.
    /// the command has stdout, stderr and stdin as `Stdio::piped()`, so all are available.
    /// The child is also `kill_on_drop`, so it will be killed when the child value is dropped.
    fn run_cmd_tokio(&self, eval_environment: &[String], cmd: &str) -> Result<Child, String> {
        match self {
            ExecutionMode::ISOLATED => Command::new("bwrap")
                .args(BUBBLEWRAP_ARGS.split(' '))
                .args(eval_environment.iter())
                .arg(cmd)
                .stdout(Stdio::piped())
                .stdin(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
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
                    .kill_on_drop(true)
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
