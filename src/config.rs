use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub files: Option<FilesConfig>,
}

#[derive(Debug, Deserialize)]
pub struct FilesConfig {
    pub glob_filter: Option<Vec<String>>,
}

pub fn find_and_load_config() -> Option<Config> {
    let current_dir = std::env::current_dir().ok()?;

    for dir in current_dir.ancestors() {
        for name in &["ised.config.toml", ".ised.config.toml"] {
            let candidate = dir.join(name);
            if candidate.exists() {
                let content = fs::read_to_string(&candidate).ok()?;
                return toml::from_str(&content).ok();
            }
        }
    }

    None
}
