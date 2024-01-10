//! The state module handles persisting the state of a debugging session between sessions.

use std::{io::Read, io::Write, path::Path};

use eyre::Context;
use serde::{Deserialize, Serialize};

/// State that is persisted
#[derive(Default, Serialize, Deserialize, Debug)]
pub struct Persistence {
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

pub fn read_from(path: impl AsRef<Path>) -> eyre::Result<Persistence> {
    let f = std::fs::File::open(path).context("opening save state")?;
    let state = load(f).context("reading from state file")?;
    Ok(state)
}
