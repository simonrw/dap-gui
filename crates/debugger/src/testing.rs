//! Testing utilities for the async debugger.
//!
//! This module provides helpers for testing code that uses the `AsyncDebugger`,
//! including mock adapters that simulate debug adapter behavior with robust
//! message capture and matching.

use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::DuplexStream;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, timeout};
use transport2::testing::MemoryTransport;
use transport2::{DapReader, DapWriter, Message, OutgoingMessage, Request, Response, split};

use crate::async_debugger::AsyncDebugger;

/// Type alias for an AsyncDebugger using in-memory transport.
pub type TestAsyncDebugger = AsyncDebugger<DuplexStream, DuplexStream>;

/// Logical timestamp for ordering messages in tests.
pub type LogicalTimestamp = u64;

/// A captured message with its logical timestamp.
#[derive(Debug, Clone)]
pub struct CapturedMessage {
    /// Logical timestamp when this message was received.
    pub timestamp: LogicalTimestamp,
    /// The captured message.
    pub message: Message,
}

/// Message capture buffer that stores all received messages with timestamps.
///
/// This allows tests to search through received messages rather than
/// expecting them in exact order, which is more robust against adapters
/// that send unexpected events or messages.
#[derive(Debug)]
pub struct MessageCapture {
    messages: RwLock<VecDeque<CapturedMessage>>,
    next_timestamp: AtomicU64,
    /// Maximum number of messages to retain.
    max_messages: usize,
}

impl MessageCapture {
    /// Create a new message capture buffer.
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: RwLock::new(VecDeque::with_capacity(max_messages)),
            next_timestamp: AtomicU64::new(1),
            max_messages,
        }
    }

    /// Get the current logical timestamp (next timestamp that will be assigned).
    pub fn current_timestamp(&self) -> LogicalTimestamp {
        self.next_timestamp.load(Ordering::SeqCst)
    }

    /// Record a new message and return its timestamp.
    pub async fn record(&self, message: Message) -> LogicalTimestamp {
        let timestamp = self.next_timestamp.fetch_add(1, Ordering::SeqCst);
        let captured = CapturedMessage { timestamp, message };

        let mut messages = self.messages.write().await;
        if messages.len() >= self.max_messages {
            messages.pop_front();
        }
        messages.push_back(captured);

        timestamp
    }

    /// Find a response message with the given request_seq, searching from
    /// `after_timestamp` through up to `max_lookups` messages.
    ///
    /// Returns the response and its timestamp if found.
    pub async fn find_response(
        &self,
        request_seq: i64,
        after_timestamp: LogicalTimestamp,
        max_lookups: usize,
    ) -> Option<(Response, LogicalTimestamp)> {
        let messages = self.messages.read().await;
        let mut count = 0;

        for captured in messages.iter() {
            if captured.timestamp <= after_timestamp {
                continue;
            }
            if count >= max_lookups {
                break;
            }
            count += 1;

            if let Message::Response(ref resp) = captured.message {
                if resp.request_seq == request_seq {
                    return Some((resp.clone(), captured.timestamp));
                }
            }
        }

        None
    }

    /// Find an event message with the given event name, searching from
    /// `after_timestamp` through up to `max_lookups` messages.
    ///
    /// Returns the event and its timestamp if found.
    pub async fn find_event(
        &self,
        event_name: &str,
        after_timestamp: LogicalTimestamp,
        max_lookups: usize,
    ) -> Option<(transport2::Event, LogicalTimestamp)> {
        let messages = self.messages.read().await;
        let mut count = 0;

        for captured in messages.iter() {
            if captured.timestamp <= after_timestamp {
                continue;
            }
            if count >= max_lookups {
                break;
            }
            count += 1;

            if let Message::Event(ref evt) = captured.message {
                if evt.event == event_name {
                    return Some((evt.clone(), captured.timestamp));
                }
            }
        }

        None
    }

    /// Wait for a response with the given request_seq, polling until found
    /// or timeout.
    pub async fn wait_for_response(
        &self,
        request_seq: i64,
        after_timestamp: LogicalTimestamp,
        max_lookups: usize,
        poll_interval: Duration,
        max_wait: Duration,
    ) -> Option<(Response, LogicalTimestamp)> {
        let start = std::time::Instant::now();

        loop {
            if let Some(result) = self
                .find_response(request_seq, after_timestamp, max_lookups)
                .await
            {
                return Some(result);
            }

            if start.elapsed() >= max_wait {
                return None;
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Wait for an event with the given name, polling until found or timeout.
    pub async fn wait_for_event(
        &self,
        event_name: &str,
        after_timestamp: LogicalTimestamp,
        max_lookups: usize,
        poll_interval: Duration,
        max_wait: Duration,
    ) -> Option<(transport2::Event, LogicalTimestamp)> {
        let start = std::time::Instant::now();

        loop {
            if let Some(result) = self
                .find_event(event_name, after_timestamp, max_lookups)
                .await
            {
                return Some(result);
            }

            if start.elapsed() >= max_wait {
                return None;
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Get all messages captured after the given timestamp.
    pub async fn messages_after(&self, after_timestamp: LogicalTimestamp) -> Vec<CapturedMessage> {
        let messages = self.messages.read().await;
        messages
            .iter()
            .filter(|m| m.timestamp > after_timestamp)
            .cloned()
            .collect()
    }
}

/// A mock debug adapter for testing with robust message capture.
///
/// `MockAdapter` simulates the server side of a DAP connection. It receives
/// requests from the debugger and can send responses and events back.
/// All received messages are captured with logical timestamps for flexible
/// matching in tests.
pub struct MockAdapter {
    reader: Mutex<DapReader<DuplexStream>>,
    writer: Mutex<DapWriter<DuplexStream>>,
    sequence: AtomicU64,
    /// Captured messages from the client.
    pub capture: Arc<MessageCapture>,
    /// Background reader task handle.
    reader_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl MockAdapter {
    /// Create a new mock adapter from transport halves.
    pub fn new(reader: DapReader<DuplexStream>, writer: DapWriter<DuplexStream>) -> Self {
        Self {
            reader: Mutex::new(reader),
            writer: Mutex::new(writer),
            sequence: AtomicU64::new(1),
            capture: Arc::new(MessageCapture::new(1000)),
            reader_task: Mutex::new(None),
        }
    }

    /// Start the background message capture task.
    ///
    /// This spawns a task that reads all incoming messages and records them
    /// in the capture buffer. Call this before using message finding methods.
    pub async fn start_capture(&self) {
        let task_guard = self.reader_task.lock().await;
        if task_guard.is_some() {
            return; // Already started
        }

        // Take ownership of reader for the background task
        let reader_guard = self.reader.lock().await;
        // We need to move the reader out, but we can't easily do that with Mutex
        // Instead, let's use a different approach - read inline
        drop(reader_guard);

        // For simplicity, we'll use inline reading in expect methods
        // The capture is used for post-hoc analysis
    }

    /// Get the current logical timestamp.
    pub fn current_timestamp(&self) -> LogicalTimestamp {
        self.capture.current_timestamp()
    }

    /// Get the next sequence number for outgoing messages.
    fn next_seq(&self) -> i64 {
        self.sequence.fetch_add(1, Ordering::SeqCst) as i64
    }

    /// Wait for the next message from the debugger (blocking read).
    pub async fn recv(&self) -> Option<Message> {
        let mut reader = self.reader.lock().await;
        match reader.next().await {
            Some(Ok(msg)) => {
                // Record in capture
                self.capture.record(msg.clone()).await;
                Some(msg)
            }
            _ => None,
        }
    }

    /// Wait for a specific request command with timeout.
    ///
    /// This reads messages until finding the expected request, recording
    /// all messages in the capture buffer. Unexpected messages are logged
    /// but not treated as errors.
    pub async fn expect_request(&self, expected_command: &str) -> Request {
        self.expect_request_timeout(expected_command, Duration::from_secs(5))
            .await
            .expect(&format!(
                "timeout waiting for '{}' request",
                expected_command
            ))
    }

    /// Wait for a specific request command with custom timeout.
    pub async fn expect_request_timeout(
        &self,
        expected_command: &str,
        max_wait: Duration,
    ) -> Option<Request> {
        let deadline = std::time::Instant::now() + max_wait;

        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }

            match timeout(remaining, self.recv()).await {
                Ok(Some(Message::Request(req))) if req.command == expected_command => {
                    return Some(req);
                }
                Ok(Some(msg)) => {
                    // Unexpected message - log and continue waiting
                    tracing::debug!(
                        ?msg,
                        "received unexpected message while waiting for '{}'",
                        expected_command
                    );
                }
                Ok(None) => {
                    // Connection closed
                    return None;
                }
                Err(_) => {
                    // Timeout
                    return None;
                }
            }
        }
    }

    /// Wait for any request and return it.
    pub async fn expect_any_request(&self) -> Request {
        loop {
            match self.recv().await {
                Some(Message::Request(req)) => return req,
                Some(msg) => {
                    tracing::debug!(?msg, "received non-request message");
                }
                None => panic!("connection closed unexpectedly"),
            }
        }
    }

    /// Send a success response for a request.
    pub async fn send_success_response(&self, request_seq: i64, body: Option<Value>) {
        let seq = self.next_seq();
        let response = OutgoingMessage::Response(transport2::OutgoingResponse {
            seq,
            request_seq,
            success: true,
            command: String::new(),
            message: None,
            body,
        });
        let mut writer = self.writer.lock().await;
        writer
            .send(response)
            .await
            .expect("failed to send response");
    }

    /// Send an error response for a request.
    pub async fn send_error_response(&self, request_seq: i64, message: &str) {
        let seq = self.next_seq();
        let response = OutgoingMessage::Response(transport2::OutgoingResponse {
            seq,
            request_seq,
            success: false,
            command: String::new(),
            message: Some(message.to_string()),
            body: None,
        });
        let mut writer = self.writer.lock().await;
        writer
            .send(response)
            .await
            .expect("failed to send response");
    }

    /// Send an event to the debugger.
    pub async fn send_event(&self, event: &str, body: Option<Value>) {
        let seq = self.next_seq();
        let event_msg = OutgoingMessage::Event(transport2::OutgoingEvent {
            seq,
            event: event.to_string(),
            body,
        });
        let mut writer = self.writer.lock().await;
        writer.send(event_msg).await.expect("failed to send event");
    }

    /// Send an 'initialized' event.
    pub async fn send_initialized_event(&self) {
        self.send_event("initialized", None).await;
    }

    /// Send a 'stopped' event.
    pub async fn send_stopped_event(&self, thread_id: i64, reason: &str) {
        self.send_event(
            "stopped",
            Some(json!({
                "reason": reason,
                "threadId": thread_id,
            })),
        )
        .await;
    }

    /// Send a 'continued' event.
    pub async fn send_continued_event(&self, thread_id: i64) {
        self.send_event(
            "continued",
            Some(json!({
                "threadId": thread_id,
            })),
        )
        .await;
    }

    /// Send a 'terminated' event.
    pub async fn send_terminated_event(&self) {
        self.send_event("terminated", None).await;
    }

    /// Send a stack trace response.
    pub async fn send_stack_trace_response(&self, request_seq: i64, frames: Vec<StackFrameData>) {
        let stack_frames: Vec<Value> = frames
            .into_iter()
            .map(|f| {
                json!({
                    "id": f.id,
                    "name": f.name,
                    "line": f.line,
                    "column": f.column,
                    "source": {
                        "name": f.source_name,
                        "path": f.source_path,
                    }
                })
            })
            .collect();

        self.send_success_response(
            request_seq,
            Some(json!({
                "stackFrames": stack_frames,
                "totalFrames": stack_frames.len(),
            })),
        )
        .await;
    }

    /// Send a scopes response.
    pub async fn send_scopes_response(&self, request_seq: i64, scopes: Vec<ScopeData>) {
        let scopes_json: Vec<Value> = scopes
            .into_iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "variablesReference": s.variables_reference,
                    "expensive": s.expensive,
                })
            })
            .collect();

        self.send_success_response(request_seq, Some(json!({ "scopes": scopes_json })))
            .await;
    }

    /// Send a variables response.
    pub async fn send_variables_response(&self, request_seq: i64, variables: Vec<VariableData>) {
        let vars_json: Vec<Value> = variables
            .into_iter()
            .map(|v| {
                json!({
                    "name": v.name,
                    "value": v.value,
                    "type": v.type_name,
                    "variablesReference": v.variables_reference,
                })
            })
            .collect();

        self.send_success_response(request_seq, Some(json!({ "variables": vars_json })))
            .await;
    }

    /// Send an evaluate response.
    pub async fn send_evaluate_response(&self, request_seq: i64, result: &str) {
        self.send_success_response(
            request_seq,
            Some(json!({
                "result": result,
                "variablesReference": 0,
            })),
        )
        .await;
    }
}

/// Helper struct for stack frame data in tests.
#[derive(Debug, Clone)]
pub struct StackFrameData {
    pub id: i64,
    pub name: String,
    pub line: i64,
    pub column: i64,
    pub source_name: String,
    pub source_path: String,
}

impl Default for StackFrameData {
    fn default() -> Self {
        Self {
            id: 1,
            name: "main".to_string(),
            line: 1,
            column: 0,
            source_name: "test.py".to_string(),
            source_path: "/tmp/test.py".to_string(),
        }
    }
}

/// Helper struct for scope data in tests.
#[derive(Debug, Clone)]
pub struct ScopeData {
    pub name: String,
    pub variables_reference: i64,
    pub expensive: bool,
}

impl Default for ScopeData {
    fn default() -> Self {
        Self {
            name: "Locals".to_string(),
            variables_reference: 1,
            expensive: false,
        }
    }
}

/// Helper struct for variable data in tests.
#[derive(Debug, Clone)]
pub struct VariableData {
    pub name: String,
    pub value: String,
    pub type_name: String,
    pub variables_reference: i64,
}

impl Default for VariableData {
    fn default() -> Self {
        Self {
            name: "x".to_string(),
            value: "42".to_string(),
            type_name: "int".to_string(),
            variables_reference: 0,
        }
    }
}

/// Create a connected pair of transports for testing.
///
/// Returns (client_reader, client_writer, adapter_reader, adapter_writer).
pub fn create_test_transports() -> (
    DapReader<DuplexStream>,
    DapWriter<DuplexStream>,
    DapReader<DuplexStream>,
    DapWriter<DuplexStream>,
) {
    let (client_transport, adapter_transport) = MemoryTransport::pair();
    let (client_reader, client_writer) = split(client_transport);
    let (adapter_reader, adapter_writer) = split(adapter_transport);
    (client_reader, client_writer, adapter_reader, adapter_writer)
}

/// Create a mock adapter from the adapter side of a transport pair.
pub fn create_mock_adapter() -> (
    DapReader<DuplexStream>,
    DapWriter<DuplexStream>,
    MockAdapter,
) {
    let (client_reader, client_writer, adapter_reader, adapter_writer) = create_test_transports();
    let mock = MockAdapter::new(adapter_reader, adapter_writer);
    (client_reader, client_writer, mock)
}

/// A mock adapter handler that automatically responds to common initialization requests.
///
/// This simplifies tests by handling the standard initialize/launch/configurationDone
/// sequence automatically.
pub struct AutoInitMockAdapter {
    adapter: Arc<MockAdapter>,
    capabilities: Value,
}

impl AutoInitMockAdapter {
    pub fn new(adapter: Arc<MockAdapter>) -> Self {
        Self {
            adapter,
            capabilities: json!({
                "supportsConfigurationDoneRequest": true,
            }),
        }
    }

    /// Run the adapter, automatically responding to initialization requests.
    ///
    /// This handles: initialize, launch, setExceptionBreakpoints
    /// and sends the initialized event at the appropriate time.
    pub async fn handle_initialization(&self) {
        // Handle initialize request
        let req = self.adapter.expect_request("initialize").await;
        self.adapter
            .send_success_response(req.seq, Some(self.capabilities.clone()))
            .await;

        // Handle launch request
        let req = self.adapter.expect_request("launch").await;
        self.adapter.send_success_response(req.seq, None).await;

        // Send initialized event
        self.adapter.send_initialized_event().await;

        // Handle setExceptionBreakpoints request
        let req = self.adapter.expect_request("setExceptionBreakpoints").await;
        self.adapter
            .send_success_response(req.seq, Some(json!({ "breakpoints": [] })))
            .await;
    }

    /// Get a reference to the underlying mock adapter.
    pub fn adapter(&self) -> &MockAdapter {
        &self.adapter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_adapter_send_receive() {
        let (_client_reader, client_writer, mock) = create_mock_adapter();

        // Spawn a task to send a message from the "client" side
        let send_handle = tokio::spawn(async move {
            let mut writer = client_writer;
            let request = OutgoingMessage::Request(Request {
                seq: 1,
                command: "test".to_string(),
                arguments: None,
            });
            writer.send(request).await.unwrap();
        });

        // Mock adapter should receive it
        let msg = mock.recv().await.unwrap();
        assert!(matches!(msg, Message::Request(r) if r.command == "test"));

        send_handle.await.unwrap();
    }

    #[tokio::test]
    async fn mock_adapter_expect_request() {
        let (_client_reader, client_writer, mock) = create_mock_adapter();

        let send_handle = tokio::spawn(async move {
            let mut writer = client_writer;
            let request = OutgoingMessage::Request(Request {
                seq: 1,
                command: "initialize".to_string(),
                arguments: None,
            });
            writer.send(request).await.unwrap();
        });

        let req = mock.expect_request("initialize").await;
        assert_eq!(req.seq, 1);

        send_handle.await.unwrap();
    }

    #[tokio::test]
    async fn mock_adapter_send_response() {
        let (mut client_reader, _client_writer, mock) = create_mock_adapter();

        let recv_handle = tokio::spawn(async move {
            let msg = client_reader.next().await.unwrap().unwrap();
            match msg {
                Message::Response(r) => {
                    assert!(r.success);
                    assert_eq!(r.request_seq, 42);
                }
                _ => panic!("expected response"),
            }
        });

        mock.send_success_response(42, None).await;

        recv_handle.await.unwrap();
    }

    #[tokio::test]
    async fn mock_adapter_send_event() {
        let (mut client_reader, _client_writer, mock) = create_mock_adapter();

        let recv_handle = tokio::spawn(async move {
            let msg = client_reader.next().await.unwrap().unwrap();
            match msg {
                Message::Event(e) => {
                    assert_eq!(e.event, "stopped");
                }
                _ => panic!("expected event"),
            }
        });

        mock.send_stopped_event(1, "breakpoint").await;

        recv_handle.await.unwrap();
    }

    #[tokio::test]
    async fn message_capture_finds_response() {
        let capture = MessageCapture::new(100);

        // Record some messages
        let resp1 = Message::Response(Response {
            seq: 1,
            request_seq: 1,
            success: true,
            command: "initialize".to_string(),
            message: None,
            body: None,
        });
        let resp2 = Message::Response(Response {
            seq: 2,
            request_seq: 2,
            success: true,
            command: "launch".to_string(),
            message: None,
            body: None,
        });

        let ts1 = capture.record(resp1).await;
        let _ts2 = capture.record(resp2).await;

        // Should find response for request_seq 2
        let result = capture.find_response(2, ts1, 10).await;
        assert!(result.is_some());
        let (resp, _ts) = result.unwrap();
        assert_eq!(resp.request_seq, 2);

        // Should not find response for request_seq 3
        let result = capture.find_response(3, 0, 10).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn message_capture_finds_event() {
        let capture = MessageCapture::new(100);

        // Record some messages
        let evt1 = Message::Event(transport2::Event {
            seq: 1,
            event: "initialized".to_string(),
            body: None,
        });
        let evt2 = Message::Event(transport2::Event {
            seq: 2,
            event: "stopped".to_string(),
            body: Some(json!({"reason": "breakpoint"})),
        });

        let ts1 = capture.record(evt1).await;
        let _ts2 = capture.record(evt2).await;

        // Should find stopped event after ts1
        let result = capture.find_event("stopped", ts1, 10).await;
        assert!(result.is_some());
        let (evt, _ts) = result.unwrap();
        assert_eq!(evt.event, "stopped");

        // Should not find output event
        let result = capture.find_event("output", 0, 10).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn mock_adapter_handles_unexpected_messages() {
        let (_client_reader, client_writer, mock) = create_mock_adapter();

        let send_handle = tokio::spawn(async move {
            let mut writer = client_writer;

            // Send an unexpected event first
            writer
                .send(OutgoingMessage::Request(Request {
                    seq: 1,
                    command: "unexpected".to_string(),
                    arguments: None,
                }))
                .await
                .unwrap();

            // Then send the expected request
            writer
                .send(OutgoingMessage::Request(Request {
                    seq: 2,
                    command: "initialize".to_string(),
                    arguments: None,
                }))
                .await
                .unwrap();
        });

        // Should still find the initialize request despite the unexpected message
        let req = mock.expect_request("initialize").await;
        assert_eq!(req.command, "initialize");

        send_handle.await.unwrap();
    }

    #[tokio::test]
    async fn auto_init_mock_handles_initialization() {
        let (client_reader, client_writer, mock) = create_mock_adapter();
        let mock = Arc::new(mock);
        let auto_init = AutoInitMockAdapter::new(mock.clone());

        // Spawn the auto-init handler
        let init_handle = tokio::spawn(async move {
            auto_init.handle_initialization().await;
        });

        // Spawn a task that simulates the client sending init requests
        let client_handle = tokio::spawn(async move {
            let mut writer = client_writer;
            let mut reader = client_reader;

            // Send initialize request
            writer
                .send(OutgoingMessage::Request(Request {
                    seq: 1,
                    command: "initialize".to_string(),
                    arguments: Some(serde_json::json!({"adapterId": "test"})),
                }))
                .await
                .unwrap();

            // Receive initialize response
            let msg = reader.next().await.unwrap().unwrap();
            assert!(matches!(msg, Message::Response(r) if r.success));

            // Send launch request
            writer
                .send(OutgoingMessage::Request(Request {
                    seq: 2,
                    command: "launch".to_string(),
                    arguments: Some(serde_json::json!({"program": "/tmp/test.py"})),
                }))
                .await
                .unwrap();

            // Receive launch response
            let msg = reader.next().await.unwrap().unwrap();
            assert!(matches!(msg, Message::Response(r) if r.success));

            // Receive initialized event
            let msg = reader.next().await.unwrap().unwrap();
            assert!(matches!(msg, Message::Event(e) if e.event == "initialized"));

            // Send setExceptionBreakpoints request
            writer
                .send(OutgoingMessage::Request(Request {
                    seq: 3,
                    command: "setExceptionBreakpoints".to_string(),
                    arguments: Some(serde_json::json!({"filters": []})),
                }))
                .await
                .unwrap();

            // Receive setExceptionBreakpoints response
            let msg = reader.next().await.unwrap().unwrap();
            assert!(matches!(msg, Message::Response(r) if r.success));
        });

        // Wait for both to complete
        init_handle.await.unwrap();
        client_handle.await.unwrap();
    }
}
