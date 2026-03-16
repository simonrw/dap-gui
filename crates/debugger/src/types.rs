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

    /// Parse a `file:line` breakpoint specification, resolving the path to an
    /// absolute path immediately. Relative paths are resolved against `cwd`.
    pub fn parse(input: &str, cwd: &Path) -> eyre::Result<Self> {
        let input = input.trim();
        eyre::ensure!(!input.is_empty(), "empty breakpoint specification");

        let colon_pos = input
            .rfind(':')
            .ok_or_else(|| eyre::eyre!("breakpoint specification '{input}' has no colon"))?;

        let path_str = &input[..colon_pos];
        let line_str = &input[colon_pos + 1..];

        eyre::ensure!(
            !path_str.is_empty(),
            "breakpoint specification '{input}' has no file path"
        );

        let line: usize = line_str
            .parse()
            .wrap_err_with(|| format!("invalid line number '{line_str}'"))?;

        let raw_path = PathBuf::from(path_str);
        let absolute = if raw_path.is_absolute() {
            raw_path
        } else {
            cwd.join(raw_path)
        };
        let path = std::fs::canonicalize(&absolute).unwrap_or(absolute);

        Ok(Self {
            name: None,
            path,
            line,
        })
    }
}

impl FromStr for Breakpoint {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (path_str, lineno_str) = s
            .split_once(':')
            .ok_or_else(|| eyre::eyre!("breakpoint specification '{s}' has no colon"))?;

        let lineno = lineno_str.parse().wrap_err("invalid line number")?;
        let mut path = PathBuf::from(path_str);

        // if passed a relative path, assume the current working directory
        if path.is_relative() {
            path = std::env::current_dir()
                .context("getting current working directory")?
                .join(path);
        }

        eyre::ensure!(
            path.is_file(),
            "breakpoint cannot be set on a non-existent file: {}",
            path.display()
        );

        Ok(Self {
            name: None,
            path,
            line: lineno,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PausedFrame {
    pub frame: StackFrame,
    pub variables: Vec<dap_types::Variable>,
}

pub(crate) use dap_types::StackFrame;

pub struct EvaluateResult {
    pub output: String,
    pub error: bool,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Breakpoint;

    macro_rules! assert_res_eq {
        ($a:expr_2021, $b:expr_2021) => {{
            match ($a, $b) {
                (Ok(_), Ok(_)) => {}
                (Err(e1), Err(e2)) => {
                    let s1 = format!("{e1}");
                    let s2 = format!("{e2}");
                    assert_eq!(s1, s2);
                }
                (Err(e), Ok(o)) => panic!("not equal, Err({:?}) != Ok({:?})", e, o),
                (Ok(o), Err(e)) => panic!("not equal, Ok({:?}) != Err({:?})", o, e),
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

    macro_rules! breakpoint_from_str_tests {
        ($($name:ident: $value:expr_2021,)*) => {
            mod breakpoint_from_str {
                use super::super::Breakpoint;
                use std::{path::PathBuf, str::FromStr};

                $(
                    #[test]
                    fn $name () {
                        let (input, expected): (&str, eyre::Result<Breakpoint>) = $value;
                        assert_res_eq!(Breakpoint::from_str(input), expected);
                    }
                )*
            }
        }
    }

    breakpoint_from_str_tests! {
        empty_string: ("", Err(eyre::eyre!("breakpoint specification '' has no colon"))),
        invalid_structure: ("test", Err(eyre::eyre!("breakpoint specification 'test' has no colon"))),
        invalid_line_number: ("test.py:foo", Err(eyre::eyre!("invalid line number"))),
        success: ("../../test.py:16", Ok(Breakpoint { path: PathBuf::from("../../test.py"), line: 16, name: None })),
    }

    mod breakpoint_parse {
        use super::Breakpoint;
        use std::path::{Path, PathBuf};

        #[test]
        fn relative_path_resolved_against_cwd() {
            let cwd = Path::new("/home/user/project");
            let bp = Breakpoint::parse("src/main.py:42", cwd).unwrap();
            // canonicalize won't work on non-existent paths, so we get the joined path
            assert_eq!(bp.path, PathBuf::from("/home/user/project/src/main.py"));
            assert_eq!(bp.line, 42);
        }

        #[test]
        fn absolute_path_used_as_is() {
            let cwd = Path::new("/other");
            let bp = Breakpoint::parse("/home/user/project/app.py:10", cwd).unwrap();
            assert_eq!(bp.path, PathBuf::from("/home/user/project/app.py"));
            assert_eq!(bp.line, 10);
        }

        #[test]
        fn missing_line_number_errors() {
            let cwd = Path::new("/tmp");
            let err = Breakpoint::parse("main.py", cwd).unwrap_err();
            assert!(
                err.to_string().contains("has no colon"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn non_numeric_line_errors() {
            let cwd = Path::new("/tmp");
            let err = Breakpoint::parse("main.py:abc", cwd).unwrap_err();
            assert!(
                err.to_string().contains("invalid line number"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn empty_input_errors() {
            let cwd = Path::new("/tmp");
            let err = Breakpoint::parse("", cwd).unwrap_err();
            assert!(
                err.to_string().contains("empty breakpoint specification"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn whitespace_only_errors() {
            let cwd = Path::new("/tmp");
            let err = Breakpoint::parse("  \t  ", cwd).unwrap_err();
            assert!(
                err.to_string().contains("empty breakpoint specification"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn path_with_spaces() {
            let cwd = Path::new("/home/user");
            let bp = Breakpoint::parse("my project/main.py:5", cwd).unwrap();
            assert_eq!(bp.path, PathBuf::from("/home/user/my project/main.py"));
            assert_eq!(bp.line, 5);
        }

        #[test]
        fn input_is_trimmed() {
            let cwd = Path::new("/home/user");
            let bp = Breakpoint::parse("  src/app.py:7  ", cwd).unwrap();
            assert_eq!(bp.path, PathBuf::from("/home/user/src/app.py"));
            assert_eq!(bp.line, 7);
        }
    }
}
