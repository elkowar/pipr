use tokio::process::Command;
pub const COMMAND_TEMPLATE_PLACEHOLDER_TOKEN: &str = "??";

#[derive(Debug, PartialEq, Clone)]
pub struct CommandTemplate(String);

impl CommandTemplate {
    pub fn from_string(s: String) -> Result<CommandTemplate, &'static str> {
        if s.is_empty() {
            Err("attempted to parse empty CommandTemplate")
        } else {
            Ok(CommandTemplate(s))
        }
    }

    /// insert the value into the template and return the resolved string
    pub fn resolve(&self, placeholder_value: &str) -> String {
        self.0.replace(COMMAND_TEMPLATE_PLACEHOLDER_TOKEN, placeholder_value)
    }

    /// generates a Command that executes the given command in `bash -c`
    pub fn resolve_to_command(&self, placeholder_value: &str) -> Command {
        let mut command = Command::new("bash");
        command.arg("-c").arg(self.resolve(placeholder_value));
        command
    }
}
