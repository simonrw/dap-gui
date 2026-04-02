//! Request construction types for the DAP protocol.
//!
//! These types are used to build requests to send to a debug adapter.
//! They were originally in the `transport` crate and have been moved here
//! as part of the transport crate removal.

use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

// Re-export spec argument types under their old names for backwards compatibility
pub use dap_types::BreakpointLocationsArguments as BreakpointLocations;
pub use dap_types::ContinueArguments as Continue;
pub use dap_types::DisconnectArguments as Disconnect;
pub use dap_types::EvaluateArguments as Evaluate;
pub use dap_types::FunctionBreakpoint;
pub use dap_types::InitializeRequestArguments as Initialize;
pub use dap_types::NextArguments as Next;
pub use dap_types::ScopesArguments as Scopes;
pub use dap_types::SetBreakpointsArguments as SetBreakpoints;
pub use dap_types::SetExceptionBreakpointsArguments as SetExceptionBreakpoints;
pub use dap_types::SetFunctionBreakpointsArguments as SetFunctionBreakpoints;
pub use dap_types::StackTraceArguments as StackTrace;
pub use dap_types::StepInArguments as StepIn;
pub use dap_types::StepOutArguments as StepOut;
pub use dap_types::TerminateArguments as Terminate;
pub use dap_types::VariablesArguments as Variables;

pub use launch_configuration::PathMapping;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "command", content = "arguments", rename_all = "camelCase")]
pub enum RequestBody {
    StackTrace(StackTrace),
    Threads,
    ConfigurationDone,
    Initialize(Initialize),
    Continue(Continue),
    SetFunctionBreakpoints(SetFunctionBreakpoints),
    SetBreakpoints(SetBreakpoints),
    SetExceptionBreakpoints(SetExceptionBreakpoints),
    Attach(Attach),
    Launch(Launch),
    Scopes(Scopes),
    Variables(Variables),
    BreakpointLocations(BreakpointLocations),
    LoadedSources,
    Terminate(Terminate),
    Disconnect(Disconnect),
    Next(Next),
    StepIn(StepIn),
    StepOut(StepOut),
    Evaluate(Evaluate),
}

impl RequestBody {
    /// Returns the DAP protocol command name for this request.
    ///
    /// This is the canonical mapping from enum variant to wire-format command
    /// string, kept in one place so it stays in sync with the enum definition.
    pub fn command_name(&self) -> &'static str {
        match self {
            Self::StackTrace(_) => "stackTrace",
            Self::Threads => "threads",
            Self::ConfigurationDone => "configurationDone",
            Self::Initialize(_) => "initialize",
            Self::Continue(_) => "continue",
            Self::SetFunctionBreakpoints(_) => "setFunctionBreakpoints",
            Self::SetBreakpoints(_) => "setBreakpoints",
            Self::SetExceptionBreakpoints(_) => "setExceptionBreakpoints",
            Self::Attach(_) => "attach",
            Self::Launch(_) => "launch",
            Self::Scopes(_) => "scopes",
            Self::Variables(_) => "variables",
            Self::BreakpointLocations(_) => "breakpointLocations",
            Self::LoadedSources => "loadedSources",
            Self::Terminate(_) => "terminate",
            Self::Disconnect(_) => "disconnect",
            Self::Next(_) => "next",
            Self::StepIn(_) => "stepIn",
            Self::StepOut(_) => "stepOut",
            Self::Evaluate(_) => "evaluate",
        }
    }

    /// Serialize only the arguments payload for the wire format.
    ///
    /// Returns `None` for argument-less commands (Threads, ConfigurationDone,
    /// LoadedSources), and `Some(value)` for commands that carry arguments.
    pub fn into_arguments(self) -> eyre::Result<Option<serde_json::Value>> {
        match self {
            Self::Threads | Self::ConfigurationDone | Self::LoadedSources => Ok(None),
            Self::StackTrace(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Initialize(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Continue(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::SetFunctionBreakpoints(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::SetBreakpoints(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::SetExceptionBreakpoints(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Attach(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Launch(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Scopes(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Variables(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::BreakpointLocations(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Terminate(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Disconnect(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Next(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::StepIn(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::StepOut(args) => Ok(Some(serde_json::to_value(args)?)),
            Self::Evaluate(args) => Ok(Some(serde_json::to_value(args)?)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConnectInfo {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Attach {
    pub connect: ConnectInfo,
    pub path_mappings: Vec<PathMapping>,
    pub just_my_code: bool,
    pub workspace_folder: PathBuf,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DebugpyLaunchArguments {
    pub just_my_code: bool,
    pub cwd: PathBuf,
    pub show_return_value: bool,
    pub debug_options: Vec<String>,
    pub stop_on_entry: bool,
    pub is_output_redirected: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged, rename_all = "camelCase")]
pub enum LaunchArguments {
    Debugpy(DebugpyLaunchArguments),
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub launch_arguments: Option<LaunchArguments>,
}
