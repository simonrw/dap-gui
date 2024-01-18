//! The state module handles persisting the state of a debugging session between sessions.

use std::{
    collections::HashMap,
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
        let mut state = crate::load_from(&path).wrap_err("loading state")?;
        state.normalise_paths();
        Ok(Self {
            save_path: path,
            current: state,
        })
    }
    pub fn load(mut self) -> eyre::Result<Self> {
        let mut state = crate::load_from(&self.save_path).wrap_err("loading state")?;
        state.normalise_paths();
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
    pub projects: HashMap<String, PerFile>,
    pub version: String,
}

impl Persistence {
    /// Convert instanfces of "~" to the users home directory
    fn normalise_paths(&mut self) {
        for project in self.projects.values_mut() {
            project.normalise_paths();
        }
    }
}

/// State that is persisted per file
#[derive(Default, Serialize, Deserialize, Debug)]
pub struct PerFile {
    pub breakpoints: Vec<debugger::Breakpoint>,
}
impl PerFile {
    fn normalise_paths(&mut self) {
        for breakpoint in &mut self.breakpoints {
            breakpoint.normalise_paths();
        }
    }
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
    let f = std::fs::File::open(path).context("opening save state")?;
    let state = load(f).context("reading from state file")?;
    Ok(state)
}
