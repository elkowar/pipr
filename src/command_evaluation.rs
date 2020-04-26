use futures::future::Either::*;
use futures::stream::StreamExt;
use std::process::Stdio;
use std::{str, time::Duration};
use tokio::io::{self, AsyncBufReadExt};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ExecutionMode {
    UNSAFE,
    ISOLATED,
}

pub struct CommandExecutionHandler {
    pub execution_mode: ExecutionMode,
    pub eval_environment: Vec<String>,
    pub cmd_timeout: Duration,
    pub cmd_out_receive: Receiver<CmdOutput>,
    cmd_in_send: Sender<String>,
    stop_send: Sender<()>,
}

pub enum CmdOutput {
    Ok(String),
    NotOk(String),
}

impl CommandExecutionHandler {
    pub fn start(cmd_timeout: Duration, execution_mode: ExecutionMode, eval_environment: Vec<String>) -> CommandExecutionHandler {
        let (cmd_in_send, mut cmd_in_receive) = mpsc::channel::<String>(10);
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
                        let child = match execution_mode {
                            ExecutionMode::UNSAFE => run_cmd_unsafe(&eval_environment, &new_cmd),
                            ExecutionMode::ISOLATED => run_cmd_isolated(&eval_environment, &new_cmd),
                        };
                        match child {
                            Ok(mut child) =>  {
                                out_lines_stream = Right(io::BufReader::new(child.stdout.take().unwrap()).lines());
                                err_lines_stream = Right(io::BufReader::new(child.stderr.take().unwrap()).lines());
                                handle = Right(tokio::time::timeout(cmd_timeout.into(), child));
                            }
                            Err(err) => cmd_out_send.send(CmdOutput::NotOk(err)).await.ok().unwrap(),
                        }
                    }

                    Some(out_line) = out_lines_stream.next() => out_lines.push_str(&(out_line.unwrap() + "\n")),
                    Some(err_line) = err_lines_stream.next() => err_lines.push_str(&(err_line.unwrap() + "\n")),

                    result = &mut handle => {
                        // resulting_output contains the command's output if everything went well,
                        // stderr if it exited non-zero, and if any other error occured information about that.
                        let resulting_output = match result {
                            Ok(Ok(result)) => {
                                if result.success() {
                                    if let Right(stream) = out_lines_stream {
                                        let pending_lines = stream.map(|x| x.unwrap()).collect::<Vec<String>>().await;
                                        out_lines.push_str(&pending_lines.join("\n"));
                                    }
                                    CmdOutput::Ok(out_lines)
                                } else {
                                    if let Right(stream) = err_lines_stream {
                                        let pending_lines = stream.map(|x| x.unwrap()).collect::<Vec<String>>().await;
                                        err_lines.push_str(&pending_lines.join("\n"));
                                    }
                                    CmdOutput::NotOk(err_lines)
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

    pub async fn execute(&mut self, cmd: &str) {
        self.cmd_in_send.send(cmd.into()).await.unwrap();
    }

    pub async fn stop(&mut self) {
        self.stop_send.send(()).await.unwrap();
    }
}

fn run_cmd_isolated(eval_environment: &Vec<String>, cmd: &str) -> Result<Child, String> {
    const BUBBLEWRAP_ARGS: &str =
        "--ro-bind / / --tmpfs /tmp --dev /dev --proc /proc --die-with-parent --share-net --unshare-pid";
    Command::new("bwrap")
        .args(BUBBLEWRAP_ARGS.split(" "))
        .args(eval_environment.into_iter())
        .arg(cmd)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "Unable to spawn command".to_string())
}

fn run_cmd_unsafe(eval_environment: &Vec<String>, cmd: &str) -> Result<Child, String> {
    if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("dd ") {
        return Err("Will not run this command, it's for your own good. Believe me.".to_string());
    }
    let mut eval_environment = eval_environment.into_iter();
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
