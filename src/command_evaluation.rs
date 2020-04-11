use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum ExecutionMode {
    UNSAFE,
    ISOLATED,
}

pub struct Executor {
    pub execution_mode: ExecutionMode,
    cmd_out_receive: Receiver<(String, String)>,
    cmd_in_send: Sender<String>,
    stop_send: Sender<()>,
}

impl Executor {
    pub fn start_executor(execution_mode: ExecutionMode) -> Executor {
        let (cmd_in_send, cmd_in_receive) = mpsc::channel::<String>();
        let (cmd_out_send, cmd_out_receive) = mpsc::channel::<(String, String)>();
        let (stop_send, stop_receive) = mpsc::channel::<()>();

        let executor = Executor {
            execution_mode,
            cmd_in_send,
            cmd_out_receive,
            stop_send,
        };

        thread::spawn(move || loop {
            if let Ok(command) = cmd_in_receive.try_recv() {
                cmd_out_send.send(execute_blocking(&execution_mode, &command)).unwrap();
            }

            if let Ok(()) = stop_receive.try_recv() {
                break;
            }
        });
        executor
    }

    pub fn execute(&self, cmd: &str) {
        self.cmd_in_send.send(cmd.into()).unwrap();
    }

    pub fn poll_output(&self) -> Option<(String, String)> {
        self.cmd_out_receive.try_recv().ok()
    }

    pub fn stop(&self) {
        self.stop_send.send(()).unwrap();
    }
}

fn execute_blocking(execution_mode: &ExecutionMode, cmd: &str) -> (String, String) {
    if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("-i") || cmd.contains("dd ") {
        return ("".into(), "Will not evaluate this command.".into());
    }
    match execution_mode {
        ExecutionMode::UNSAFE => run_cmd_unsafe(cmd),
        ExecutionMode::ISOLATED => run_cmd_isolated(cmd),
    }
}

fn run_cmd_isolated(cmd: &str) -> (String, String) {
    let args = "--ro-bind ./ /working_directory --chdir /working_directory \
                    --ro-bind /lib /lib --ro-bind /usr /usr --ro-bind /lib64 /lib64 --ro-bind /bin /bin \
                    --tmpfs /tmp --proc /proc --dev /dev --ro-bind /etc /etc --die-with-parent --share-net --unshare-pid";
    let mut command = Command::new("bwrap");
    for arg in args.split(" ") {
        command.arg(arg);
    }
    let output = command
        .arg("bash")
        .arg("-c")
        .arg(cmd)
        .output()
        .expect("Failed to execute process in bwrap. this might be a bwrap problem,... or not");

    let stdout = std::str::from_utf8(&output.stdout).unwrap().to_owned();
    let stderr = std::str::from_utf8(&output.stderr).unwrap().to_owned();
    (stdout, stderr)
}

fn run_cmd_unsafe(cmd: &str) -> (String, String) {
    let output = Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .output()
        .expect("failed to execute process");
    let stdout = std::str::from_utf8(&output.stdout).unwrap().to_owned();
    let stderr = std::str::from_utf8(&output.stderr).unwrap().to_owned();
    (stdout, stderr)
}
