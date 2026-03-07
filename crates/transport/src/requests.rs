//! Requests you can send to a DAP server
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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

pub type Seq = i64;
pub type ThreadId = i64;
pub type StackFrameId = i64;
pub type VariablesReference = i64;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub seq: Seq,
    #[serde(skip_deserializing)]
    pub r#type: String,
    #[serde(flatten)]
    pub body: RequestBody,
}

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

// Types not in the spec (custom to this crate)

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PathFormat {
    #[default]
    Path,
    Uri,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConnectInfo {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathMapping {
    pub local_root: String,
    pub remote_root: String,
}

impl PathMapping {
    /// resolve VS Code workspace placeholders, e.g. ${workspaceFolder}
    pub fn resolve(&mut self, root: impl AsRef<Path>) {
        let root = root.as_ref();
        if self.local_root.contains("workspaceFolder:") {
            // TODO: assume only one location
            let Some((_, after)) = self.local_root.split_once("${workspaceFolder:") else {
                todo!()
            };

            let Some((subpath, _)) = after.split_once("}") else {
                todo!()
            };

            self.local_root = self.local_root.replace(
                &format!("${{workspaceFolder:{}}}", subpath),
                &format!("{}/{}", root.display(), subpath),
            );
        } else {
            self.local_root = self
                .local_root
                .replace("${workspaceFolder}", &format!("{}", root.display()));
        }
    }
}

/// Custom breakpoint type for function breakpoints used in the old API.
/// The spec uses `FunctionBreakpoint` with fields: name, condition, hit_condition.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Breakpoint {
    pub name: String,
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
    // pub console: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_arguments() {
        let body = RequestBody::Launch(Launch {
            program: Some(PathBuf::from("/")),
            module: None,
            args: None,
            env: None,
            launch_arguments: Some(LaunchArguments::Debugpy(DebugpyLaunchArguments {
                just_my_code: true,
                // console: "integratedTerminal".to_string(),
                cwd: std::env::current_dir().unwrap(),
                show_return_value: true,
                debug_options: vec!["DebugStdLib".to_string(), "ShowReturnValue".to_string()],
                stop_on_entry: false,
                is_output_redirected: false,
            })),
        });

        let s = serde_json::to_string(&body).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();

        let just_my_code = v
            .as_object()
            .unwrap()
            .get("arguments")
            .unwrap()
            .as_object()
            .unwrap()
            .get("justMyCode")
            .unwrap()
            .as_bool()
            .unwrap();

        assert!(just_my_code);
    }

    #[test]
    fn path_mapping_resolving() {
        let root = std::env::current_dir().unwrap();
        let tests = [
            (
                "${workspaceFolder}/foo".to_string(),
                format!("{}/foo", root.display()),
            ),
            (
                "${workspaceFolder:a}/b".to_string(),
                format!("{}/a/b", root.display()),
            ),
        ];

        for (local_root, expected) in tests {
            let mut mapping = PathMapping {
                local_root,
                remote_root: "/".to_string(),
            };
            mapping.resolve(&root);
            assert_eq!(mapping.local_root, expected);
        }
    }
}
