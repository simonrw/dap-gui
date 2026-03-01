use std::{collections::HashMap, path::PathBuf, str::FromStr};

use transport::{
    DEFAULT_DAP_PORT,
    requests::{self, DebugpyLaunchArguments},
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
pub struct ProgramState {
    pub stack: Vec<types::StackFrame>,
    pub breakpoints: Vec<types::Breakpoint>,
    pub paused_frame: types::PausedFrame,
}

#[derive(Debug, Clone)]
pub enum Event {
    Uninitialised,
    Initialised,
    Paused(ProgramState),
    ScopeChange(ProgramState),
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
            } => Event::Paused(ProgramState {
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
#[derive(Debug, Clone)]
pub struct AttachArguments {
    /// Working directory for the debugging session
    pub working_directory: PathBuf,

    /// Debugger port to connect to (defaults to 5678)
    pub port: Option<u16>,

    /// Host to connect to (defaults to "localhost")
    pub host: Option<String>,

    /// Programming language of the debugee
    pub language: Language,

    /// Custom mappings from the running code (e.g. in a Docker container) to local source checkout
    pub path_mappings: Option<Vec<requests::PathMapping>>,

    /// Only debug user code, not library code
    pub just_my_code: Option<bool>,
}

impl AttachArguments {
    pub fn to_request(self) -> requests::RequestBody {
        requests::RequestBody::Attach(requests::Attach {
            connect: requests::ConnectInfo {
                host: self.host.unwrap_or_else(|| "localhost".to_string()),
                port: self.port.unwrap_or(DEFAULT_DAP_PORT),
            },
            path_mappings: self.path_mappings.unwrap_or_default(),
            just_my_code: self.just_my_code.unwrap_or(false),
            workspace_folder: self.working_directory,
        })
    }
}

/// Arguments for launching a new process
#[derive(Debug, Clone)]
pub struct LaunchArguments {
    /// Program to run (mutually exclusive with `module`)
    pub program: Option<PathBuf>,

    /// Python module to run instead of a program file (e.g. "pytest")
    pub module: Option<String>,

    /// Command-line arguments to the program
    pub args: Option<Vec<String>>,

    /// Environment variables for the launched process
    pub env: Option<HashMap<String, String>>,

    /// Current working directory for the launched process
    pub working_directory: Option<PathBuf>,

    /// Language used to create the process
    pub language: Language,

    /// Only debug user code, not library code
    pub just_my_code: Option<bool>,

    /// Stop at the first line of user code
    pub stop_on_entry: Option<bool>,
}

impl LaunchArguments {
    pub fn from_path(program: impl Into<PathBuf>, language: Language) -> Self {
        let program = program.into();
        let working_directory = program.parent().map(|p| p.to_path_buf());

        Self {
            program: Some(program),
            module: None,
            args: None,
            env: None,
            working_directory,
            language,
            just_my_code: None,
            stop_on_entry: None,
        }
    }
}

impl LaunchArguments {
    pub fn to_request(self) -> eyre::Result<requests::RequestBody> {
        let program = self
            .program
            .map(|p| p.canonicalize())
            .transpose()
            .map_err(|e| eyre::eyre!("launch target not a valid path: {e}"))?;

        let cwd = self.working_directory.unwrap_or_else(|| {
            program
                .as_ref()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        });

        let just_my_code = self.just_my_code.unwrap_or(true);
        let stop_on_entry = self.stop_on_entry.unwrap_or(false);

        match self.language {
            Language::DebugPy => Ok(requests::RequestBody::Launch(requests::Launch {
                program,
                module: self.module,
                args: self.args,
                env: self.env,
                launch_arguments: Some(transport::requests::LaunchArguments::Debugpy(
                    DebugpyLaunchArguments {
                        just_my_code,
                        cwd,
                        show_return_value: true,
                        debug_options: vec![
                            "DebugStdLib".to_string(),
                            "ShowReturnValue".to_string(),
                        ],
                        stop_on_entry,
                        is_output_redirected: false,
                    },
                )),
            })),
            Language::Delve => {
                eyre::bail!("Delve launch mode is not yet supported")
            }
        }
    }
}
