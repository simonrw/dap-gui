//! Requests you can send to a DAP server
use std::path::PathBuf;

use serde::Serialize;

use crate::types::{Seq, StackFrameId, ThreadId, VariablesReference, Source, SourceBreakpoint};

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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StackTrace {
    pub thread_id: ThreadId,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Initialize {
    #[serde(rename = "adapterID")]
    pub adapter_id: String,
    #[serde(rename = "linesStartAt1")]
    pub lines_start_at_one: Option<bool>,
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
pub struct Launch {
    pub program: PathBuf,
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
