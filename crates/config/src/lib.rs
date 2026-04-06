pub mod keybindings;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub keybindings: keybindings::KeybindingConfig,
}

/// Load configuration from the user's config directory.
///
/// Reads `$XDG_CONFIG_HOME/dapgui/config.toml` (or platform equivalent).
/// Returns defaults if the file is missing or unparseable.
pub fn load_config() -> Config {
    load_config_from(&config_path())
}

/// Load configuration from a specific path.
///
/// Returns defaults if the file is missing or unparseable.
pub fn load_config_from(path: &std::path::Path) -> Config {
    match std::fs::read_to_string(path) {
        Ok(contents) => match toml::from_str(&contents) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    path = %path.display(),
                    "invalid config file, using defaults"
                );
                Config::default()
            }
        },
        Err(_) => {
            tracing::debug!(path = %path.display(), "no config file found, using defaults");
            Config::default()
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("dapgui")
        .join("config.toml")
}
