use std::process::Command;

pub trait ExecutionEnvironment {
    /// this function should be called with the actual command line pipe to execute
    /// it will return a tuple of (stdout, stderr), where stderr will be None if empty
    fn execute(&self, cmd: &str) -> (String, Option<String>);
    fn is_isolated(&self) -> bool;
}

pub struct IsolatedEnvironment {}

impl Default for IsolatedEnvironment {
    fn default() -> Self { IsolatedEnvironment {} }
}

impl ExecutionEnvironment for IsolatedEnvironment {
    fn is_isolated(&self) -> bool { true }
    fn execute(&self, cmd: &str) -> (String, Option<String>) {
        if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("-i") || cmd.contains("dd ") {
            return ("".into(), Some("Will not evaluate this command.".into()));
        }
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
        (stdout, if stderr.is_empty() { None } else { Some(stderr) })
    }
}

pub struct UnsafeEnvironment();

impl Default for UnsafeEnvironment {
    fn default() -> Self { UnsafeEnvironment() }
}
impl ExecutionEnvironment for UnsafeEnvironment {
    fn is_isolated(&self) -> bool { false }
    fn execute(&self, cmd: &str) -> (String, Option<String>) {
        if cmd.contains("rm ") || cmd.contains("mv ") || cmd.contains("-i") || cmd.contains("dd ") {
            return ("".into(), Some("Will not evaluate this command.".into()));
        }

        let output = Command::new("bash")
            .arg("-c")
            .arg(cmd)
            .output()
            .expect("failed to execute process");
        let stdout = std::str::from_utf8(&output.stdout).unwrap().to_owned();
        let stderr = std::str::from_utf8(&output.stderr).unwrap().to_owned();
        (stdout, if stderr.is_empty() { None } else { Some(stderr) })
    }
}
