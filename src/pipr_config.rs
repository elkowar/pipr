use std::env;
use std::fs::{DirBuilder, File};
use std::path::Path;
pub const CONFIG_PATH_RELATIVE_TO_HOME: &'static str = ".config/pipr/pipr.toml";

#[derive(Debug, Clone)]
pub struct PiprConfig {
    pub finish_hook: Option<String>,
}

impl PiprConfig {
    pub fn load_from_file() -> PiprConfig {
        let home_path = env::var("HOME").unwrap();
        let config_path = Path::new(&home_path).join(CONFIG_PATH_RELATIVE_TO_HOME);
        DirBuilder::new()
            .recursive(true)
            .create(&config_path.parent().unwrap())
            .unwrap();
        if !config_path.exists() {
            File::create(&config_path).unwrap();
        }

        let mut settings = config::Config::default();
        let config_file = config::File::new(config_path.to_str().unwrap(), config::FileFormat::Toml);
        settings.merge(config_file).unwrap();
        PiprConfig {
            finish_hook: settings.get::<String>("finish_hook").ok(),
        }
    }
}
