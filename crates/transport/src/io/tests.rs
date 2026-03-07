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
        client_id: None,
        client_name: None,
        columns_start_at1: None,
        lines_start_at1: Some(true),
        locale: None,
        path_format: Some("path".to_string()),
        supports_ansi_styling: None,
        supports_args_can_be_interpreted_by_shell: None,
        supports_invalidated_event: None,
        supports_memory_event: Some(false),
        supports_memory_references: None,
        supports_progress_reporting: Some(false),
        supports_run_in_terminal_request: None,
        supports_start_debugging_request: Some(false),
        supports_variable_paging: Some(false),
        supports_variable_type: Some(false),
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
                ..Default::default()
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
        single_thread: Some(false),
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
