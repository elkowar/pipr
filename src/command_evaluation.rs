//use std::process::{Child, Command, Stdio};
use std::process::Stdio;
use std::str;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::stream::StreamExt;
use tokio::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ExecutionMode {
    UNSAFE,
    ISOLATED,
}

pub struct CommandExecutionHandler {
    pub execution_mode: ExecutionMode,
    pub eval_environment: Vec<String>,
    cmd_out_receive: Receiver<CmdOutput>,
    cmd_in_send: Sender<String>,
    stop_send: Sender<()>,
}

pub enum CmdOutput {
    Finish,
    Stdout(String),
    Stderr(String),
}

impl CommandExecutionHandler {
    pub fn start(execution_mode: ExecutionMode, eval_environment: Vec<String>) -> CommandExecutionHandler {
        let (mut cmd_in_send, mut cmd_in_receive) = mpsc::channel::<String>(10);
        let (mut cmd_out_send, mut cmd_out_receive) = mpsc::channel::<CmdOutput>(10);
        let (mut stop_send, mut stop_receive) = mpsc::channel::<()>(10);

        let executor = CommandExecutionHandler {
            eval_environment: eval_environment.clone(),
            execution_mode,
            cmd_in_send,
            cmd_out_receive,
            stop_send,
        };

        tokio::spawn(async move {
            let first_command_in = cmd_in_receive.recv().await.unwrap();
            let mut cmd = run_cmd_isolated(&eval_environment, &first_command_in).unwrap();

            let mut child = cmd.spawn().unwrap();
            let mut stdout_reader = BufReader::new(child.stdout.take().unwrap()).lines();
            let mut stderr_reader = BufReader::new(child.stderr.take().unwrap()).lines();

            let mut cmd_out_send2 = cmd_out_send.clone();
            let mut process_handle = tokio::spawn(async move {
                child.await.unwrap();
                cmd_out_send2.send(CmdOutput::Finish).await.ok().unwrap();
            });

            loop {
                let mut cmd_out_send2 = cmd_out_send.clone();
                tokio::select! {
                    Some(new_cmd) = cmd_in_receive.recv() => {
                        let mut cmd = Command::new("bash");
                        cmd.arg("-c");
                        cmd.arg(new_cmd);
                        cmd.kill_on_drop(true).stdout(Stdio::piped()).stderr(Stdio::piped());
                        let new_child = cmd.spawn();
                        if let Some(mut new_child) = new_child.ok().take() {
                            stdout_reader = BufReader::new(new_child.stdout.take().unwrap()).lines();
                            stderr_reader = BufReader::new(new_child.stderr.take().unwrap()).lines();
                            let new_process_handle = tokio::spawn(async move {
                                new_child.await.unwrap();
                                cmd_out_send2.send(CmdOutput::Finish).await.ok().unwrap();
                            });
                            drop(std::mem::replace(&mut process_handle, new_process_handle));
                        } else {
                            cmd_out_send2.send(CmdOutput::Stderr("Error running your line".to_string())).await.ok().unwrap();
                        }
                    }
                    Ok(Some(command_stdout)) = stdout_reader.next_line() => {
                        cmd_out_send2.send(CmdOutput::Stdout(command_stdout)).await.ok().unwrap();
                    }
                    Ok(Some(command_stderr)) = stderr_reader.next_line() => {
                        cmd_out_send2.send(CmdOutput::Stderr(command_stderr)).await.ok().unwrap();
                    }
                    Some(_) = stop_receive.recv() => {
                        break;
                    }
                };
            }
        });
        executor
    }

    pub async fn execute(&mut self, cmd: &str) {
        self.cmd_in_send.send(cmd.into()).await.unwrap();
    }

    pub fn poll_output(&mut self) -> Option<CmdOutput> {
        self.cmd_out_receive.try_recv().ok()
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
        .map_err(|err| err.to_string())
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
        .map_err(|err| err.to_string())
}
