#[allow(clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
pub use generated::*;

pub type Seq = i64;
pub type ThreadId = i64;
pub type BreakpointId = i64;
pub type StackFrameId = i64;
pub type VariablesReference = i64;
pub type SourceReference = i64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_roundtrip() {
        let thread = Thread {
            id: 1,
            name: "main".to_string(),
        };
        let json = serde_json::to_string(&thread).unwrap();
        let parsed: Thread = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.name, "main");
    }

    #[test]
    fn variable_roundtrip() {
        let json = r#"{
            "name": "x",
            "value": "42",
            "type": "int",
            "variablesReference": 0
        }"#;
        let var: Variable = serde_json::from_str(json).unwrap();
        assert_eq!(var.name, "x");
        assert_eq!(var.value, "42");
        assert_eq!(var.r#type.as_deref(), Some("int"));
    }

    #[test]
    fn stopped_event_body_roundtrip() {
        let json = r#"{
            "reason": "breakpoint",
            "threadId": 1,
            "allThreadsStopped": true,
            "hitBreakpointIds": [1, 2]
        }"#;
        let body: StoppedEventBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.thread_id, Some(1));
        assert_eq!(body.hit_breakpoint_ids, Some(vec![1, 2]));
    }

    #[test]
    fn capabilities_roundtrip() {
        let json = r#"{
            "supportsConfigurationDoneRequest": true,
            "supportsFunctionBreakpoints": true,
            "supportsConditionalBreakpoints": false
        }"#;
        let caps: Capabilities = serde_json::from_str(json).unwrap();
        assert_eq!(caps.supports_configuration_done_request, Some(true));
        assert_eq!(caps.supports_function_breakpoints, Some(true));
    }

    #[test]
    fn request_arguments_serialize() {
        let args = RequestArguments::Continue(ContinueArguments {
            thread_id: 1,
            single_thread: None,
        });
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains("\"command\":\"continue\""));
        assert!(json.contains("\"threadId\":1"));
    }

    #[test]
    fn request_arguments_threads_no_args() {
        let args = RequestArguments::Threads;
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains("\"command\":\"threads\""));
    }

    #[test]
    fn response_body_roundtrip() {
        let json = r#"{"command": "continue", "body": {"allThreadsContinued": true}}"#;
        let body: ResponseBody = serde_json::from_str(json).unwrap();
        assert!(matches!(body, ResponseBody::Continue(_)));
    }

    #[test]
    fn event_roundtrip() {
        let json = r#"{"event": "stopped", "body": {"reason": "breakpoint", "threadId": 1}}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert!(matches!(event, Event::Stopped(_)));
    }

    #[test]
    fn event_unknown_graceful() {
        let json = r#"{"event": "someFutureEvent", "body": {"data": 123}}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert!(matches!(event, Event::Unknown));
    }

    #[test]
    fn event_initialized_no_body() {
        let json = r#"{"event": "initialized"}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert!(matches!(event, Event::Initialized));
    }

    #[test]
    fn source_with_path() {
        let json = r#"{"name": "test.py", "path": "/home/user/test.py", "sourceReference": 0}"#;
        let source: Source = serde_json::from_str(json).unwrap();
        assert_eq!(
            source.path,
            Some(std::path::PathBuf::from("/home/user/test.py"))
        );
    }

    #[test]
    fn stack_frame_fields() {
        let json = r#"{
            "id": 1,
            "name": "main",
            "line": 10,
            "column": 0,
            "source": {"name": "test.py", "path": "/tmp/test.py"}
        }"#;
        let frame: StackFrame = serde_json::from_str(json).unwrap();
        assert_eq!(frame.id, 1);
        assert_eq!(frame.line, 10);
        assert_eq!(frame.column, 0);
    }

    #[test]
    fn source_breakpoint_fields() {
        let json = r#"{"line": 42}"#;
        let bp: SourceBreakpoint = serde_json::from_str(json).unwrap();
        assert_eq!(bp.line, 42);
    }
}
