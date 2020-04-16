use std::collections::HashMap;
use std::fs::{DirBuilder, File};
use std::io::prelude::*;
use std::path::PathBuf;

use super::snippets::*;
use crate::command_template::CommandTemplate;

pub const DEFAULT_CONFIG: &str = "
#  ____  _
# |  _ \\(_)_ __  _ __      .__________.
# | |_) | | '_ \\| '__|     |__________|
# |  __/| | |_) | |          |_    -|
# |_|   |_| .__/|_|     xX  xXx___x_|
#         |_|
#
# A commandline utility by
# Leon Kowarschick

# finish_hook: Executed once you close pipr, getting the command you constructed piped into stdin.
# finish_hook = \"xclip -selection clipboard -in\"

# Paranoid history mode writes every sucessfully running command into the history in autoeval mode.
paranoid_history_mode_default = false

autoeval_mode_default = false

history_size = 500
cmdlist_always_show_preview = false

eval_environment = [\"bash\", \"-c\"]

# directories mounted into the isolated environment.
# Syntax: '<on_host>:<in_isolated>'
isolation_mounts_readonly = ['/lib:/lib', '/usr:/usr', '/lib64:/lib64', '/bin:/bin', '/etc:/etc']

# Add paths info the isolated environments $PATH.
# for example, you might mount your '~/.local/bin' to '/local_bin', 
# and then add '/local_bin' to the isolation_path_additions.
isolation_path_additions = []

# Snippets can be used to quickly insert common bits of shell
# use || (two pipes) where you want your cursor to be after insertion

[snippets]
s = \" | sed -r 's/||//g'\"

[help_viewers]
'm' = \"man ??\"
'h' = \"?? --help | less\"
";

#[derive(Debug, Clone)]
pub struct PiprConfig {
    pub finish_hook: Option<String>,
    pub isolation_mounts_readonly: Vec<(String, String)>,
    pub isolation_path_additions: Vec<String>,
    pub cmdlist_always_show_preview: bool,
    pub paranoid_history_mode_default: bool,
    pub eval_environment: Vec<String>,
    pub autoeval_mode_default: bool,
    pub history_size: usize,
    pub snippets: HashMap<char, Snippet>,
    pub help_viewers: HashMap<char, CommandTemplate>,
}

impl PiprConfig {
    pub fn load_from_file(path: &PathBuf) -> PiprConfig {
        DirBuilder::new().recursive(true).create(&path.parent().unwrap()).unwrap();
        if !path.exists() {
            create_default_file(&path);
        }
        let mut settings = config::Config::default();
        let config_file = config::File::new(path.to_str().unwrap(), config::FileFormat::Toml);
        settings.merge(config_file).unwrap();
        PiprConfig::from_settings(&settings)
    }

    fn from_settings(settings: &config::Config) -> PiprConfig {
        let snippets = settings
            .get::<HashMap<String, String>>("snippets")
            .unwrap_or_default()
            .iter()
            .map(|(k, v)| (k.chars().nth(0).unwrap(), Snippet::parse(v)))
            .collect();

        let isolation_mounts_readonly =
            parse_isolation_mounts(&settings.get::<Vec<String>>("isolation_mounts_readonly").unwrap_or(vec![
                "/lib:/lib".into(),
                "/usr:/usr".into(),
                "/lib64:/lib64".into(),
                "/bin:/bin".into(),
                "/etc:/etc".into(),
            ]));

        let help_viewers = settings
            .get::<HashMap<char, String>>("help_viewers")
            .unwrap_or(hashmap! {
                'm' => "man ??".into(),
                'h' => "?? --help | less".into(),
            })
            .into_iter()
            .map(|(k, v)| (k, CommandTemplate::from_string(v).unwrap()))
            .collect::<HashMap<_, _>>();

        PiprConfig {
            finish_hook: settings.get::<String>("finish_hook").ok(),
            isolation_path_additions: settings.get::<Vec<String>>("isolation_path_additions").unwrap_or(Vec::new()),
            paranoid_history_mode_default: settings.get::<bool>("paranoid_history_mode_default").unwrap_or(false),
            autoeval_mode_default: settings.get::<bool>("autoeval_mode_default").unwrap_or(false),
            eval_environment: settings
                .get::<Vec<String>>("eval_environment")
                .unwrap_or(vec!["bash".into(), "-c".into()]),
            history_size: settings.get::<usize>("history_size").unwrap_or(500),
            cmdlist_always_show_preview: settings.get::<bool>("cmdlist_always_show_preview").unwrap_or(false),
            help_viewers,
            snippets,
            isolation_mounts_readonly,
        }
    }
}

fn parse_isolation_mounts(entries: &Vec<String>) -> Vec<(String, String)> {
    let parse_error_msg = "Invalid format in mount configuration. Format: '<on-host>:<in-isolated>'";
    entries
        .iter()
        .map(|entry| entry.split(':').collect::<Vec<&str>>())
        .map(|vec| {
            (
                vec.get(0).expect(parse_error_msg).to_string(),
                vec.get(1).expect(parse_error_msg).to_string(),
            )
        })
        .collect::<Vec<(String, String)>>()
}

fn create_default_file(path: &PathBuf) {
    let mut file = File::create(path).unwrap();
    file.write_all(DEFAULT_CONFIG.as_bytes()).unwrap();
}
