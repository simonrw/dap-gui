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
