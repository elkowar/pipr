use std::process::{Child, Command, Stdio};
use std::str;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ExecutionMode {
    UNSAFE,
    ISOLATED {
        additional_mounts: Vec<(String, String)>,
        additional_path_entries: Vec<String>,
    },
}

pub struct Executor {
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

impl Executor {
    pub fn start_executor(execution_mode: ExecutionMode, eval_environment: Vec<String>) -> Executor {
        let (cmd_in_send, cmd_in_receive) = mpsc::channel::<String>();
        let (cmd_out_send, cmd_out_receive) = mpsc::channel::<ProcessResult>();
        let (stop_send, stop_receive) = mpsc::channel::<()>();

        let executor = Executor {
            execution_mode: execution_mode.clone(),
            eval_environment: eval_environment.clone(),
            cmd_in_send,
            cmd_out_receive,
            stop_send,
        };

        thread::spawn(move || {
            let mut latest_process_handle: Option<Child> = None;

            loop {
                if let Ok(command) = cmd_in_receive.try_recv() {
                    match start_command(&execution_mode, &eval_environment, &command) {
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

fn start_command(execution_mode: &ExecutionMode, eval_environment: &Vec<String>, cmd: &str) -> Result<Child, &'static str> {
    match execution_mode {
        ExecutionMode::UNSAFE => run_cmd_unsafe(eval_environment, cmd),
        ExecutionMode::ISOLATED {
            additional_mounts,
            additional_path_entries,
        } => Ok(run_cmd_isolated(
            additional_mounts,
            additional_path_entries,
            eval_environment,
            cmd,
        )),
    }
}

fn run_cmd_isolated(
    mounts: &Vec<(String, String)>,
    additional_path_entries: &Vec<String>,
    eval_environment: &Vec<String>,
    cmd: &str,
) -> Child {
    let args = "--ro-bind ./ /working_directory --chdir /working_directory \
                --tmpfs /tmp --proc /proc --dev /dev --die-with-parent --share-net --unshare-pid";
    let mut command = Command::new("bwrap");
    for arg in args.split(" ") {
        command.arg(arg);
    }
    for (on_host, in_isolated) in mounts {
        command.arg("--ro-bind").arg(&on_host).arg(&in_isolated);
    }

    if !additional_path_entries.is_empty() {
        let mut path_variable = std::env::var("PATH").unwrap();
        for path_entry in additional_path_entries {
            path_variable.push_str(&format!(":{}", path_entry));
        }
        command.env("PATH", path_variable);
    }

    let mut eval_environment = eval_environment.into_iter();

    command.arg(eval_environment.next().expect("eval_environment was empty"));
    for arg in eval_environment {
        command.arg(arg);
    }
    command
        .arg(cmd)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute process in bwrap. this might be a bwrap problem,... or not")
}

fn run_cmd_unsafe(eval_environment: &Vec<String>, cmd: &str) -> Result<Child, &'static str> {
    if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("dd ") {
        return Err("Will not run this command, it's for your own good. Believe me.");
    }
    let mut eval_environment = eval_environment.into_iter();
    let mut command = Command::new(eval_environment.next().expect("eval_environment was empty"));
    for arg in eval_environment {
        command.arg(arg);
    }
    let child = command
        .arg(cmd)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute process");
    Ok(child)
}
