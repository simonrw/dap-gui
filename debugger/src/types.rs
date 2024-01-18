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
impl Breakpoint {
    pub fn normalise_paths(&mut self) {
        if self.path.starts_with("~") {
            let stub: String = self.path.display().to_string().chars().skip(2).collect();
            self.path = dirs::home_dir().unwrap().join(stub);
        }
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
        let mut b = Breakpoint {
            name: None,
            path: PathBuf::from("~/test"),
            line: 0,
        };

        b.normalise_paths();

        // TODO: only applicable to one system
        assert_eq!(b.path, PathBuf::from("/Users/simon/test"));
    }
}
