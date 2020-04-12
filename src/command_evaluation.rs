use std::process::{Child, Command, Stdio};
use std::str;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ExecutionMode {
    UNSAFE,
    ISOLATED(Vec<(String, String)>),
}

pub struct Executor {
    pub execution_mode: ExecutionMode,
    cmd_out_receive: Receiver<ProcessResult>,
    cmd_in_send: Sender<String>,
    stop_send: Sender<()>,
}

pub enum ProcessResult {
    Ok(String),
    NotOk(String),
}

impl Executor {
    pub fn start_executor(execution_mode: ExecutionMode) -> Executor {
        let (cmd_in_send, cmd_in_receive) = mpsc::channel::<String>();
        let (cmd_out_send, cmd_out_receive) = mpsc::channel::<ProcessResult>();
        let (stop_send, stop_receive) = mpsc::channel::<()>();

        let executor = Executor {
            execution_mode: execution_mode.clone(),
            cmd_in_send,
            cmd_out_receive,
            stop_send,
        };

        thread::spawn(move || {
            let mut latest_process_handle: Option<Child> = None;

            loop {
                if let Ok(command) = cmd_in_receive.try_recv() {
                    match start_command(&execution_mode, &command) {
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
                        let result = if status.success() {
                            ProcessResult::Ok(str::from_utf8(&command_output.stdout).unwrap().to_owned())
                        } else {
                            ProcessResult::NotOk(str::from_utf8(&command_output.stderr).unwrap().to_owned())
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

fn start_command(execution_mode: &ExecutionMode, cmd: &str) -> Result<Child, &'static str> {
    if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("-i") || cmd.contains("dd ") {
        return Err("Will not run this command, it's for your own good. Believe me.");
    }
    match execution_mode {
        ExecutionMode::UNSAFE => Ok(run_cmd_unsafe(cmd)),
        ExecutionMode::ISOLATED(mounts) => Ok(run_cmd_isolated(mounts, cmd)),
    }
}

fn run_cmd_isolated(mounts: &Vec<(String, String)>, cmd: &str) -> Child {
    let args = "--ro-bind ./ /working_directory --chdir /working_directory \
                --tmpfs /tmp --proc /proc --dev /dev --die-with-parent --share-net --unshare-pid";
    let mut command = Command::new("bwrap");
    for arg in args.split(" ") {
        command.arg(arg);
    }
    for mount in mounts {
        command.arg("--ro-bind").arg(&mount.0).arg(&mount.1);
    }
    command
        .arg("bash")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute process in bwrap. this might be a bwrap problem,... or not")
}

fn run_cmd_unsafe(cmd: &str) -> Child {
    Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute process")
}
