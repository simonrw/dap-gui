//! DAP message types.
//!
//! This module defines the message types used in the Debug Adapter Protocol.
//! Messages are categorized into incoming (from debug adapter) and outgoing
//! (to debug adapter) types.

use serde::{Deserialize, Serialize};

/// Sequence number type for message ordering and request-response correlation.
pub type Seq = i64;

/// An incoming DAP message from the debug adapter.
///
/// The debug adapter can send three types of messages:
/// - `Response`: A response to a previously sent request
/// - `Event`: An asynchronous notification about debugger state
/// - `Request`: A "reverse request" from the adapter to the client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Message {
    /// A response to a request sent by the client.
    Response(Response),
    /// An asynchronous event notification.
    Event(Event),
    /// A reverse request from the debug adapter.
    Request(Request),
}

/// A response message from the debug adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    /// Sequence number of this response.
    pub seq: Seq,
    /// Sequence number of the request this response is for.
    #[serde(rename = "request_seq")]
    pub request_seq: Seq,
    /// Whether the request was successful.
    pub success: bool,
    /// The command that was requested.
    pub command: String,
    /// Error message if success is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Response body (command-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// An event notification from the debug adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Sequence number of this event.
    pub seq: Seq,
    /// The event type.
    pub event: String,
    /// Event body (event-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// A request message (either outgoing or reverse request).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Sequence number of this request.
    pub seq: Seq,
    /// The command to execute.
    pub command: String,
    /// Command arguments (command-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// An outgoing response message (for mock adapters or reverse request responses).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingResponse {
    /// Sequence number of this response.
    pub seq: Seq,
    /// Sequence number of the request this response is for.
    #[serde(rename = "request_seq")]
    pub request_seq: Seq,
    /// Whether the request was successful.
    pub success: bool,
    /// The command that was requested.
    pub command: String,
    /// Error message if success is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Response body (command-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// An outgoing event message (for mock adapters).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingEvent {
    /// Sequence number of this event.
    pub seq: Seq,
    /// The event type.
    pub event: String,
    /// Event body (event-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// An outgoing message to send over the transport.
///
/// This enum supports:
/// - `Request`: Standard client-to-adapter requests
/// - `Response`: Responses to reverse requests (or mock adapter responses in tests)
/// - `Event`: Events from mock adapters in tests
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OutgoingMessage {
    /// A request to send to the debug adapter.
    Request(Request),
    /// A response to a reverse request (or from a mock adapter).
    Response(OutgoingResponse),
    /// An event (from a mock adapter in tests).
    Event(OutgoingEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_response() {
        let json = r#"{
            "seq": 1,
            "type": "response",
            "request_seq": 1,
            "success": true,
            "command": "initialize",
            "body": {"supportsConfigurationDoneRequest": true}
        }"#;

        let msg: Message = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, Message::Response(r) if r.success));
    }

    #[test]
    fn deserialize_event() {
        let json = r#"{
            "seq": 2,
            "type": "event",
            "event": "initialized"
        }"#;

        let msg: Message = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, Message::Event(e) if e.event == "initialized"));
    }

    #[test]
    fn deserialize_request() {
        let json = r#"{
            "seq": 3,
            "type": "request",
            "command": "runInTerminal",
            "arguments": {"kind": "integrated"}
        }"#;

        let msg: Message = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, Message::Request(r) if r.command == "runInTerminal"));
    }

    #[test]
    fn serialize_outgoing_request() {
        let msg = OutgoingMessage::Request(Request {
            seq: 1,
            command: "initialize".to_string(),
            arguments: Some(serde_json::json!({"clientID": "test"})),
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"request""#));
        assert!(json.contains(r#""command":"initialize""#));
    }
}
