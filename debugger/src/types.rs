use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type BreakpointId = u64;

// Serialize/Deserialize are required for persisting
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Breakpoint {
    pub name: Option<String>,
    pub path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct PausedFrame {
    pub frame: StackFrame,
    pub variables: Vec<transport::types::Variable>,
}

pub(crate) use transport::types::StackFrame;

pub struct EvaluateResult {
    pub output: String,
    pub error: bool,
}
