use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

pub type BreakpointId = u64;

// Serialize/Deserialize are required for persisting
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Breakpoint {
    pub name: Option<String>,
    pub path: PathBuf,
    pub line: usize,
}

impl Breakpoint {
    pub fn normalised_path(&self) -> Cow<'_, Path> {
        crate::utils::normalise_path(&self.path)
    }
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Breakpoint;

    #[test]
    fn test_normalisation() {
        let b = Breakpoint {
            name: None,
            path: PathBuf::from("~/test"),
            line: 0,
        };

        let path = b.normalised_path();

        // TODO: only applicable to one system
        assert_eq!(path, PathBuf::from("/Users/simon/test"));
    }
}
