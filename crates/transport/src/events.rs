//! Events emitted by a DAP server
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::types::{BreakpointId, Module, Source, ThreadId};

#[derive(Debug, Clone, Serialize)]
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
    // debugpy types
    DebugpyWaitingForServer {
        host: String,
        port: u16,
    },
    Module(ModuleEventBody),
    // Catch-all for unknown event types - not part of serde tag/content
    #[serde(skip)]
    Unknown,
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;

        // Try to deserialize as a known event type first
        match serde_json::from_value::<EventHelper>(value.clone()) {
            Ok(helper) => Ok(helper.into()),
            Err(_) => {
                // If deserialization fails, log the unknown event and return Unknown
                if let Some(event_name) = value.get("event").and_then(|v| v.as_str()) {
                    tracing::debug!(event = event_name, "received unknown event, ignoring");
                }
                Ok(Event::Unknown)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", content = "body", rename_all = "camelCase")]
enum EventHelper {
    Initialized,
    Output(OutputEventBody),
    Process(ProcessEventBody),
    Stopped(StoppedEventBody),
    Continued(ContinuedEventBody),
    Thread(ThreadEventBody),
    Exited(ExitedEventBody),
    Terminated,
    DebugpyWaitingForServer { host: String, port: u16 },
    Module(ModuleEventBody),
}

impl From<EventHelper> for Event {
    fn from(helper: EventHelper) -> Self {
        match helper {
            EventHelper::Initialized => Event::Initialized,
            EventHelper::Output(body) => Event::Output(body),
            EventHelper::Process(body) => Event::Process(body),
            EventHelper::Stopped(body) => Event::Stopped(body),
            EventHelper::Continued(body) => Event::Continued(body),
            EventHelper::Thread(body) => Event::Thread(body),
            EventHelper::Exited(body) => Event::Exited(body),
            EventHelper::Terminated => Event::Terminated,
            EventHelper::DebugpyWaitingForServer { host, port } => {
                Event::DebugpyWaitingForServer { host, port }
            }
            EventHelper::Module(body) => Event::Module(body),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StoppedReason {
    #[serde(rename = "step")]
    Step,
    #[serde(rename = "function breakpoint")]
    FunctionBreakpoint,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEventBody {
    pub reason: StoppedReason,
    pub thread_id: ThreadId,
    pub hit_breakpoint_ids: Option<Vec<BreakpointId>>,
    pub description: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEventBody {
    pub reason: String,
    pub thread_id: ThreadId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEventBody {
    pub name: String,
    pub start_method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitedEventBody {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuedEventBody {
    pub thread_id: ThreadId,
    pub all_threads_continued: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleEventBody {
    // TODO: enum
    pub reason: String,
    pub module: Module,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_event_deserialization() {
        // Test that unknown events deserialize to Event::Unknown
        let unknown_event_json = r#"{"event": "debugpySockets", "body": {"sockets": [{"host": "127.0.0.1", "port": 57003, "internal": false}]}}"#;

        let result: Result<Event, _> = serde_json::from_str(unknown_event_json);
        assert!(
            result.is_ok(),
            "Failed to deserialize unknown event: {:?}",
            result
        );

        let event = result.unwrap();
        assert!(
            matches!(event, Event::Unknown),
            "Expected Event::Unknown, got {:?}",
            event
        );
    }

    #[test]
    fn test_known_event_deserialization() {
        // Test that known events still deserialize correctly
        let initialized_event_json = r#"{"event": "initialized"}"#;

        let result: Result<Event, _> = serde_json::from_str(initialized_event_json);
        assert!(
            result.is_ok(),
            "Failed to deserialize initialized event: {:?}",
            result
        );

        let event = result.unwrap();
        assert!(
            matches!(event, Event::Initialized),
            "Expected Event::Initialized, got {:?}",
            event
        );
    }
}
