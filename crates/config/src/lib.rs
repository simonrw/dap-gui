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

#[cfg(test)]
mod tests {
    use super::*;
    use keybindings::{KeyBinding, KeyName};
    use std::io::Write;

    #[test]
    fn missing_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.toml");
        let config = load_config_from(&path);
        assert_eq!(
            config.keybindings.continue_start,
            keybindings::KeybindingConfig::default().continue_start
        );
    }

    #[test]
    fn empty_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "").unwrap();
        let config = load_config_from(&path);
        assert_eq!(
            config.keybindings.continue_start,
            keybindings::KeybindingConfig::default().continue_start
        );
    }

    #[test]
    fn invalid_toml_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is not valid toml {{{{").unwrap();
        let config = load_config_from(&path);
        assert_eq!(
            config.keybindings.continue_start,
            keybindings::KeybindingConfig::default().continue_start
        );
    }

    #[test]
    fn invalid_keybinding_value_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[keybindings]
step_over = "NotARealKey"
"#,
        )
        .unwrap();
        // Invalid key in a binding falls back to full defaults
        let config = load_config_from(&path);
        assert_eq!(
            config.keybindings.continue_start,
            keybindings::KeybindingConfig::default().continue_start
        );
    }

    #[test]
    fn wrong_type_for_keybinding_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[keybindings]
step_over = 42
"#,
        )
        .unwrap();
        let config = load_config_from(&path);
        assert_eq!(
            config.keybindings.step_over,
            keybindings::KeybindingConfig::default().step_over
        );
    }

    #[test]
    fn partial_config_preserves_unset_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[keybindings]
step_over = "F10"
step_into = "ctrl+F11"
"#,
        )
        .unwrap();
        let config = load_config_from(&path);
        // Overridden values
        assert_eq!(
            config.keybindings.step_over,
            KeyBinding::new(KeyName::F10, false, false, false)
        );
        assert_eq!(
            config.keybindings.step_into,
            KeyBinding::new(KeyName::F11, false, true, false)
        );
        // Unset values use defaults
        assert_eq!(
            config.keybindings.continue_start,
            keybindings::KeybindingConfig::default().continue_start
        );
        assert_eq!(
            config.keybindings.stop,
            keybindings::KeybindingConfig::default().stop
        );
        assert_eq!(
            config.keybindings.restart,
            keybindings::KeybindingConfig::default().restart
        );
        assert_eq!(
            config.keybindings.step_out,
            keybindings::KeybindingConfig::default().step_out
        );
    }

    #[test]
    fn unknown_sections_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[unknown_section]
foo = "bar"

[keybindings]
step_over = "F10"
"#,
        )
        .unwrap();
        // Unknown sections should not prevent parsing. Config uses deny_unknown_fields
        // only if specified — our default serde behavior ignores unknown fields.
        let config = load_config_from(&path);
        assert_eq!(
            config.keybindings.step_over,
            KeyBinding::new(KeyName::F10, false, false, false)
        );
    }

    #[test]
    fn directory_as_config_path_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        // Pass a directory, not a file
        let config = load_config_from(dir.path());
        assert_eq!(
            config.keybindings.continue_start,
            keybindings::KeybindingConfig::default().continue_start
        );
    }

    #[test]
    fn unreadable_file_returns_defaults() {
        // Use a path that definitely doesn't exist (nested nonexistent dirs)
        let path = PathBuf::from("/nonexistent/deeply/nested/config.toml");
        let config = load_config_from(&path);
        assert_eq!(
            config.keybindings.continue_start,
            keybindings::KeybindingConfig::default().continue_start
        );
    }

    #[test]
    fn valid_config_roundtrip_through_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let original = Config::default();
        let toml_str = toml::to_string_pretty(&original).unwrap();
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(toml_str.as_bytes()).unwrap();

        let loaded = load_config_from(&path);
        assert_eq!(
            original.keybindings.continue_start,
            loaded.keybindings.continue_start
        );
        assert_eq!(original.keybindings.step_out, loaded.keybindings.step_out);
    }

    #[test]
    fn duplicate_modifier_is_accepted() {
        // "ctrl+ctrl+F9" - duplicate modifier, should still parse
        let kb: KeyBinding = "ctrl+ctrl+F9".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::F9, false, true, false));
    }

    #[test]
    fn all_three_modifiers() {
        let kb: KeyBinding = "ctrl+alt+shift+F1".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::F1, true, true, true));
    }
}
