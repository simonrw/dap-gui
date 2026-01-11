//! Transport abstraction and split functionality.
//!
//! This module provides the [`DapTransport`] trait for abstracting over
//! different async byte streams, and the [`split`] function for creating
//! reader/writer pairs.

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::reader::DapReader;
use crate::writer::DapWriter;

/// A transport that can be split into separate read and write halves.
///
/// This trait abstracts over different async transports (TCP, unix sockets,
/// in-memory streams) to provide a uniform interface for the DAP layer.
///
/// # Example
///
/// ```ignore
/// use transport2::{split, DapTransport};
/// use tokio::net::TcpStream;
///
/// let stream = TcpStream::connect("127.0.0.1:5678").await?;
/// let (reader, writer) = split(stream);
/// ```
pub trait DapTransport: Send + 'static {
    /// The read half type.
    type Read: AsyncRead + Unpin + Send + 'static;
    /// The write half type.
    type Write: AsyncWrite + Unpin + Send + 'static;

    /// Split the transport into separate read and write halves.
    fn into_split(self) -> (Self::Read, Self::Write);
}

impl DapTransport for TcpStream {
    type Read = OwnedReadHalf;
    type Write = OwnedWriteHalf;

    fn into_split(self) -> (Self::Read, Self::Write) {
        TcpStream::into_split(self)
    }
}

/// Split a transport into a DAP reader and writer pair.
///
/// This is the primary entry point for using the transport layer.
/// The returned reader and writer can be used independently and
/// concurrently, allowing upstream code to handle multiplexing
/// as needed.
///
/// # Example
///
/// ```ignore
/// use transport2::split;
/// use tokio::net::TcpStream;
///
/// let stream = TcpStream::connect("127.0.0.1:5678").await?;
/// let (reader, writer) = split(stream);
///
/// // Use reader and writer concurrently
/// tokio::spawn(async move {
///     while let Some(msg) = reader.next().await {
///         // handle messages
///     }
/// });
///
/// writer.send(request).await?;
/// ```
pub fn split<T: DapTransport>(transport: T) -> (DapReader<T::Read>, DapWriter<T::Write>) {
    let (read, write) = transport.into_split();
    (DapReader::new(read), DapWriter::new(write))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify that TcpStream implements DapTransport (compile-time check)
    fn _assert_tcp_transport(_: impl DapTransport) {}

    fn _check_tcp() {
        fn make_stream() -> TcpStream {
            unimplemented!()
        }
        _assert_tcp_transport(make_stream());
    }
}
