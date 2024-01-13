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
        paused_frame: PausedFrame,
        breakpoints: Vec<types::Breakpoint>,
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
        breakpoints: Vec<types::Breakpoint>,
        paused_frame: types::PausedFrame,
    },
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
            } => Event::Paused {
                stack: stack.clone(),
                paused_frame: paused_frame.clone(),
                breakpoints: breakpoints.clone(),
            },
            DebuggerState::Running => Event::Running,
            DebuggerState::Ended => Event::Ended,
        }
    }
}

#[derive(Clone, Copy)]
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

pub struct AttachArguments {
    pub working_directory: PathBuf,
    pub port: Option<u16>,
    pub language: Language,
}

impl AttachArguments {
    pub fn to_request(self) -> requests::RequestBody {
        requests::RequestBody::Attach(requests::Attach {
            connect: requests::ConnectInfo {
                host: "localhost".to_string(),
                port: self.port.unwrap_or(DEFAULT_DAP_PORT),
            },
            path_mappings: Vec::new(),
            just_my_code: false,
            workspace_folder: self.working_directory,
        })
    }
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
            Language::Delve => todo!(),
        }
    }
}
