//! Events emitted by a DAP server
pub use dap_types::{
    ContinuedEventBody, Event, ExitedEventBody, ModuleEventBody, OutputEventBody, ProcessEventBody,
    StoppedEventBody, TerminatedEventBody, ThreadEventBody,
};

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
