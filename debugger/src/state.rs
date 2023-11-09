use std::path::PathBuf;

use transport::requests::{self, DebugpyLaunchArguments};

use crate::types;

pub(crate) enum DebuggerState {
    Initialised,
    Paused {
        stack: Vec<types::StackFrame>,
        source: crate::FileSource,
    },
    Running,
    Ended,
}

#[derive(Debug, Clone)]
pub enum Event {
    Uninitialised,
    Initialised,
    Paused {
        stack: Vec<types::StackFrame>,
        source: crate::FileSource,
    },
    Running,
    Ended,
}

impl<'a> From<&'a DebuggerState> for Event {
    fn from(value: &'a DebuggerState) -> Self {
        match value {
            DebuggerState::Initialised => Event::Initialised,
            DebuggerState::Paused { stack, source, .. } => Event::Paused {
                stack: stack.clone(),
                source: source.clone(),
            },
            DebuggerState::Running => Event::Running,
            DebuggerState::Ended => Event::Ended,
        }
    }
}

pub enum Language {
    DebugPy,
}

pub struct LaunchArguments {
    pub program: PathBuf,
    pub working_directory: Option<PathBuf>,
    pub language: Language,
}

impl LaunchArguments {
    pub fn from_path(program: impl Into<PathBuf>, language: Language) -> Self {
        let program = program.into();
        let working_directory = program.parent().unwrap().to_path_buf();

        Self {
            program,
            working_directory: Some(working_directory),
            language,
        }
    }
}

impl LaunchArguments {
    pub fn to_request(self) -> requests::RequestBody {
        let program = self
            .program
            .canonicalize()
            .expect("launch target not a valid path");
        let cwd = self
            .working_directory
            .unwrap_or_else(|| program.parent().unwrap().to_path_buf());

        match self.language {
            Language::DebugPy => requests::RequestBody::Launch(requests::Launch {
                program,
                launch_arguments: Some(transport::requests::LaunchArguments::Debugpy(
                    DebugpyLaunchArguments {
                        just_my_code: true,
                        cwd,
                        show_return_value: true,
                        debug_options: vec![
                            "DebugStdLib".to_string(),
                            "ShowReturnValue".to_string(),
                        ],
                        stop_on_entry: false,
                        is_output_redirected: false,
                    },
                )),
            }),
        }
    }
}
