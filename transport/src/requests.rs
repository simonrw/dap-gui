//! Requests you can send to a DAP server
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::types::{
    Seq, Source, SourceBreakpoint, StackFrameFormat, StackFrameId, ThreadId, VariablesReference,
};

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

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Next {
    pub thread_id: ThreadId,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StepIn {
    pub thread_id: ThreadId,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StepOut {
    pub thread_id: ThreadId,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Evaluate {
    pub expression: String,
    pub frame_id: Option<StackFrameId>,
    pub context: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StackTrace {
    pub thread_id: ThreadId,
    pub start_frame: Option<usize>,
    pub levels: Option<usize>,
    pub format: Option<StackFrameFormat>,
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PathFormat {
    #[default]
    Path,
    Uri,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Initialize {
    #[serde(rename = "adapterID")]
    pub adapter_id: String,
    pub path_format: PathFormat,

    #[serde(rename = "linesStartAt1")]
    pub lines_start_at_one: bool,
    pub supports_start_debugging_request: bool,
    pub supports_variable_type: bool,
    pub supports_variable_paging: bool,
    pub supports_progress_reporting: bool,
    pub supports_memory_event: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Continue {
    pub thread_id: ThreadId,
    pub single_thread: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Breakpoint {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetFunctionBreakpoints {
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpoints {
    pub source: Source,
    pub breakpoints: Option<Vec<SourceBreakpoint>>,
    pub lines: Option<Vec<usize>>,
    pub source_modified: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetExceptionBreakpoints {
    pub filters: Vec<String>,
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
    pub program: PathBuf,

    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub launch_arguments: Option<LaunchArguments>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scopes {
    pub frame_id: StackFrameId,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Variables {
    pub variables_reference: VariablesReference,
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointLocations {
    pub source: Source,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Terminate {
    pub restart: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Disconnect {
    pub terminate_debugee: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_arguments() {
        let body = RequestBody::Launch(Launch {
            program: PathBuf::from("/"),
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
