//! TCP-based transport implementation

use std::io::BufReader;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use eyre::{Context, Result};

use super::DapTransport;

/// TCP-based DAP transport
///
/// This transport uses TCP sockets for communication with debug adapters.
/// It wraps [`TcpStream`] and provides the necessary configuration for
/// DAP protocol communication.
///
/// # Timeout Behavior
///
/// The reader is configured with a 1-second read timeout to enable
/// periodic checking of shutdown signals in the background polling thread.
/// When no data is available, the reader returns `WouldBlock` errors.
///
/// # Examples
///
/// ```no_run
/// use transport::io::TcpTransport;
///
/// // Connect to a debug adapter
/// let transport = TcpTransport::connect("127.0.0.1:5678")?;
/// # Ok::<(), eyre::Error>(())
/// ```
///
/// ```no_run
/// use std::net::TcpStream;
/// use transport::io::TcpTransport;
///
/// // Create from existing stream
/// let stream = TcpStream::connect("127.0.0.1:5678")?;
/// let transport = TcpTransport::new(stream)?;
/// # Ok::<(), eyre::Error>(())
/// ```
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Create a new TCP transport from an existing stream
    ///
    /// This configures the stream with appropriate read timeouts for
    /// DAP protocol communication.
    ///
    /// # Errors
    ///
    /// Returns an error if setting the read timeout fails
    pub fn new(stream: TcpStream) -> Result<Self> {
        // Set read timeout for WouldBlock behavior in polling loop
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .context("setting read timeout on TCP stream")?;
        Ok(Self { stream })
    }

    /// Connect to a debug adapter at the given address
    ///
    /// This is a convenience method that combines connection and transport creation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The connection fails
    /// - Setting the read timeout fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use transport::io::TcpTransport;
    ///
    /// let transport = TcpTransport::connect("127.0.0.1:5678")?;
    /// # Ok::<(), eyre::Error>(())
    /// ```
    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self> {
        let stream = TcpStream::connect(addr).context("connecting to debug adapter")?;
        Self::new(stream)
    }
}

impl DapTransport for TcpTransport {
    type Reader = BufReader<TcpStream>;
    type Writer = TcpStream;

    fn split(self) -> Result<(Self::Reader, Self::Writer)> {
        // Clone stream for reader, keep original for writer
        let input = self
            .stream
            .try_clone()
            .context("cloning TCP stream for reader")?;
        let output = self.stream;

        // Wrap reader in BufReader for efficient buffered reading
        let reader = BufReader::new(input);

        Ok((reader, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_transport_creation() {
        // This test just verifies the types compile correctly
        // Actual connection tests require a running server
        fn _type_check() -> Result<()> {
            let stream = TcpStream::connect("127.0.0.1:5678")?;
            let _transport = TcpTransport::new(stream)?;
            Ok(())
        }
    }
}
