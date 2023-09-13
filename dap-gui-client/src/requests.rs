//! Requests you can send to a DAP server
use serde::Serialize;

use crate::types::{Seq, StackFrameId, ThreadId, VariablesReference};

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
    Launch(Launch),
    Scopes(Scopes),
    Variables(Variables),
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    pub program: String,
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Disconnect {
    pub terminate_debugee: bool,
}



