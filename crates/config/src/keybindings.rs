use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A frontend-agnostic key identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyName {
    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    // Digits
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    // Navigation
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    // Editing
    Enter,
    Escape,
    Backspace,
    Delete,
    Insert,
    Tab,
    Space,
}

impl fmt::Display for KeyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyName::F1 => write!(f, "F1"),
            KeyName::F2 => write!(f, "F2"),
            KeyName::F3 => write!(f, "F3"),
            KeyName::F4 => write!(f, "F4"),
            KeyName::F5 => write!(f, "F5"),
            KeyName::F6 => write!(f, "F6"),
            KeyName::F7 => write!(f, "F7"),
            KeyName::F8 => write!(f, "F8"),
            KeyName::F9 => write!(f, "F9"),
            KeyName::F10 => write!(f, "F10"),
            KeyName::F11 => write!(f, "F11"),
            KeyName::F12 => write!(f, "F12"),
            KeyName::A => write!(f, "A"),
            KeyName::B => write!(f, "B"),
            KeyName::C => write!(f, "C"),
            KeyName::D => write!(f, "D"),
            KeyName::E => write!(f, "E"),
            KeyName::F => write!(f, "F"),
            KeyName::G => write!(f, "G"),
            KeyName::H => write!(f, "H"),
            KeyName::I => write!(f, "I"),
            KeyName::J => write!(f, "J"),
            KeyName::K => write!(f, "K"),
            KeyName::L => write!(f, "L"),
            KeyName::M => write!(f, "M"),
            KeyName::N => write!(f, "N"),
            KeyName::O => write!(f, "O"),
            KeyName::P => write!(f, "P"),
            KeyName::Q => write!(f, "Q"),
            KeyName::R => write!(f, "R"),
            KeyName::S => write!(f, "S"),
            KeyName::T => write!(f, "T"),
            KeyName::U => write!(f, "U"),
            KeyName::V => write!(f, "V"),
            KeyName::W => write!(f, "W"),
            KeyName::X => write!(f, "X"),
            KeyName::Y => write!(f, "Y"),
            KeyName::Z => write!(f, "Z"),
            KeyName::Digit0 => write!(f, "0"),
            KeyName::Digit1 => write!(f, "1"),
            KeyName::Digit2 => write!(f, "2"),
            KeyName::Digit3 => write!(f, "3"),
            KeyName::Digit4 => write!(f, "4"),
            KeyName::Digit5 => write!(f, "5"),
            KeyName::Digit6 => write!(f, "6"),
            KeyName::Digit7 => write!(f, "7"),
            KeyName::Digit8 => write!(f, "8"),
            KeyName::Digit9 => write!(f, "9"),
            KeyName::Up => write!(f, "Up"),
            KeyName::Down => write!(f, "Down"),
            KeyName::Left => write!(f, "Left"),
            KeyName::Right => write!(f, "Right"),
            KeyName::Home => write!(f, "Home"),
            KeyName::End => write!(f, "End"),
            KeyName::PageUp => write!(f, "PageUp"),
            KeyName::PageDown => write!(f, "PageDown"),
            KeyName::Enter => write!(f, "Enter"),
            KeyName::Escape => write!(f, "Escape"),
            KeyName::Backspace => write!(f, "Backspace"),
            KeyName::Delete => write!(f, "Delete"),
            KeyName::Insert => write!(f, "Insert"),
            KeyName::Tab => write!(f, "Tab"),
            KeyName::Space => write!(f, "Space"),
        }
    }
}

impl FromStr for KeyName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Try case-insensitive match for named keys first
        match s.to_ascii_uppercase().as_str() {
            "F1" => Ok(KeyName::F1),
            "F2" => Ok(KeyName::F2),
            "F3" => Ok(KeyName::F3),
            "F4" => Ok(KeyName::F4),
            "F5" => Ok(KeyName::F5),
            "F6" => Ok(KeyName::F6),
            "F7" => Ok(KeyName::F7),
            "F8" => Ok(KeyName::F8),
            "F9" => Ok(KeyName::F9),
            "F10" => Ok(KeyName::F10),
            "F11" => Ok(KeyName::F11),
            "F12" => Ok(KeyName::F12),
            "A" => Ok(KeyName::A),
            "B" => Ok(KeyName::B),
            "C" => Ok(KeyName::C),
            "D" => Ok(KeyName::D),
            "E" => Ok(KeyName::E),
            "F" => Ok(KeyName::F),
            "G" => Ok(KeyName::G),
            "H" => Ok(KeyName::H),
            "I" => Ok(KeyName::I),
            "J" => Ok(KeyName::J),
            "K" => Ok(KeyName::K),
            "L" => Ok(KeyName::L),
            "M" => Ok(KeyName::M),
            "N" => Ok(KeyName::N),
            "O" => Ok(KeyName::O),
            "P" => Ok(KeyName::P),
            "Q" => Ok(KeyName::Q),
            "R" => Ok(KeyName::R),
            "S" => Ok(KeyName::S),
            "T" => Ok(KeyName::T),
            "U" => Ok(KeyName::U),
            "V" => Ok(KeyName::V),
            "W" => Ok(KeyName::W),
            "X" => Ok(KeyName::X),
            "Y" => Ok(KeyName::Y),
            "Z" => Ok(KeyName::Z),
            "0" => Ok(KeyName::Digit0),
            "1" => Ok(KeyName::Digit1),
            "2" => Ok(KeyName::Digit2),
            "3" => Ok(KeyName::Digit3),
            "4" => Ok(KeyName::Digit4),
            "5" => Ok(KeyName::Digit5),
            "6" => Ok(KeyName::Digit6),
            "7" => Ok(KeyName::Digit7),
            "8" => Ok(KeyName::Digit8),
            "9" => Ok(KeyName::Digit9),
            "UP" => Ok(KeyName::Up),
            "DOWN" => Ok(KeyName::Down),
            "LEFT" => Ok(KeyName::Left),
            "RIGHT" => Ok(KeyName::Right),
            "HOME" => Ok(KeyName::Home),
            "END" => Ok(KeyName::End),
            "PAGEUP" => Ok(KeyName::PageUp),
            "PAGEDOWN" => Ok(KeyName::PageDown),
            "ENTER" | "RETURN" => Ok(KeyName::Enter),
            "ESCAPE" | "ESC" => Ok(KeyName::Escape),
            "BACKSPACE" => Ok(KeyName::Backspace),
            "DELETE" | "DEL" => Ok(KeyName::Delete),
            "INSERT" | "INS" => Ok(KeyName::Insert),
            "TAB" => Ok(KeyName::Tab),
            "SPACE" => Ok(KeyName::Space),
            _ => Err(format!("unknown key: {s}")),
        }
    }
}

/// A key chord: a key plus modifier flags.
///
/// Serialises to/from a human-readable string like `"shift+F7"` or `"ctrl+shift+F9"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub key: KeyName,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl KeyBinding {
    pub fn new(key: KeyName, shift: bool, ctrl: bool, alt: bool) -> Self {
        Self {
            key,
            shift,
            ctrl,
            alt,
        }
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "ctrl+")?;
        }
        if self.alt {
            write!(f, "alt+")?;
        }
        if self.shift {
            write!(f, "shift+")?;
        }
        write!(f, "{}", self.key)
    }
}

impl FromStr for KeyBinding {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut shift = false;
        let mut ctrl = false;
        let mut alt = false;

        let parts: Vec<&str> = s.split('+').collect();
        if parts.is_empty() {
            return Err("empty keybinding string".to_string());
        }

        // All parts except the last are modifiers; the last is the key name.
        for &part in &parts[..parts.len() - 1] {
            match part.to_ascii_lowercase().as_str() {
                "shift" => shift = true,
                "ctrl" => ctrl = true,
                "alt" => alt = true,
                other => return Err(format!("unknown modifier: {other}")),
            }
        }

        let key = parts[parts.len() - 1].parse::<KeyName>()?;
        Ok(KeyBinding {
            key,
            shift,
            ctrl,
            alt,
        })
    }
}

impl Serialize for KeyBinding {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for KeyBinding {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Debug actions that can be bound to keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugAction {
    ContinueOrStart,
    Stop,
    Restart,
    StepOver,
    StepInto,
    StepOut,
}

/// Keybinding configuration for debug actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingConfig {
    #[serde(default = "default_continue_start")]
    pub continue_start: KeyBinding,
    #[serde(default = "default_stop")]
    pub stop: KeyBinding,
    #[serde(default = "default_restart")]
    pub restart: KeyBinding,
    #[serde(default = "default_step_over")]
    pub step_over: KeyBinding,
    #[serde(default = "default_step_into")]
    pub step_into: KeyBinding,
    #[serde(default = "default_step_out")]
    pub step_out: KeyBinding,
}

fn default_continue_start() -> KeyBinding {
    KeyBinding::new(KeyName::F9, false, false, false)
}
fn default_stop() -> KeyBinding {
    KeyBinding::new(KeyName::F9, true, false, false)
}
fn default_restart() -> KeyBinding {
    KeyBinding::new(KeyName::F9, true, true, false)
}
fn default_step_over() -> KeyBinding {
    KeyBinding::new(KeyName::F8, false, false, false)
}
fn default_step_into() -> KeyBinding {
    KeyBinding::new(KeyName::F7, false, false, false)
}
fn default_step_out() -> KeyBinding {
    KeyBinding::new(KeyName::F7, true, false, false)
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self {
            continue_start: default_continue_start(),
            stop: default_stop(),
            restart: default_restart(),
            step_over: default_step_over(),
            step_into: default_step_into(),
            step_out: default_step_out(),
        }
    }
}

impl KeybindingConfig {
    /// Match a key event (expressed as agnostic key name + modifiers) to a debug action.
    pub fn match_action(
        &self,
        key: KeyName,
        shift: bool,
        ctrl: bool,
        alt: bool,
    ) -> Option<DebugAction> {
        let input = KeyBinding::new(key, shift, ctrl, alt);
        if input == self.continue_start {
            return Some(DebugAction::ContinueOrStart);
        }
        if input == self.stop {
            return Some(DebugAction::Stop);
        }
        if input == self.restart {
            return Some(DebugAction::Restart);
        }
        if input == self.step_over {
            return Some(DebugAction::StepOver);
        }
        if input == self.step_into {
            return Some(DebugAction::StepInto);
        }
        if input == self.step_out {
            return Some(DebugAction::StepOut);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_key() {
        let kb: KeyBinding = "F9".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::F9, false, false, false));
    }

    #[test]
    fn parse_with_shift() {
        let kb: KeyBinding = "shift+F7".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::F7, true, false, false));
    }

    #[test]
    fn parse_with_ctrl_shift() {
        let kb: KeyBinding = "ctrl+shift+F9".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::F9, true, true, false));
    }

    #[test]
    fn parse_case_insensitive() {
        let kb: KeyBinding = "Shift+f7".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::F7, true, false, false));
    }

    #[test]
    fn display_roundtrip() {
        let config = KeybindingConfig::default();
        let bindings = [
            &config.continue_start,
            &config.stop,
            &config.restart,
            &config.step_over,
            &config.step_into,
            &config.step_out,
        ];
        for binding in bindings {
            let s = binding.to_string();
            let parsed: KeyBinding = s.parse().unwrap();
            assert_eq!(&parsed, binding);
        }
    }

    #[test]
    fn toml_roundtrip() {
        let config = crate::Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: crate::Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            config.keybindings.continue_start,
            parsed.keybindings.continue_start
        );
        assert_eq!(config.keybindings.step_out, parsed.keybindings.step_out);
    }

    #[test]
    fn partial_override() {
        let toml_str = r#"
[keybindings]
step_over = "F10"
"#;
        let config: crate::Config = toml::from_str(toml_str).unwrap();
        // Overridden
        assert_eq!(
            config.keybindings.step_over,
            KeyBinding::new(KeyName::F10, false, false, false)
        );
        // Defaults preserved
        assert_eq!(config.keybindings.continue_start, default_continue_start());
        assert_eq!(config.keybindings.step_into, default_step_into());
        assert_eq!(config.keybindings.step_out, default_step_out());
    }

    #[test]
    fn match_action_found() {
        let config = KeybindingConfig::default();
        assert_eq!(
            config.match_action(KeyName::F9, false, false, false),
            Some(DebugAction::ContinueOrStart)
        );
        assert_eq!(
            config.match_action(KeyName::F7, true, false, false),
            Some(DebugAction::StepOut)
        );
        assert_eq!(
            config.match_action(KeyName::F9, true, true, false),
            Some(DebugAction::Restart)
        );
    }

    #[test]
    fn match_action_none() {
        let config = KeybindingConfig::default();
        assert_eq!(config.match_action(KeyName::F1, false, false, false), None);
        // Wrong modifiers
        assert_eq!(config.match_action(KeyName::F9, false, true, false), None);
    }

    #[test]
    fn parse_unknown_key_errors() {
        assert!("~".parse::<KeyBinding>().is_err());
        assert!("NumLock".parse::<KeyBinding>().is_err());
    }

    #[test]
    fn parse_unknown_modifier_errors() {
        assert!("super+F9".parse::<KeyBinding>().is_err());
    }

    #[test]
    fn empty_config_file_uses_defaults() {
        let config: crate::Config = toml::from_str("").unwrap();
        assert_eq!(config.keybindings.continue_start, default_continue_start());
    }

    #[test]
    fn parse_letter_keys() {
        let kb: KeyBinding = "A".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::A, false, false, false));
        // Case insensitive
        let kb: KeyBinding = "a".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::A, false, false, false));
    }

    #[test]
    fn parse_letter_with_modifiers() {
        let kb: KeyBinding = "ctrl+S".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::S, false, true, false));
        let kb: KeyBinding = "alt+shift+D".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::D, true, false, true));
    }

    #[test]
    fn parse_digit_keys() {
        let kb: KeyBinding = "0".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Digit0, false, false, false));
        let kb: KeyBinding = "9".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Digit9, false, false, false));
    }

    #[test]
    fn parse_navigation_keys() {
        let kb: KeyBinding = "Up".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Up, false, false, false));
        let kb: KeyBinding = "PageDown".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::PageDown, false, false, false));
        let kb: KeyBinding = "Home".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Home, false, false, false));
    }

    #[test]
    fn parse_special_keys() {
        let kb: KeyBinding = "Enter".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Enter, false, false, false));
        let kb: KeyBinding = "Return".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Enter, false, false, false));
        let kb: KeyBinding = "Escape".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Escape, false, false, false));
        let kb: KeyBinding = "Esc".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Escape, false, false, false));
        let kb: KeyBinding = "Space".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Space, false, false, false));
        let kb: KeyBinding = "Tab".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Tab, false, false, false));
        let kb: KeyBinding = "Delete".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Delete, false, false, false));
        let kb: KeyBinding = "Del".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Delete, false, false, false));
        let kb: KeyBinding = "Insert".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Insert, false, false, false));
        let kb: KeyBinding = "Ins".parse().unwrap();
        assert_eq!(kb, KeyBinding::new(KeyName::Insert, false, false, false));
    }

    #[test]
    fn display_roundtrip_all_key_types() {
        let keys = [
            KeyName::A,
            KeyName::Z,
            KeyName::Digit0,
            KeyName::Digit9,
            KeyName::Up,
            KeyName::PageDown,
            KeyName::Enter,
            KeyName::Escape,
            KeyName::Space,
            KeyName::Tab,
            KeyName::Delete,
            KeyName::Insert,
            KeyName::Home,
            KeyName::End,
            KeyName::Backspace,
        ];
        for key in keys {
            let binding = KeyBinding::new(key, false, false, false);
            let s = binding.to_string();
            let parsed: KeyBinding = s.parse().unwrap();
            assert_eq!(parsed, binding, "roundtrip failed for {key:?}");
        }
    }

    #[test]
    fn letter_key_binding_in_toml() {
        let toml_str = r#"
[keybindings]
step_over = "ctrl+N"
"#;
        let config: crate::Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.keybindings.step_over,
            KeyBinding::new(KeyName::N, false, true, false)
        );
    }
}
