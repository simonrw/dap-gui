//! Async DAP transport layer using tokio.
//!
//! This crate provides the transport layer for the Debug Adapter Protocol (DAP),
//! handling encoding/decoding of messages over async byte streams.
//!
//! # Architecture
//!
//! The crate is designed around the tokio-util codec pattern:
//!
//! - [`DapCodec`] implements both `Encoder` and `Decoder` for DAP messages
//! - [`DapReader`] wraps an `AsyncRead` to produce a `Stream` of [`Message`]s
//! - [`DapWriter`] wraps an `AsyncWrite` to provide a `Sink` for outgoing messages
//!
//! # Usage
//!
//! ```ignore
//! use transport2::{connect, Message};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (mut reader, mut writer) = transport2::connect("127.0.0.1:5678").await?;
//!
//!     // Send a request
//!     writer.send(/* request */).await?;
//!
//!     // Read messages
//!     while let Some(msg) = reader.next().await {
//!         match msg? {
//!             Message::Response(r) => { /* handle response */ }
//!             Message::Event(e) => { /* handle event */ }
//!             Message::Request(r) => { /* handle reverse request */ }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Scope
//!
//! This crate intentionally handles only transport concerns:
//! - Encoding outgoing messages to the DAP wire format
//! - Decoding incoming bytes into typed messages
//! - Providing split reader/writer for upstream multiplexing
//!
//! Request-response correlation, event routing, and business logic
//! belong in upstream crates (e.g., `debugger`).

mod codec;
mod error;
mod message;
mod reader;
mod transport;
mod writer;

pub mod testing;

// Re-export main types
pub use codec::DapCodec;
pub use error::CodecError;
pub use message::{
    Event, Message, OutgoingEvent, OutgoingMessage, OutgoingResponse, Request, Response,
};
pub use reader::DapReader;
pub use transport::{DapTransport, split};
pub use writer::DapWriter;

use std::io;
use tokio::net::{TcpStream, ToSocketAddrs};

/// Connect to a DAP server and return a reader/writer pair.
///
/// This is a convenience function for the common case of connecting
/// to a debug adapter over TCP.
///
/// # Example
///
/// ```ignore
/// let (reader, writer) = transport2::connect("127.0.0.1:5678").await?;
/// ```
pub async fn connect(
    addr: impl ToSocketAddrs,
) -> io::Result<(
    DapReader<tokio::net::tcp::OwnedReadHalf>,
    DapWriter<tokio::net::tcp::OwnedWriteHalf>,
)> {
    let stream = TcpStream::connect(addr).await?;
    Ok(split(stream))
}
