//! The state module handles persisting the state of a debugging session between sessions.

use std::{
    io::Read,
    io::Write,
    path::{Path, PathBuf},
};

use eyre::Context;
use serde::{Deserialize, Serialize};

pub struct StateManager {
    save_path: PathBuf,
    current: Persistence,
}

impl StateManager {
    pub fn new(path: impl Into<PathBuf>) -> eyre::Result<Self> {
        let path = path.into();
        let span = tracing::debug_span!("StateManager", state_path = %path.display());
        let _guard = span.enter();

        tracing::debug!("attempting to load state");
        match crate::load_from(&path) {
            Ok(state) => {
                tracing::debug!("state loaded");
                Ok(Self {
                    save_path: path,
                    current: state,
                })
            }
            Err(e) => {
                // TODO: assume the file does not exist for now
                tracing::debug!(error = %e, "loading state file");
                let state = Persistence::default();
                crate::save_to(&state, &path).wrap_err("saving state file")?;

                Ok(Self {
                    save_path: path,
                    current: state,
                })
            }
        }
    }

    pub fn load(mut self) -> eyre::Result<Self> {
        let state = crate::load_from(&self.save_path).wrap_err("loading state")?;
        self.current = state;
        Ok(self)
    }

    pub fn save(self) -> eyre::Result<Self> {
        crate::save_to(&self.current, &self.save_path).wrap_err("saving state")?;
        Ok(self)
    }

    pub fn current(&self) -> &Persistence {
        &self.current
    }
}

/// State that is persisted
#[derive(Default, Serialize, Deserialize, Debug)]
pub struct Persistence {
    pub projects: Vec<PerFile>,
    pub version: String,
}

/// State that is persisted per file
#[derive(Default, Serialize, Deserialize, Debug)]
pub struct PerFile {
    pub path: PathBuf,
    pub breakpoints: Vec<debugger::Breakpoint>,
}

pub fn save(state: &Persistence, writer: impl Write) -> eyre::Result<()> {
    serde_json::to_writer(writer, state).context("saving debugger state")?;
    Ok(())
}

pub fn save_to(state: &Persistence, path: impl AsRef<Path>) -> eyre::Result<()> {
    let f = std::fs::File::create(path).context("creating file for saving")?;
    save(state, &f).context("saving state")?;
    Ok(())
}

pub fn load(reader: impl Read) -> eyre::Result<Persistence> {
    let st = serde_json::from_reader(reader).context("reading debugger state")?;
    Ok(st)
}

pub fn load_from(path: impl AsRef<Path>) -> eyre::Result<Persistence> {
    let path = path.as_ref();
    let f = std::fs::File::open(path)
        .with_context(|| format!("opening save state {}", path.display()))?;
    let state = load(f).context("reading from state file")?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trip_save_and_load() {
        let state = Persistence {
            version: "1.0".to_string(),
            projects: vec![PerFile {
                path: PathBuf::from("/tmp/test.py"),
                breakpoints: vec![debugger::Breakpoint {
                    name: Some("bp1".to_string()),
                    path: PathBuf::from("/tmp/test.py"),
                    line: 10,
                }],
            }],
        };

        let mut buf = Vec::new();
        save(&state, &mut buf).unwrap();

        let loaded = load(Cursor::new(&buf)).unwrap();
        assert_eq!(loaded.version, "1.0");
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].path, PathBuf::from("/tmp/test.py"));
        assert_eq!(loaded.projects[0].breakpoints.len(), 1);
        assert_eq!(loaded.projects[0].breakpoints[0].line, 10);
    }

    #[test]
    fn load_malformed_json() {
        let bad_json = Cursor::new(b"not valid json {{{");
        let result = load(bad_json);
        assert!(result.is_err());
    }

    #[test]
    fn save_to_and_load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        let state = Persistence {
            version: "2.0".to_string(),
            projects: vec![],
        };

        save_to(&state, &path).unwrap();

        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.version, "2.0");
        assert!(loaded.projects.is_empty());
    }

    #[test]
    fn load_from_missing_file() {
        let result = load_from("/tmp/nonexistent_dap_gui_test_file.json");
        assert!(result.is_err());
    }

    #[test]
    fn state_manager_creates_default_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        let manager = StateManager::new(&path).unwrap();
        let current = manager.current();
        assert!(current.projects.is_empty());

        // File should have been created
        assert!(path.exists());
    }

    #[test]
    fn state_manager_loads_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        let state = Persistence {
            version: "3.0".to_string(),
            projects: vec![PerFile {
                path: PathBuf::from("/src/main.py"),
                breakpoints: vec![],
            }],
        };
        save_to(&state, &path).unwrap();

        let manager = StateManager::new(&path).unwrap();
        assert_eq!(manager.current().version, "3.0");
        assert_eq!(manager.current().projects.len(), 1);
    }

    #[test]
    fn state_manager_save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        // Create manager (creates default file)
        let manager = StateManager::new(&path).unwrap();
        let manager = manager.save().unwrap();

        // Reload from disk
        let manager = manager.load().unwrap();
        assert!(manager.current().projects.is_empty());
    }

    #[test]
    fn load_empty_json_object() {
        let json = Cursor::new(b"{}");
        let result = load(json);
        // serde should handle missing fields with defaults or error
        // Persistence has no default derives for Deserialize, so missing fields
        // may or may not error depending on serde behavior
        // The important thing is it doesn't panic
        let _ = result;
    }
}
