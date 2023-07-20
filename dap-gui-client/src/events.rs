use serde::Deserialize;

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEventBody {
    pub reason: String,
    pub thread_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct ThreadEventBody {}

#[derive(Debug, Deserialize)]
pub struct ProcessEventBody {}

#[derive(Debug, Deserialize)]
pub struct ExitedEventBody {}

#[derive(Debug, Deserialize)]
pub struct ContinuedEventBody {}
