use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", content = "body", rename_all = "camelCase")]
pub enum Event {
    Initialized,
    Output(OutputEventBody),
    Process(ProcessEventBody),
    Stopped(StoppedEventBody),
    Continued(ContinuedEventBody),
    Thread(ThreadEventBody),
    Exited(ExitedEventBody),
    Terminated,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputEventBody {
    // pub category: Option<OutputEventCategory>,
    pub output: String,
    // pub group: Option<OutputEventGroup>,
    pub variables_reference: Option<i64>,
    // pub source: Option<Source>,
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
    pub thread_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadEventBody {}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessEventBody {}

#[derive(Debug, Clone, Deserialize)]
pub struct ExitedEventBody {}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuedEventBody {
    pub thread_id: i64,
    pub all_threads_continued: Option<bool>,
}
