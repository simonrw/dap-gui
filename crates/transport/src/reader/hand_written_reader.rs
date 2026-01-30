use std::io::{self, BufRead};
use std::time::{Duration, Instant};

use eyre::WrapErr;

use crate::Reader;

/// Persistent state for parsing a DAP message
///
/// This allows `try_poll_message` to resume parsing after a timeout
/// without losing progress.
#[derive(Debug, Default)]
struct ParseState {
    /// Current parsing phase
    phase: ParsePhase,
    /// Partial line buffer for Header/blank-line reads, preserved across timeouts
    line_buffer: Vec<u8>,
    /// Content length from the header (valid when phase is Content or ContentReading)
    content_length: usize,
    /// Buffer for content bytes being read
    content_buffer: Vec<u8>,
    /// Number of content bytes read so far
    content_bytes_read: usize,
}

/// Parsing phase for DAP messages
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ParsePhase {
    /// Waiting for Content-Length header
    #[default]
    Header,
    /// Read the blank line after header, then read content
    Content,
    /// Actively reading content bytes (may be partially complete)
    ContentReading,
}

impl ParseState {
    fn reset(&mut self) {
        self.phase = ParsePhase::Header;
        self.line_buffer.clear();
        self.content_length = 0;
        self.content_buffer.clear();
        self.content_bytes_read = 0;
    }
}

pub struct HandWrittenReader<R> {
    input: R,
    /// Persistent state for try_poll_message to handle partial reads
    parse_state: ParseState,
}

/// Result of a timeout-aware poll operation
#[derive(Debug)]
pub enum PollResult {
    /// A message was successfully received
    Message(Box<crate::Message>),
    /// The connection was closed
    Closed,
    /// The timeout expired before a complete message was received
    Timeout,
}

enum ReaderState {
    Header,
    Content,
}

impl<R> Reader<R> for HandWrittenReader<R>
where
    R: BufRead,
{
    fn new(input: R) -> Self {
        Self {
            input,
            parse_state: ParseState::default(),
        }
    }

    fn poll_message(&mut self) -> eyre::Result<Option<crate::Message>> {
        let mut state = ReaderState::Header;
        let mut buffer = String::new();
        let mut content_length: usize = 0;

        loop {
            match self.input.read_line(&mut buffer) {
                Ok(read_size) => {
                    if read_size == 0 {
                        return Ok(None);
                    }

                    match state {
                        ReaderState::Header => {
                            let parts: Vec<&str> = buffer.trim_end().split(':').collect();
                            match parts[0] {
                                "Content-Length" => {
                                    content_length = match parts[1].trim().parse() {
                                        Ok(val) => val,
                                        Err(_) => {
                                            eyre::bail!("failed to parse content length")
                                        }
                                    };
                                    buffer.clear();
                                    buffer.reserve(content_length);
                                    state = ReaderState::Content;
                                }
                                other => {
                                    eyre::bail!("header {} not implemented", other);
                                }
                            }
                        }
                        ReaderState::Content => {
                            buffer.clear();
                            let mut content = vec![0; content_length];
                            self.input
                                .read_exact(content.as_mut_slice())
                                .map_err(|e| eyre::eyre!("failed to read: {:?}", e))?;
                            let content =
                                std::str::from_utf8(content.as_slice()).context("invalid utf8")?;
                            tracing::debug!(content, "received raw message");
                            let message = serde_json::from_str(content).with_context(|| {
                                format!("could not construct message from: {content}")
                            })?;
                            return Ok(Some(message));
                        }
                    }
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        // Read timeout expired with no data available.
                        // Sleep briefly to prevent potential CPU spinning in edge cases
                        // where the timeout might not be properly enforced.
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    return Err(eyre::eyre!("error reading from buffer: {e:?}"));
                }
            }
        }
    }
}

impl<R> HandWrittenReader<R>
where
    R: BufRead,
{
    /// Attempt to receive a message with a timeout
    ///
    /// This method polls for a message and returns after the timeout expires
    /// if no complete message has been received. Unlike `poll_message`, this
    /// method will not block indefinitely.
    ///
    /// **Important**: This method maintains persistent state between calls.
    /// If a timeout occurs mid-read, the next call will resume from where it
    /// left off. This allows safe use in a polling loop without losing data.
    ///
    /// Returns:
    /// - `Ok(PollResult::Message(msg))` if a complete message was received
    /// - `Ok(PollResult::Closed)` if the connection was closed
    /// - `Ok(PollResult::Timeout)` if the timeout expired
    /// - `Err(_)` if an error occurred
    ///
    /// Note: The actual timeout resolution depends on the underlying transport's
    /// read timeout. For TCP streams with a 1-second read timeout, the actual
    /// wait time may be up to 1 second longer than the specified timeout.
    pub fn try_poll_message(&mut self, timeout: Duration) -> eyre::Result<PollResult> {
        let start = Instant::now();

        loop {
            // Check timeout before each operation
            if start.elapsed() >= timeout {
                return Ok(PollResult::Timeout);
            }

            match self.parse_state.phase {
                ParsePhase::Header => {
                    // Try to read a header line
                    match self
                        .input
                        .read_until(b'\n', &mut self.parse_state.line_buffer)
                    {
                        Ok(0) => {
                            self.parse_state.reset();
                            return Ok(PollResult::Closed);
                        }
                        Ok(_) => {
                            let line = std::str::from_utf8(&self.parse_state.line_buffer)
                                .context("invalid utf8 in header")?;
                            let line = line.trim_end_matches(['\r', '\n']);
                            let (name, value) = line
                                .split_once(':')
                                .ok_or_else(|| eyre::eyre!("malformed header line: {}", line))?;
                            let error_name = name.to_string();
                            match name {
                                "Content-Length" => {
                                    let content_length = match value.trim().parse() {
                                        Ok(val) => val,
                                        Err(_) => {
                                            self.parse_state.reset();
                                            eyre::bail!("failed to parse content length")
                                        }
                                    };
                                    self.parse_state.content_length = content_length;
                                    self.parse_state.content_buffer = vec![0u8; content_length];
                                    self.parse_state.content_bytes_read = 0;
                                    self.parse_state.phase = ParsePhase::Content;
                                    self.parse_state.line_buffer.clear();
                                }
                                _ => {
                                    self.parse_state.reset();
                                    eyre::bail!("header {} not implemented", error_name);
                                }
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            if start.elapsed() >= timeout {
                                return Ok(PollResult::Timeout);
                            }
                            std::thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        Err(e) => {
                            self.parse_state.reset();
                            return Err(eyre::eyre!("error reading header: {e:?}"));
                        }
                    }
                }
                ParsePhase::Content => {
                    // Read the blank line between header and content
                    match self
                        .input
                        .read_until(b'\n', &mut self.parse_state.line_buffer)
                    {
                        Ok(0) => {
                            self.parse_state.reset();
                            return Ok(PollResult::Closed);
                        }
                        Ok(_) => {
                            // Optional: validate it's actually the CRLF separator
                            // (or extend Header parsing to consume additional headers like Content-Type)
                            self.parse_state.phase = ParsePhase::ContentReading;
                            self.parse_state.line_buffer.clear();
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            if start.elapsed() >= timeout {
                                return Ok(PollResult::Timeout);
                            }
                            std::thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        Err(e) => {
                            self.parse_state.reset();
                            return Err(eyre::eyre!("error reading blank line: {e:?}"));
                        }
                    }
                }
                ParsePhase::ContentReading => {
                    // Read content bytes incrementally
                    let remaining =
                        self.parse_state.content_length - self.parse_state.content_bytes_read;
                    if remaining == 0 {
                        // All bytes read, parse the message
                        let content = std::str::from_utf8(&self.parse_state.content_buffer)
                            .context("invalid utf8")?;
                        tracing::debug!(content, "received raw message");
                        let message = serde_json::from_str(content).with_context(|| {
                            format!("could not construct message from: {content}")
                        })?;
                        self.parse_state.reset();
                        return Ok(PollResult::Message(message));
                    }

                    // Try to read more bytes
                    let start_pos = self.parse_state.content_bytes_read;
                    let buf = &mut self.parse_state.content_buffer[start_pos..];

                    match self.input.read(buf) {
                        Ok(0) => {
                            self.parse_state.reset();
                            return Ok(PollResult::Closed);
                        }
                        Ok(n) => {
                            self.parse_state.content_bytes_read += n;
                            // Continue loop to check if we have all bytes
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            if start.elapsed() >= timeout {
                                // Timeout with partial content - state is preserved
                                return Ok(PollResult::Timeout);
                            }
                            std::thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                        Err(e) => {
                            self.parse_state.reset();
                            return Err(eyre::eyre!("error reading content: {e:?}"));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufReader, Write},
        net::{TcpListener, TcpStream},
    };

    use crate::{Message, Reader, bindings::get_random_tcp_port, events, responses};

    use super::HandWrittenReader;

    macro_rules! execute_test {
        // multiple bodies for single message
        ($($body:expr_2021),+ => $match_expr:pat) => {{
            let port = get_random_tcp_port().expect("getting random port");
            let server =
                TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
            let mut client =
                TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
            let (conn, _) = server.accept().expect("accepting connection");

            let mut reader = HandWrittenReader::new(BufReader::new(conn));

            $(write!(&mut client, "{}", $body).expect("sending message");)+

            let message = reader.poll_message().expect("polling message");

            match message {
                Some(msg) => {
                    assert!(matches!(msg, $match_expr), "Got message {:?}", msg);
                }
                None => eyre::bail!("no message found"),
            }
        }};

        // multiple messages for single body
        ($body:expr_2021 => $($match_expr:pat),+) => {{
            let port = get_random_tcp_port().expect("getting random port");
            let server =
                TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
            let mut client =
                TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
            let (conn, _) = server.accept().expect("accepting connection");

            let mut reader = HandWrittenReader::new(BufReader::new(conn));



            write!(&mut client, "{}", $body).expect("sending message");

            $(

            let message = reader.poll_message().expect("polling message");

            match message {
                Some(msg) => {
                    assert!(matches!(msg, $match_expr));
                }
                None => eyre::bail!("no message found"),
            }

            )+
        }};
    }

    #[test]
    fn single_message() -> eyre::Result<()> {
        let body = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}";

        execute_test!(body => Message::Event(events::Event::Terminated));

        Ok(())
    }

    #[test]
    fn split_between_requests() -> eyre::Result<()> {
        execute_test!(
            "Content-Length: 37\r\n\r\n{\"ty",
            "pe\":\"event\",\"event\":\"terminated\"}" =>
        Message::Event(events::Event::Terminated));

        Ok(())
    }

    #[test]
    fn multiple_messages() -> eyre::Result<()> {
        let body = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}";

        execute_test!(body => Message::Event(events::Event::Terminated), Message::Event(events::Event::Terminated));

        Ok(())
    }

    #[test]
    fn evaluate_error() -> eyre::Result<()> {
        let body = r#"Content-Length: 220"#.to_owned()
            + "\r\n\r\n"
            + r#"{"seq": 21, "type": "response", "request_seq": 13, "success": false, "command": "evaluate", "message": "Traceback (most recent call last):\n  File \"<string>\", line 1, in <module>\nNameError: name 'b' is not defined\n"}"#;

        execute_test!(body => Message::Response(responses::Response {
            message: Some(_),
            success: false,
            ..
        }));

        Ok(())
    }

    #[test]
    fn try_poll_message_timeout() -> eyre::Result<()> {
        use super::PollResult;
        use std::time::Duration;

        let port = get_random_tcp_port().expect("getting random port");
        let server = TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
        let _client =
            TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
        let (conn, _) = server.accept().expect("accepting connection");

        // Set a short read timeout on the connection
        conn.set_read_timeout(Some(Duration::from_millis(10)))
            .expect("setting read timeout");

        let mut reader = HandWrittenReader::new(BufReader::new(conn));

        // Try to receive with a short timeout - should timeout since no data is sent
        let start = std::time::Instant::now();
        let result = reader.try_poll_message(Duration::from_millis(50))?;
        let elapsed = start.elapsed();

        assert!(matches!(result, PollResult::Timeout));
        // Verify it actually waited approximately the timeout duration
        assert!(
            elapsed >= Duration::from_millis(20),
            "elapsed: {:?}",
            elapsed
        );
        assert!(
            elapsed < Duration::from_millis(500),
            "elapsed: {:?}",
            elapsed
        );

        Ok(())
    }

    #[test]
    fn try_poll_message_receives_message() -> eyre::Result<()> {
        use super::PollResult;
        use std::time::Duration;

        let port = get_random_tcp_port().expect("getting random port");
        let server = TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
        let mut client =
            TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
        let (conn, _) = server.accept().expect("accepting connection");

        // Set a read timeout on the connection
        conn.set_read_timeout(Some(Duration::from_millis(100)))
            .expect("setting read timeout");

        let mut reader = HandWrittenReader::new(BufReader::new(conn));

        // Send a message
        let body = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}";
        write!(&mut client, "{}", body).expect("sending message");

        // Try to receive - should get the message
        let result = reader.try_poll_message(Duration::from_secs(1))?;

        match result {
            // PollResult::Message(Message::Event(events::Event::Terminated)) => {}
            PollResult::Message(message)
                if matches!(*message, Message::Event(events::Event::Terminated)) => {}
            other => panic!("unexpected result: {:?}", other),
        }

        Ok(())
    }

    #[test]
    fn try_poll_message_connection_closed() -> eyre::Result<()> {
        use super::PollResult;
        use std::time::Duration;

        let port = get_random_tcp_port().expect("getting random port");
        let server = TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
        let client = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
        let (conn, _) = server.accept().expect("accepting connection");

        // Set a read timeout on the connection
        conn.set_read_timeout(Some(Duration::from_millis(100)))
            .expect("setting read timeout");

        let mut reader = HandWrittenReader::new(BufReader::new(conn));

        // Close the client connection
        drop(client);

        // Try to receive - should get Closed
        let result = reader.try_poll_message(Duration::from_secs(1))?;

        assert!(matches!(result, PollResult::Closed));

        Ok(())
    }

    #[test]
    fn try_poll_message_partial_read_recovery() -> eyre::Result<()> {
        use super::PollResult;
        use std::time::Duration;

        let port = get_random_tcp_port().expect("getting random port");
        let server = TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
        let mut client =
            TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
        let (conn, _) = server.accept().expect("accepting connection");

        // Set a short read timeout on the connection
        conn.set_read_timeout(Some(Duration::from_millis(10)))
            .expect("setting read timeout");

        let mut reader = HandWrittenReader::new(BufReader::new(conn));

        // Send only the header and partial content
        // Full message: Content-Length: 37\r\n\r\n{"type":"event","event":"terminated"}
        let header = "Content-Length: 37\r\n\r\n";
        let partial_content = "{\"type\":\"event\"";
        write!(&mut client, "{}{}", header, partial_content).expect("sending partial message");
        client.flush().expect("flushing");

        // First poll should timeout because message is incomplete
        let result = reader.try_poll_message(Duration::from_millis(50))?;
        assert!(
            matches!(result, PollResult::Timeout),
            "expected Timeout, got {:?}",
            result
        );

        // Verify that state was preserved (we're in ContentReading phase)
        // Now send the rest of the content
        let remaining_content = ",\"event\":\"terminated\"}";
        write!(&mut client, "{}", remaining_content).expect("sending remaining content");
        client.flush().expect("flushing");

        // Second poll should complete the message
        let result = reader.try_poll_message(Duration::from_secs(1))?;
        match result {
            PollResult::Message(message)
                if matches!(*message, Message::Event(events::Event::Terminated)) => {}
            other => panic!("expected Terminated event, got {:?}", other),
        }

        Ok(())
    }
}
