use std::collections::HashMap;
use std::fs::{DirBuilder, File};
use std::io::prelude::*;
use std::{path::PathBuf, time::Duration};

use super::snippets::*;
use maplit::hashmap;

use crate::command_template::CommandTemplate;

pub const DEFAULT_CONFIG: &str = "
#  ____  _
# |  _ \\(_)_ __  _ __       .________.
# | |_) | | '_ \\| '__|      |________|
# |  __/| | |_) | |          |_    -|
# |_|   |_| .__/|_|     xX  xXx___x_|
#         |_|
#
# A commandline utility by
# Leon Kowarschick

# finish_hook: Executed once you close pipr, getting the command you constructed piped into stdin.
# finish_hook = \"xclip -selection clipboard -in\"

# Paranoid history mode writes every successfully running command into the history in autoeval mode.
paranoid_history_mode_default = false

autoeval_mode_default = true

history_size = 500
cmdlist_always_show_preview = false
cmd_timeout_millis = 2000

highlighting_enabled = true

eval_environment = [\"bash\", \"-c\"]

# Snippets can be used to quickly insert common bits of shell
# use || (two pipes) where you want your cursor to be after insertion
[snippets]
s = \" | sed -r 's/||//g'\"

[help_viewers]
'm' = \"man ??\"
'h' = \"?? --help | less\"

[output_viewers]
'l' = \"less\"
";

#[derive(Debug, Clone)]
pub struct PiprConfig {
    pub finish_hook: Option<String>,
    pub cmdlist_always_show_preview: bool,
    pub paranoid_history_mode_default: bool,
    pub eval_environment: Vec<String>,
    pub autoeval_mode_default: bool,
    pub cmd_timeout: Duration,
    pub history_size: usize,
    pub snippets: HashMap<char, Snippet>,
    pub help_viewers: HashMap<char, CommandTemplate>,
    pub output_viewers: HashMap<char, String>,
    pub highlighting_enabled: bool,
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
        PiprConfig::from_settings(settings)
    }

    fn from_settings(settings: config::Config) -> PiprConfig {
        let snippets = settings
            .get::<HashMap<char, String>>("snippets")
            .unwrap_or_default()
            .iter()
            .map(|(&k, v)| (k, Snippet::parse(v)))
            .collect();

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
            paranoid_history_mode_default: settings.get::<bool>("paranoid_history_mode_default").unwrap_or(false),
            autoeval_mode_default: settings.get::<bool>("autoeval_mode_default").unwrap_or(false),
            cmd_timeout: Duration::from_millis(settings.get::<u64>("cmd_timeout_millis").unwrap_or(2000)),
            eval_environment: settings
                .get::<Vec<String>>("eval_environment")
                .unwrap_or_else(|_| vec!["bash".into(), "-c".into()]),
            history_size: settings.get::<usize>("history_size").unwrap_or(500),
            cmdlist_always_show_preview: settings.get::<bool>("cmdlist_always_show_preview").unwrap_or(false),
            highlighting_enabled: settings.get::<bool>("highlighting_enabled").unwrap_or(true),
            output_viewers: settings
                .get::<HashMap<char, String>>("output_viewers")
                .unwrap_or_else(|_| hashmap! { 'l' => "less".into() }),
            help_viewers,
            snippets,
        }
    }
}

fn create_default_file(path: &PathBuf) {
    let mut file = File::create(path).unwrap();
    file.write_all(DEFAULT_CONFIG.as_bytes()).unwrap();
}
