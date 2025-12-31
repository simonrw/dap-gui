//! Tests for transport implementations

use super::*;
use crate::{Message, Reader, SyncTransport, TransportConnection, requests, responses};

#[test]
fn test_tcp_transport_implements_trait() {
    // Compile-time verification that TcpTransport implements DapTransport
    fn _assert_impl<T: DapTransport>() {}
    _assert_impl::<TcpTransport>();
}

#[test]
fn test_memory_transport_implements_trait() {
    // Compile-time verification that InMemoryTransport implements DapTransport
    fn _assert_impl<T: DapTransport>() {}
    _assert_impl::<InMemoryTransport>();
}

#[test]
fn test_transport_connection_with_memory_transport() {
    // Create a pair of in-memory transports
    let (client_transport, _server_transport) = InMemoryTransport::pair();

    // Create a TransportConnection from the in-memory transport
    let conn = TransportConnection::with_transport(client_transport);
    assert!(
        conn.is_ok(),
        "Should create TransportConnection from InMemoryTransport"
    );
}

#[test]
fn test_sync_transport_send_and_receive() {
    use std::io::Write;

    // Create a pair of in-memory transports
    let (client_transport, server_transport) = InMemoryTransport::pair();

    // Create client connection
    let mut client = TransportConnection::with_transport(client_transport)
        .expect("should create client connection");

    // Get server reader/writer
    let (server_reader, mut server_writer) = server_transport
        .split()
        .expect("should split server transport");

    // Client sends a request
    let request_body = requests::RequestBody::Initialize(requests::Initialize {
        adapter_id: "test".to_string(),
        lines_start_at_one: true,
        path_format: requests::PathFormat::Path,
        supports_start_debugging_request: false,
        supports_variable_type: false,
        supports_variable_paging: false,
        supports_progress_reporting: false,
        supports_memory_event: false,
    });

    let seq = client
        .send_request(request_body.clone())
        .expect("should send request");

    assert_eq!(seq, 1, "First request should have sequence number 1");

    // Server reads the request
    let mut reader = crate::reader::get(server_reader);
    let message = reader
        .poll_message()
        .expect("should read message")
        .expect("should have message");

    match message {
        Message::Request(req) => {
            assert_eq!(req.seq, 1);
            assert!(matches!(req.body, requests::RequestBody::Initialize(_)));
        }
        _ => panic!("Expected Request message"),
    }

    // Server sends a response
    let response = responses::Response {
        request_seq: seq,
        success: true,
        message: None,
        body: Some(responses::ResponseBody::Initialize(
            responses::Capabilities {
                supports_configuration_done_request: Some(true),
                supports_function_breakpoints: Some(false),
                supports_conditional_breakpoints: Some(false),
                supports_hit_conditional_breakpoints: Some(false),
                supports_evaluate_for_hovers: Some(false),
                supports_step_back: Some(false),
                supports_set_variable: Some(false),
                supports_restart_frame: Some(false),
                supports_goto_targets_request: Some(false),
                supports_step_in_targets_request: Some(false),
                supports_completions_request: Some(false),
                completion_trigger_characters: None,
                supports_modules_request: Some(false),
                supports_restart_request: Some(false),
                supports_exception_options: Some(false),
                supports_value_formatting_options: Some(false),
                supports_exception_info_request: Some(false),
                support_terminate_debuggee: Some(false),
                support_suspend_debuggee: Some(false),
                supports_delayed_stack_trace_loading: Some(false),
                supports_loaded_sources_request: Some(false),
                supports_log_points: Some(false),
                supports_terminate_threads_request: Some(false),
                supports_set_expression: Some(false),
                supports_terminate_request: Some(false),
                supports_data_breakpoints: Some(false),
                supports_read_memory_request: Some(false),
                supports_write_memory_request: Some(false),
                supports_disassemble_request: Some(false),
                supports_cancel_request: Some(false),
                supports_breakpoint_locations_request: Some(false),
                supports_clipboard_context: Some(false),
                supports_stepping_granularity: Some(false),
                supports_instruction_breakpoints: Some(false),
                supports_exception_filter_options: Some(false),
                supports_single_thread_execution_requests: Some(false),
            },
        )),
    };

    // Wrap response in Message::Response to include the "type" field
    let message = Message::Response(response);
    let response_json = serde_json::to_string(&message).expect("should serialize");
    write!(
        server_writer,
        "Content-Length: {}\r\n\r\n{}",
        response_json.len(),
        response_json
    )
    .expect("should write response");
    server_writer.flush().expect("should flush");

    // Client receives the response
    let received = client
        .receive_message()
        .expect("should receive message")
        .expect("should have message");

    match received {
        Message::Response(resp) => {
            assert_eq!(resp.request_seq, seq);
            assert!(resp.success);
        }
        _ => panic!("Expected Response message"),
    }
}

#[test]
fn test_sync_transport_execute() {
    // Create a pair of in-memory transports
    let (client_transport, server_transport) = InMemoryTransport::pair();

    // Create client connection
    let mut client = TransportConnection::with_transport(client_transport)
        .expect("should create client connection");

    // Get server reader
    let (server_reader, _server_writer) = server_transport
        .split()
        .expect("should split server transport");

    // Client sends an execute (fire-and-forget) request
    let request_body = requests::RequestBody::Continue(requests::Continue {
        thread_id: 1,
        single_thread: false,
    });

    client
        .send_execute(request_body)
        .expect("should send execute");

    // Server reads the request
    let mut reader = crate::reader::get(server_reader);
    let message = reader
        .poll_message()
        .expect("should read message")
        .expect("should have message");

    match message {
        Message::Request(req) => {
            assert_eq!(req.seq, 1);
            assert!(matches!(req.body, requests::RequestBody::Continue(_)));
        }
        _ => panic!("Expected Request message"),
    }
}

#[test]
fn test_sync_transport_multiple_requests() {
    // Create a pair of in-memory transports
    let (client_transport, server_transport) = InMemoryTransport::pair();

    // Create client connection
    let mut client = TransportConnection::with_transport(client_transport)
        .expect("should create client connection");

    // Get server reader
    let (server_reader, _server_writer) = server_transport
        .split()
        .expect("should split server transport");

    // Send multiple requests
    let seq1 = client
        .send_request(requests::RequestBody::Threads)
        .expect("should send first request");
    let seq2 = client
        .send_request(requests::RequestBody::Threads)
        .expect("should send second request");
    let seq3 = client
        .send_request(requests::RequestBody::Threads)
        .expect("should send third request");

    assert_eq!(seq1, 1);
    assert_eq!(seq2, 2);
    assert_eq!(seq3, 3);

    // Server reads all requests
    let mut reader = crate::reader::get(server_reader);

    for expected_seq in 1..=3 {
        let message = reader
            .poll_message()
            .expect("should read message")
            .expect("should have message");

        match message {
            Message::Request(req) => {
                assert_eq!(req.seq, expected_seq);
            }
            _ => panic!("Expected Request message"),
        }
    }
}
