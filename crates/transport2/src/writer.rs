//! DAP message writer.
//!
//! This module provides [`DapWriter`], a typed wrapper around a framed
//! async writer for sending DAP messages.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Sink;
use pin_project_lite::pin_project;
use tokio::io::AsyncWrite;
use tokio_util::codec::FramedWrite;

use crate::codec::DapCodec;
use crate::error::CodecError;
use crate::message::OutgoingMessage;

pin_project! {
    /// An async sink for outgoing DAP messages.
    ///
    /// `DapWriter` wraps an [`AsyncWrite`] destination and encodes DAP messages
    /// to the wire format. It provides a simple `send` method for common usage.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use transport2::{DapWriter, OutgoingMessage, Request};
    ///
    /// let mut writer = DapWriter::new(tcp_write_half);
    ///
    /// let request = OutgoingMessage::Request(Request {
    ///     seq: 1,
    ///     command: "initialize".to_string(),
    ///     arguments: None,
    /// });
    ///
    /// writer.send(request).await?;
    /// ```
    pub struct DapWriter<W> {
        #[pin]
        inner: FramedWrite<W, DapCodec>,
    }
}

impl<W> DapWriter<W>
where
    W: AsyncWrite + Unpin,
{
    /// Create a new DAP writer from an async write destination.
    pub fn new(writer: W) -> Self {
        Self {
            inner: FramedWrite::new(writer, DapCodec::new()),
        }
    }

    /// Create a new DAP writer with a custom codec.
    pub fn with_codec(writer: W, codec: DapCodec) -> Self {
        Self {
            inner: FramedWrite::new(writer, codec),
        }
    }

    /// Send a message to the debug adapter.
    ///
    /// This is a convenience method that handles the full send cycle:
    /// feeding the message, flushing, and awaiting completion.
    pub async fn send(&mut self, msg: OutgoingMessage) -> Result<(), CodecError> {
        use futures::SinkExt;
        SinkExt::send(&mut self.inner, msg).await
    }

    /// Get a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }

    /// Consume the writer and return the underlying destination.
    pub fn into_inner(self) -> W {
        self.inner.into_inner()
    }
}

impl<W> Sink<OutgoingMessage> for DapWriter<W>
where
    W: AsyncWrite + Unpin,
{
    type Error = CodecError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: OutgoingMessage) -> Result<(), Self::Error> {
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Request;
    use std::io::Cursor;

    #[tokio::test]
    async fn write_single_message() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut writer = DapWriter::new(cursor);

        let msg = OutgoingMessage::Request(Request {
            seq: 1,
            command: "initialize".to_string(),
            arguments: None,
        });

        writer.send(msg).await.unwrap();

        let output = writer.into_inner().into_inner();
        let output_str = String::from_utf8(output).unwrap();

        assert!(output_str.starts_with("Content-Length: "));
        assert!(output_str.contains("\r\n\r\n"));
        assert!(output_str.contains(r#""command":"initialize""#));
    }

    #[tokio::test]
    async fn write_multiple_messages() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let mut writer = DapWriter::new(cursor);

        for seq in 1..=3 {
            let msg = OutgoingMessage::Request(Request {
                seq,
                command: format!("command{}", seq),
                arguments: None,
            });
            writer.send(msg).await.unwrap();
        }

        let output = writer.into_inner().into_inner();
        let output_str = String::from_utf8(output).unwrap();

        assert!(output_str.contains(r#""command":"command1""#));
        assert!(output_str.contains(r#""command":"command2""#));
        assert!(output_str.contains(r#""command":"command3""#));
    }
}
