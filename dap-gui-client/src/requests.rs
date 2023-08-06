use serde::Serialize;

use crate::types::{ThreadId, StackFrameId};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub seq: i64,
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