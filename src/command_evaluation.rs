use std::process::{Child, Command, Stdio};
use std::str;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ExecutionMode {
    UNSAFE,
    ISOLATED,
}

pub struct CommandExecutionHandler {
    pub execution_mode: ExecutionMode,
    pub eval_environment: Vec<String>,
    cmd_out_receive: Receiver<ProcessResult>,
    cmd_in_send: Sender<String>,
    stop_send: Sender<()>,
}

pub enum ProcessResult {
    Ok(String),
    NotOk(String),
}

impl CommandExecutionHandler {
    pub fn start(execution_mode: ExecutionMode, eval_environment: Vec<String>) -> CommandExecutionHandler {
        let (cmd_in_send, cmd_in_receive) = mpsc::channel::<String>();
        let (cmd_out_send, cmd_out_receive) = mpsc::channel::<ProcessResult>();
        let (stop_send, stop_receive) = mpsc::channel::<()>();

        let executor = CommandExecutionHandler {
            eval_environment: eval_environment.clone(),
            execution_mode,
            cmd_in_send,
            cmd_out_receive,
            stop_send,
        };

        thread::spawn(move || {
            let mut latest_process_handle: Option<Child> = None;

            loop {
                if let Ok(cmd) = cmd_in_receive.try_recv() {
                    let process_handle_result = match execution_mode {
                        ExecutionMode::UNSAFE => run_cmd_unsafe(&eval_environment, &cmd),
                        ExecutionMode::ISOLATED => run_cmd_isolated(&eval_environment, &cmd),
                    };

                    match process_handle_result {
                        Ok(handle) => {
                            // replace the latest_process_handle with the new one, killing any old processes
                            if let Some(mut old_handle) = latest_process_handle.replace(handle) {
                                old_handle.kill().unwrap();
                            }
                        }
                        Err(error) => cmd_out_send.send(ProcessResult::NotOk(error.into())).unwrap(), // if there's an error, show it!
                    };
                }

                // take ownership of the handle and check if the process has finished
                if let Some(mut handle) = latest_process_handle.take() {
                    if let Some(status) = handle.try_wait().unwrap() {
                        // if yes, send out it's output
                        let command_output = handle.wait_with_output().unwrap();

                        const ERROR_MSG_UNDECODABLE_OUTPUT: &str =
                            "This program tried to print something to stdout which could not be decoded as utf8. Sorry.";

                        let result = if status.success() {
                            let stdout = str::from_utf8(&command_output.stdout).unwrap_or(ERROR_MSG_UNDECODABLE_OUTPUT);
                            ProcessResult::Ok(stdout.to_owned())
                        } else {
                            let stderr = str::from_utf8(&command_output.stderr).unwrap_or(ERROR_MSG_UNDECODABLE_OUTPUT);
                            ProcessResult::NotOk(stderr.to_owned())
                        };

                        cmd_out_send.send(result).unwrap();
                    } else {
                        // otherwise give back the process_handle to the latest_process_handle option.
                        // this code effectively moves the handle out of the option _conditionally_, depending on the try_wait()
                        latest_process_handle = Some(handle);
                    }
                }

                if let Ok(()) = stop_receive.try_recv() {
                    break;
                }
            }
        });
        executor
    }

    pub fn execute(&self, cmd: &str) {
        self.cmd_in_send.send(cmd.into()).unwrap();
    }

    pub fn poll_output(&self) -> Option<ProcessResult> {
        self.cmd_out_receive.try_recv().ok()
    }

    pub fn stop(&self) {
        self.stop_send.send(()).unwrap();
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
        .spawn()
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
        .spawn()
        .map_err(|err| err.to_string())
}
