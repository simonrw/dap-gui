//! Integration tests for in-memory transport
//!
//! These tests verify that the in-memory transport layer works correctly
//! for testing scenarios without requiring actual TCP connections.

use std::io::Write;
use std::thread;

use eyre::Result;
use transport::{
    Client, Reader, events,
    io::{DapTransport, InMemoryTransport},
    requests, responses,
};

#[test]
fn test_in_memory_basic_request_response() -> Result<()> {
    // Create connected in-memory transports
    let (client_transport, server_transport) = InMemoryTransport::pair();

    // Create client with in-memory transport
    let (event_tx, event_rx) = crossbeam_channel::unbounded();
    let client = Client::with_transport(client_transport, event_tx)?;

    // Spawn mock server thread
    let server_handle = thread::spawn(move || {
        mock_server(server_transport).unwrap();
    });

    // Send initialize request
    let req = requests::RequestBody::Initialize(requests::Initialize {
        adapter_id: "test".to_string(),
        lines_start_at_one: true,
        path_format: requests::PathFormat::Path,
        supports_start_debugging_request: false,
        supports_variable_type: false,
        supports_variable_paging: false,
        supports_progress_reporting: false,
        supports_memory_event: false,
    });

    let response = client.send(req)?;
    assert!(response.success, "Initialize request should succeed");

    // Wait for initialized event
    let event = event_rx.recv_timeout(std::time::Duration::from_secs(5))?;
    assert!(
        matches!(event, events::Event::Initialized { .. }),
        "Should receive Initialized event"
    );

    // Send disconnect to clean up
    let req = requests::RequestBody::Disconnect(requests::Disconnect {
        terminate_debugee: false,
    });
    let response = client.send(req)?;
    assert!(response.success, "Disconnect request should succeed");

    // Clean up server thread
    drop(client);
    server_handle.join().unwrap();

    Ok(())
}

#[test]
fn test_in_memory_multiple_requests() -> Result<()> {
    let (client_transport, server_transport) = InMemoryTransport::pair();

    let (event_tx, _event_rx) = crossbeam_channel::unbounded();
    let client = Client::with_transport(client_transport, event_tx)?;

    let server_handle = thread::spawn(move || {
        mock_server_multiple_requests(server_transport).unwrap();
    });

    // Send multiple requests in sequence
    for i in 0..5 {
        let req = requests::RequestBody::Threads;
        let response = client.send(req)?;
        assert!(response.success, "Request {} should succeed", i);
    }

    drop(client);
    server_handle.join().unwrap();

    Ok(())
}

/// Mock DAP server that handles basic requests
fn mock_server(transport: InMemoryTransport) -> Result<()> {
    let (reader, mut writer) = transport.split()?;
    let mut reader = transport::reader::get(reader);

    // Read and respond to messages using the DAP reader
    loop {
        match reader.poll_message() {
            Ok(Some(transport::Message::Request(request))) => {
                // Handle different request types
                match request.body {
                    requests::RequestBody::Initialize(_) => {
                        // Send success response
                        send_response(
                            &mut writer,
                            request.seq,
                            responses::ResponseBody::Initialize(responses::Capabilities {
                                supports_configuration_done_request: Some(true),
                                supports_function_breakpoints: Some(true),
                                supports_conditional_breakpoints: Some(true),
                                supports_hit_conditional_breakpoints: Some(false),
                                supports_evaluate_for_hovers: Some(true),
                                completion_trigger_characters: None,
                                supports_step_back: Some(false),
                                supports_set_variable: Some(true),
                                supports_restart_frame: Some(false),
                                supports_goto_targets_request: Some(false),
                                supports_step_in_targets_request: Some(false),
                                supports_completions_request: Some(true),
                                supports_modules_request: Some(true),
                                supports_restart_request: Some(true),
                                supports_exception_options: Some(true),
                                supports_value_formatting_options: Some(true),
                                supports_exception_info_request: Some(true),
                                support_terminate_debuggee: Some(true),
                                support_suspend_debuggee: None,
                                supports_delayed_stack_trace_loading: Some(true),
                                supports_loaded_sources_request: Some(false),
                                supports_log_points: Some(true),
                                supports_terminate_threads_request: Some(false),
                                supports_set_expression: Some(true),
                                supports_terminate_request: Some(true),
                                supports_data_breakpoints: Some(false),
                                supports_read_memory_request: Some(false),
                                supports_write_memory_request: None,
                                supports_disassemble_request: Some(false),
                                supports_cancel_request: Some(false),
                                supports_breakpoint_locations_request: None,
                                supports_clipboard_context: Some(false),
                                supports_stepping_granularity: Some(false),
                                supports_instruction_breakpoints: Some(false),
                                supports_exception_filter_options: Some(true),
                                supports_single_thread_execution_requests: None,
                            }),
                        )?;

                        // Send initialized event
                        send_event(&mut writer, events::Event::Initialized {})?;
                    }
                    requests::RequestBody::Disconnect(_) => {
                        send_response(
                            &mut writer,
                            request.seq,
                            responses::ResponseBody::Disconnect,
                        )?;
                        break;
                    }
                    _ => {
                        // Send generic success response
                        send_empty_response(&mut writer, request.seq)?;
                    }
                }
            }
            Ok(Some(_)) => {
                // Ignore other message types (events, responses)
            }
            Ok(None) => {
                // EOF
                break;
            }
            Err(e) => {
                // Log the error but continue - might be WouldBlock
                let err_str = e.to_string();
                if err_str.contains("WouldBlock") {
                    // Sleep a bit to avoid busy waiting
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                eprintln!("Mock server error: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Mock server for multiple requests test
fn mock_server_multiple_requests(transport: InMemoryTransport) -> Result<()> {
    let (reader, mut writer) = transport.split()?;
    let mut reader = transport::reader::get(reader);

    loop {
        match reader.poll_message() {
            Ok(Some(transport::Message::Request(request))) => {
                // Just send success for all requests
                match request.body {
                    requests::RequestBody::Threads => {
                        send_response(
                            &mut writer,
                            request.seq,
                            responses::ResponseBody::Threads(responses::ThreadsResponse {
                                threads: vec![],
                            }),
                        )?;
                    }
                    _ => {
                        send_empty_response(&mut writer, request.seq)?;
                    }
                }
            }
            Ok(Some(_)) => {
                // Ignore other message types
            }
            Ok(None) => break,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("WouldBlock") {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                return Err(e);
            }
        }
    }

    Ok(())
}

fn send_response(
    writer: &mut dyn Write,
    request_seq: i64,
    body: responses::ResponseBody,
) -> Result<()> {
    let response = responses::Response {
        request_seq,
        success: true,
        message: None,
        body: Some(body),
    };

    // Create the full message structure
    let message = transport::Message::Response(response);
    let json = serde_json::to_string(&message)?;

    write!(writer, "Content-Length: {}\r\n\r\n{}", json.len(), json)?;
    writer.flush()?;
    Ok(())
}

fn send_empty_response(writer: &mut dyn Write, request_seq: i64) -> Result<()> {
    // For empty responses, we can just send a generic success with no body
    // The command field will be missing but that's OK for testing
    let json = serde_json::to_string(&serde_json::json!({
        "type": "response",
        "seq": 1,
        "request_seq": request_seq,
        "success": true,
    }))?;

    write!(writer, "Content-Length: {}\r\n\r\n{}", json.len(), json)?;
    writer.flush()?;
    Ok(())
}

fn send_event(writer: &mut dyn Write, event: events::Event) -> Result<()> {
    let message = transport::Message::Event(event);
    let json = serde_json::to_string(&message)?;

    write!(writer, "Content-Length: {}\r\n\r\n{}", json.len(), json)?;
    writer.flush()?;
    Ok(())
}
