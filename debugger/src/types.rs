use std::path::PathBuf;

pub type BreakpointId = u64;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Breakpoint {
    pub name: Option<String>,
    pub path: PathBuf,
    pub line: usize,
}

pub(crate) use transport::types::StackFrame;
