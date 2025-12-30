//! IO abstraction layer for DAP transport
//!
//! This module provides an abstraction over different transport mechanisms for
//! the Debug Adapter Protocol. The core trait [`DapTransport`] allows plugging
//! in different IO implementations such as TCP sockets, in-memory channels, or
//! stdio streams.
//!
//! # Examples
//!
//! ## Using TCP Transport
//!
//! ```no_run
//! use transport::io::TcpTransport;
//! use transport::Client;
//!
//! let transport = TcpTransport::connect("127.0.0.1:5678")?;
//! let (tx, rx) = crossbeam_channel::unbounded();
//! let client = Client::with_transport(transport, tx)?;
//! # Ok::<(), eyre::Error>(())
//! ```
//!
//! ## Using In-Memory Transport for Testing
//!
//! ```
//! use transport::io::InMemoryTransport;
//! use transport::Client;
//!
//! let (client_transport, server_transport) = InMemoryTransport::pair();
//! let (tx, rx) = crossbeam_channel::unbounded();
//! let client = Client::with_transport(client_transport, tx)?;
//! # Ok::<(), eyre::Error>(())
//! ```

use std::io::{BufRead, Write};

mod memory;
mod tcp;

#[cfg(test)]
mod tests;

pub use memory::InMemoryTransport;
pub use tcp::TcpTransport;

/// Trait for bidirectional DAP message transport
///
/// This trait abstracts over different IO mechanisms for sending and receiving
/// DAP protocol messages. Implementations must provide a way to split the
/// transport into separate reader and writer halves that can be moved into
/// different threads.
///
/// # Requirements
///
/// - The reader must implement [`BufRead`] for efficient line-based parsing
/// - The writer must implement [`Write`] for sending messages
/// - Both reader and writer must be `Send + 'static` to work with background threads
/// - The transport should handle timeouts appropriately (readers should return
///   `WouldBlock` when no data is available within the timeout period)
///
/// # Thread Safety
///
/// The transport is split into reader and writer halves. Typically:
/// - The reader is moved into a background polling thread
/// - The writer stays in the main thread (or calling thread)
///
/// Both halves must be independently usable across threads.
pub trait DapTransport: Send + 'static {
    /// The reader type that implements BufRead
    type Reader: BufRead + Send + 'static;

    /// The writer type that implements Write
    type Writer: Write + Send + 'static;

    /// Split the transport into separate reader and writer halves
    ///
    /// This method consumes the transport and returns independent reader and
    /// writer handles. The exact semantics depend on the implementation:
    ///
    /// - For TCP: Creates cloned handles to the same underlying socket
    /// - For in-memory: Returns the two ends of a bidirectional channel
    ///
    /// # Errors
    ///
    /// Returns an error if the transport cannot be split (e.g., socket cloning fails)
    fn split(self) -> eyre::Result<(Self::Reader, Self::Writer)>;
}
