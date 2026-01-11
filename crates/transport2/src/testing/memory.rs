//! In-memory transport for testing.

use tokio::io::{DuplexStream, duplex};

use crate::transport::DapTransport;

/// An in-memory transport for testing DAP communication.
///
/// `MemoryTransport` uses tokio's [`DuplexStream`] to provide a bidirectional
/// in-memory channel that can be split into read and write halves.
///
/// # Example
///
/// ```
/// use transport2::testing::MemoryTransport;
/// use transport2::split;
///
/// // Create a connected pair of transports
/// let (client_transport, server_transport) = MemoryTransport::pair();
///
/// // Split into reader/writer pairs
/// let (client_reader, client_writer) = split(client_transport);
/// let (server_reader, server_writer) = split(server_transport);
///
/// // Now client_writer -> server_reader and server_writer -> client_reader
/// ```
pub struct MemoryTransport {
    read: DuplexStream,
    write: DuplexStream,
}

impl MemoryTransport {
    /// Create a connected pair of in-memory transports.
    ///
    /// Messages sent on one transport's writer will be received on the
    /// other transport's reader, simulating a bidirectional connection.
    ///
    /// Uses a default buffer size of 64KB for each direction.
    pub fn pair() -> (Self, Self) {
        Self::pair_with_buffer_size(64 * 1024)
    }

    /// Create a connected pair with a custom buffer size.
    ///
    /// Smaller buffers can be useful for testing backpressure behavior.
    pub fn pair_with_buffer_size(buffer_size: usize) -> (Self, Self) {
        let (a_to_b_write, a_to_b_read) = duplex(buffer_size);
        let (b_to_a_write, b_to_a_read) = duplex(buffer_size);

        let transport_a = MemoryTransport {
            read: b_to_a_read,
            write: a_to_b_write,
        };

        let transport_b = MemoryTransport {
            read: a_to_b_read,
            write: b_to_a_write,
        };

        (transport_a, transport_b)
    }
}

impl DapTransport for MemoryTransport {
    type Read = DuplexStream;
    type Write = DuplexStream;

    fn into_split(self) -> (Self::Read, Self::Write) {
        (self.read, self.write)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{OutgoingMessage, Request};
    use crate::split;
    use futures::StreamExt;

    #[tokio::test]
    async fn memory_transport_roundtrip() {
        let (client, server) = MemoryTransport::pair();

        let (mut client_reader, mut client_writer) = split(client);
        let (mut server_reader, mut server_writer) = split(server);

        // Client sends a request
        let request = OutgoingMessage::Request(Request {
            seq: 1,
            command: "test".to_string(),
            arguments: None,
        });
        client_writer.send(request).await.unwrap();

        // Server receives it
        let msg = server_reader.next().await.unwrap().unwrap();
        assert!(matches!(msg, crate::message::Message::Request(r) if r.command == "test"));

        // Server sends a response (as a request for simplicity)
        let response = OutgoingMessage::Request(Request {
            seq: 1,
            command: "reply".to_string(),
            arguments: None,
        });
        server_writer.send(response).await.unwrap();

        // Client receives it
        let msg = client_reader.next().await.unwrap().unwrap();
        assert!(matches!(msg, crate::message::Message::Request(r) if r.command == "reply"));
    }

    #[tokio::test]
    async fn memory_transport_multiple_messages() {
        let (client, server) = MemoryTransport::pair();

        let (_client_reader, mut client_writer) = split(client);
        let (mut server_reader, _server_writer) = split(server);

        // Send multiple messages
        for i in 1..=5 {
            let msg = OutgoingMessage::Request(Request {
                seq: i,
                command: format!("cmd{}", i),
                arguments: None,
            });
            client_writer.send(msg).await.unwrap();
        }

        // Receive all messages
        for i in 1..=5 {
            let msg = server_reader.next().await.unwrap().unwrap();
            if let crate::message::Message::Request(r) = msg {
                assert_eq!(r.seq, i);
                assert_eq!(r.command, format!("cmd{}", i));
            } else {
                panic!("expected request");
            }
        }
    }

    #[tokio::test]
    async fn memory_transport_close_signals_eof() {
        let (client, server) = MemoryTransport::pair();

        let (_client_reader, client_writer) = split(client);
        let (mut server_reader, _server_writer) = split(server);

        // Drop the writer
        drop(client_writer);

        // Reader should get EOF
        let result = server_reader.next().await;
        assert!(result.is_none());
    }
}
