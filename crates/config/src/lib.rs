pub mod keybindings;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// User preference for the color theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    /// Detect automatically from the system setting and follow changes.
    #[default]
    Auto,
    /// Always use the dark palette.
    Dark,
    /// Always use the light palette.
    Light,
}

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub keybindings: keybindings::KeybindingConfig,
    #[serde(default)]
    pub theme: ThemePreference,
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
    let mut config: Config = match std::fs::read_to_string(path) {
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
    };

    // Build lookup maps (skipped by serde, already populated for Default)
    config.keybindings.build_lookup();

    for conflict in config.keybindings.validate() {
        tracing::warn!("{conflict}");
    }

    config
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

    #[test]
    fn default_config_has_no_conflicts() {
        let config = keybindings::KeybindingConfig::default();
        assert!(config.validate().is_empty());
    }

    #[test]
    fn conflicting_bindings_detected() {
        let mut config = keybindings::KeybindingConfig::default();
        // Set step_over to the same binding as continue_start (F9)
        config.step_over = config.continue_start.clone();
        let conflicts = config.validate();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            conflicts[0].action_a,
            keybindings::DebugAction::ContinueOrStart
        );
        assert_eq!(conflicts[0].action_b, keybindings::DebugAction::StepOver);
    }

    #[test]
    fn multiple_conflicts_detected() {
        let mut config = keybindings::KeybindingConfig::default();
        // Make everything F1
        let f1 = KeyBinding::new(KeyName::F1, false, false, false);
        config.continue_start = f1.clone();
        config.stop = f1.clone();
        config.restart = f1.clone();
        config.step_over = f1.clone();
        config.step_into = f1.clone();
        config.step_out = f1;
        let conflicts = config.validate();
        // 6 actions all on the same key = C(6,2) = 15 pairwise conflicts
        assert_eq!(conflicts.len(), 15);
    }

    #[test]
    fn conflicting_config_file_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[keybindings]
continue_start = "F9"
stop = "F9"
"#,
        )
        .unwrap();
        // Should still load (conflicts are warnings, not errors)
        let config = load_config_from(&path);
        assert_eq!(config.keybindings.continue_start, config.keybindings.stop);
    }

    #[test]
    fn conflict_display_message() {
        let conflict = keybindings::KeyConflict {
            binding: KeyBinding::new(KeyName::F9, false, false, false),
            action_a: keybindings::DebugAction::ContinueOrStart,
            action_b: keybindings::DebugAction::Stop,
        };
        let msg = conflict.to_string();
        assert!(msg.contains("F9"));
        assert!(msg.contains("continue_start"));
        assert!(msg.contains("stop"));
    }
}
