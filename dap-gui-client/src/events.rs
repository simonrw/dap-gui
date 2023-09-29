//! Events emitted by a DAP server
use serde::Deserialize;

use crate::types::{BreakpointId, Module, Source, ThreadId};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", content = "body", rename_all = "camelCase")]
#[non_exhaustive]
pub enum Event {
    Initialized,
    Output(OutputEventBody),
    Process(ProcessEventBody),
    Stopped(StoppedEventBody),
    Continued(ContinuedEventBody),
    Thread(ThreadEventBody),
    Exited(ExitedEventBody),
    Terminated,
    // TODO: handle unknown event types
    // debugpy types
    DebugpyWaitingForServer {
        host: String,
        port: u16,
    },
    Module(ModuleEventBody),
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputEventBody {
    // pub category: Option<OutputEventCategory>,
    pub output: String,
    // pub group: Option<OutputEventGroup>,
    pub variables_reference: Option<i64>,
    pub source: Option<Source>,
    pub line: Option<i64>,
    pub column: Option<i64>,
    // pub data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StoppedReason {
    #[serde(rename = "step")]
    Step,
    #[serde(rename = "function breakpoint")]
    FunctionBreakpoint,
    Other(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEventBody {
    pub reason: StoppedReason,
    pub thread_id: ThreadId,
    pub hit_breakpoint_ids: Option<Vec<BreakpointId>>,
    pub description: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEventBody {
    pub reason: String,
    pub thread_id: ThreadId,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEventBody {
    pub name: String,
    pub start_method: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExitedEventBody {}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuedEventBody {
    pub thread_id: ThreadId,
    pub all_threads_continued: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleEventBody {
    // TODO: enum
    pub reason: String,
    pub module: Module,
}
