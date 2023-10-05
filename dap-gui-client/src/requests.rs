//! Requests you can send to a DAP server
use std::path::PathBuf;

use serde::Serialize;

use crate::types::{
    Seq, Source, SourceBreakpoint, StackFrameFormat, StackFrameId, ThreadId, VariablesReference,
};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub seq: Seq,
    pub r#type: String,
    #[serde(flatten)]
    pub body: RequestBody,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "command", content = "arguments", rename_all = "camelCase")]
pub enum RequestBody {
    StackTrace(StackTrace),
    Threads,
    ConfigurationDone,
    Initialize(Initialize),
    Continue(Continue),
    SetFunctionBreakpoints(SetFunctionBreakpoints),
    SetBreakpoints(SetBreakpoints),
    Attach(Attach),
    Launch(Launch),
    Scopes(Scopes),
    Variables(Variables),
    BreakpointLocations(BreakpointLocations),
    LoadedSources,
    Terminate(Terminate),
    Disconnect(Disconnect),
}

#[derive(Debug, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StackTrace {
    pub thread_id: ThreadId,
    pub start_frame: Option<usize>,
    pub levels: Option<usize>,
    pub format: Option<StackFrameFormat>,
}

#[derive(Default, Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PathFormat {
    #[default]
    Path,
    Uri,
}

#[derive(Debug, Serialize, Clone)]
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Continue {
    pub thread_id: ThreadId,
    pub single_thread: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct Breakpoint {
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetFunctionBreakpoints {
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Debug, Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpoints {
    pub source: Source,
    pub breakpoints: Option<Vec<SourceBreakpoint>>,
    pub lines: Option<Vec<usize>>,
    pub source_modified: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConnectInfo {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PathMapping {
    pub local_root: String,
    pub remote_root: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Attach {
    pub connect: ConnectInfo,
    pub path_mappings: Vec<PathMapping>,
    pub just_my_code: bool,
    pub workspace_folder: PathBuf,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DebugpyLaunchArguments {
    pub just_my_code: bool,
    // pub console: String,
    pub cwd: PathBuf,
    pub show_return_value: bool,
    pub debug_options: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged, rename_all = "camelCase")]
pub enum LaunchArguments {
    Debugpy(DebugpyLaunchArguments),
}

#[derive(Default, Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    pub program: PathBuf,

    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub launch_arguments: Option<LaunchArguments>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scopes {
    pub frame_id: StackFrameId,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Variables {
    pub variables_reference: VariablesReference,
}

#[derive(Default, Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointLocations {
    pub source: Source,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Terminate {
    pub restart: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
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
}
