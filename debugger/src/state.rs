use std::{path::PathBuf, str::FromStr};

use transport::{
    requests::{self, DebugpyLaunchArguments},
    DEFAULT_DAP_PORT,
};

use crate::types::{self, PausedFrame};

#[derive(Debug)]
pub(crate) enum DebuggerState {
    Initialised,
    Paused {
        stack: Vec<types::StackFrame>,
        paused_frame: Box<PausedFrame>,
        breakpoints: Vec<types::Breakpoint>,
    },
    Running,
    Ended,
}

/// Represents the current program state
#[derive(Debug, Clone)]
pub struct ProgramDescription {
    pub stack: Vec<types::StackFrame>,
    pub breakpoints: Vec<types::Breakpoint>,
    pub paused_frame: types::PausedFrame,
}

#[derive(Debug, Clone)]
pub enum Event {
    Uninitialised,
    Initialised,
    Paused(ProgramDescription),
    ScopeChange(ProgramDescription),
    Running,
    Ended,
}

impl<'a> From<&'a DebuggerState> for Event {
    fn from(value: &'a DebuggerState) -> Self {
        match value {
            DebuggerState::Initialised => Event::Initialised,
            DebuggerState::Paused {
                stack,
                paused_frame,
                breakpoints,
                ..
            } => Event::Paused(ProgramDescription {
                stack: stack.clone(),
                paused_frame: *paused_frame.clone(),
                breakpoints: breakpoints.clone(),
            }),
            DebuggerState::Running => Event::Running,
            DebuggerState::Ended => Event::Ended,
        }
    }
}

/// Languages supported by the debugger crate
#[derive(Debug, Clone, Copy)]
pub enum Language {
    DebugPy,
    Delve,
}

impl FromStr for Language {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "debugpy" => Ok(Self::DebugPy),
            "delve" => Ok(Self::Delve),
            other => Err(eyre::eyre!("invalid language {other}")),
        }
    }
}

/// Arguments for attaching to a running process
#[derive(Debug)]
pub struct AttachArguments {
    /// Working directory for the debugging session
    pub working_directory: PathBuf,

    /// Debugger port to connect to (defaults to 5678)
    pub port: Option<u16>,

    /// Programming language of the debugee
    pub language: Language,

    /// Custom mappings from the running code (e.g. in a Docker container) to local source checkout
    pub path_mappings: Option<Vec<requests::PathMapping>>,
}

impl AttachArguments {
    pub fn to_request(self) -> requests::RequestBody {
        requests::RequestBody::Attach(requests::Attach {
            connect: requests::ConnectInfo {
                host: "localhost".to_string(),
                port: self.port.unwrap_or(DEFAULT_DAP_PORT),
            },
            path_mappings: self.path_mappings.unwrap_or_default(),
            just_my_code: false,
            workspace_folder: self.working_directory,
        })
    }
}

/// Arguments for launching a new process
#[derive(Debug)]
pub struct LaunchArguments {
    /// Program to run
    pub program: PathBuf,

    /// Current working directory for the launched process
    pub working_directory: Option<PathBuf>,

    /// Language used to create the process
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
            Language::Delve => todo!(),
        }
    }
}
