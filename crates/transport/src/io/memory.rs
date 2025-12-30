//! In-memory transport implementation for testing

use std::io::{self, BufRead, Cursor, Read, Write};

use crossbeam_channel::{Receiver, Sender, TryRecvError};

use super::DapTransport;

/// In-memory transport for testing
///
/// This transport uses channels for bidirectional communication without
/// requiring actual network connections. It's primarily intended for unit
/// tests where you want to test the protocol layer without TCP overhead.
///
/// # Examples
///
/// ```
/// use transport::io::InMemoryTransport;
/// use transport::Client;
///
/// // Create a connected pair of transports
/// let (client_transport, server_transport) = InMemoryTransport::pair();
///
/// // Use client_transport with Client
/// let (tx, rx) = crossbeam_channel::unbounded();
/// let client = Client::with_transport(client_transport, tx)?;
///
/// // Use server_transport to simulate a debug adapter
/// // (send responses, receive requests)
/// # Ok::<(), eyre::Error>(())
/// ```
pub struct InMemoryTransport {
    reader: InMemoryReader,
    writer: InMemoryWriter,
}

/// Reader half of in-memory transport
///
/// Implements [`BufRead`] by reading from a channel and maintaining an
/// internal buffer. When the channel is empty, it returns `WouldBlock`
/// to simulate timeout behavior.
pub struct InMemoryReader {
    buffer: Cursor<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
}

/// Writer half of in-memory transport
///
/// Implements [`Write`] by sending data through a channel
pub struct InMemoryWriter {
    tx: Sender<Vec<u8>>,
}

impl InMemoryTransport {
    /// Create a connected pair of in-memory transports
    ///
    /// Returns `(client_transport, server_transport)` where data written to
    /// one can be read from the other.
    ///
    /// # Examples
    ///
    /// ```
    /// use transport::io::InMemoryTransport;
    ///
    /// let (client, server) = InMemoryTransport::pair();
    /// // client writes -> server reads
    /// // server writes -> client reads
    /// ```
    pub fn pair() -> (Self, Self) {
        let (client_tx, server_rx) = crossbeam_channel::unbounded();
        let (server_tx, client_rx) = crossbeam_channel::unbounded();

        let client = Self {
            reader: InMemoryReader {
                buffer: Cursor::new(Vec::new()),
                rx: client_rx,
            },
            writer: InMemoryWriter { tx: client_tx },
        };

        let server = Self {
            reader: InMemoryReader {
                buffer: Cursor::new(Vec::new()),
                rx: server_rx,
            },
            writer: InMemoryWriter { tx: server_tx },
        };

        (client, server)
    }
}

impl DapTransport for InMemoryTransport {
    type Reader = InMemoryReader;
    type Writer = InMemoryWriter;

    fn split(self) -> eyre::Result<(Self::Reader, Self::Writer)> {
        Ok((self.reader, self.writer))
    }
}

impl BufRead for InMemoryReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        // If current buffer is exhausted, try to get more data
        if self.buffer.position() >= self.buffer.get_ref().len() as u64 {
            match self.rx.try_recv() {
                Ok(data) => {
                    // Received new data, reset cursor
                    self.buffer = Cursor::new(data);
                }
                Err(TryRecvError::Empty) => {
                    // Simulate WouldBlock for timeout behavior
                    // This matches the TCP transport's read timeout semantics
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "no data available",
                    ));
                }
                Err(TryRecvError::Disconnected) => {
                    // Channel closed, return EOF
                    return Ok(&[]);
                }
            }
        }

        // Return buffered data
        self.buffer.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.buffer.consume(amt)
    }
}

impl Read for InMemoryReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Use BufRead implementation
        let available = self.fill_buf()?;
        let len = std::cmp::min(available.len(), buf.len());
        buf[..len].copy_from_slice(&available[..len]);
        self.consume(len);
        Ok(len)
    }
}

impl Write for InMemoryWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.tx
            .send(buf.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "channel disconnected"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // No-op for channels (always immediately flushed)
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_in_memory_pair_creation() {
        let (client, server) = InMemoryTransport::pair();
        let (_client_reader, mut client_writer) = client.split().unwrap();
        let (_server_reader, mut server_writer) = server.split().unwrap();

        // Verify we can write to the writers
        // Note: We keep readers alive so channels don't disconnect
        client_writer.write_all(b"test").unwrap();
        server_writer.write_all(b"test").unwrap();
    }

    #[test]
    fn test_bidirectional_communication() -> io::Result<()> {
        let (client, server) = InMemoryTransport::pair();
        let (mut client_reader, mut client_writer) = client.split().unwrap();
        let (mut server_reader, mut server_writer) = server.split().unwrap();

        // Client writes, server reads
        let msg = b"Hello from client";
        client_writer.write_all(msg)?;

        let mut buf = vec![0u8; msg.len()];
        server_reader.read_exact(&mut buf)?;
        assert_eq!(&buf, msg);

        // Server writes, client reads
        let response = b"Hello from server";
        server_writer.write_all(response)?;

        let mut buf = vec![0u8; response.len()];
        client_reader.read_exact(&mut buf)?;
        assert_eq!(&buf, response);

        Ok(())
    }

    #[test]
    fn test_would_block_on_empty_channel() -> io::Result<()> {
        let (client, _server) = InMemoryTransport::pair();
        let (mut reader, _writer) = client.split().unwrap();

        // Try to read when no data is available
        match reader.fill_buf() {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Ok(_) => panic!("Expected WouldBlock, got Ok"),
            Err(e) => panic!("Expected WouldBlock, got {:?}", e),
        }
    }

    #[test]
    fn test_eof_on_disconnect() -> io::Result<()> {
        let (client, server) = InMemoryTransport::pair();
        let (mut reader, _writer) = client.split().unwrap();

        // Drop server to close the channel
        drop(server);

        // Should get EOF (empty slice)
        let buf = reader.fill_buf()?;
        assert_eq!(buf.len(), 0);

        Ok(())
    }

    #[test]
    fn test_multiple_writes_buffering() -> io::Result<()> {
        let (client, server) = InMemoryTransport::pair();
        let (mut client_reader, _client_writer) = client.split().unwrap();
        let (_server_reader, mut server_writer) = server.split().unwrap();

        // Write multiple messages
        server_writer.write_all(b"First")?;
        server_writer.write_all(b"Second")?;
        server_writer.write_all(b"Third")?;

        // Read them one by one
        let mut buf = vec![0u8; 5];
        client_reader.read_exact(&mut buf)?;
        assert_eq!(&buf, b"First");

        client_reader.read_exact(&mut buf)?;
        assert_eq!(&buf, b"Secon");

        let mut buf = vec![0u8; 6];
        client_reader.read_exact(&mut buf)?;
        assert_eq!(&buf, b"dThird");

        Ok(())
    }
}
