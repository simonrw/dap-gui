use eyre::Context;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    str::FromStr,
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

impl FromStr for Breakpoint {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (path_str, lineno_str) = s
            .split_once(':')
            .ok_or_else(|| eyre::eyre!("breakpoint specification '{s}' has no colon"))?;

        let lineno = lineno_str.parse().wrap_err("invalid line number")?;
        Ok(Self {
            name: None,
            path: PathBuf::from(path_str),
            line: lineno,
        })
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
    use std::{path::PathBuf, str::FromStr};

    use super::Breakpoint;

    macro_rules! assert_res_eq {
        ($a:expr, $b:expr) => {{
            match ($a, $b) {
                (Ok(_), Ok(_)) => {}
                (Err(e1), Err(e2)) => {
                    let s1 = format!("{e1}");
                    let s2 = format!("{e2}");
                    assert_eq!(s1, s2);
                }
                (Err(_), _) => panic!("not equal"),
                (_, Err(_)) => panic!("not equal"),
            }
        }};
    }

    #[test]
    fn test_normalisation() {
        let b = Breakpoint {
            name: None,
            path: PathBuf::from("~/test"),
            line: 0,
        };

        let path = b.normalised_path();

        let home_dir = dirs::home_dir().unwrap();
        assert_eq!(path, home_dir.join("test"));
    }

    #[test]
    fn breakpoint_from_str() {
        let cases = vec![
            (
                "",
                Err(eyre::eyre!("breakpoint specification '' has no colon")),
            ),
            (
                "test",
                Err(eyre::eyre!("breakpoint specification 'test' has no colon")),
            ),
            ("test.py:foo", Err(eyre::eyre!("invalid line number"))),
            (
                "test.py:16",
                Ok(Breakpoint {
                    path: PathBuf::from("test.py"),
                    line: 16,
                    name: None,
                }),
            ),
        ];
        for (input, expected) in cases {
            assert_res_eq!(Breakpoint::from_str(input), expected);
        }
    }
}
