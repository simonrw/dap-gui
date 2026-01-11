//! DAP message reader.
//!
//! This module provides [`DapReader`], a typed wrapper around a framed
//! async reader that produces a stream of DAP messages.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use pin_project_lite::pin_project;
use tokio::io::AsyncRead;
use tokio_util::codec::FramedRead;

use crate::codec::DapCodec;
use crate::error::CodecError;
use crate::message::Message;

pin_project! {
    /// An async stream of incoming DAP messages.
    ///
    /// `DapReader` wraps an [`AsyncRead`] source and decodes DAP messages
    /// from the byte stream. It implements [`Stream`], allowing it to be
    /// used with async iteration patterns.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use futures::StreamExt;
    /// use transport2::DapReader;
    ///
    /// let reader = DapReader::new(tcp_read_half);
    ///
    /// while let Some(result) = reader.next().await {
    ///     match result? {
    ///         Message::Response(r) => { /* handle response */ }
    ///         Message::Event(e) => { /* handle event */ }
    ///         Message::Request(r) => { /* handle reverse request */ }
    ///     }
    /// }
    /// ```
    pub struct DapReader<R> {
        #[pin]
        inner: FramedRead<R, DapCodec>,
    }
}

impl<R> DapReader<R>
where
    R: AsyncRead + Unpin,
{
    /// Create a new DAP reader from an async read source.
    pub fn new(reader: R) -> Self {
        Self {
            inner: FramedRead::new(reader, DapCodec::new()),
        }
    }

    /// Create a new DAP reader with a custom codec.
    ///
    /// This allows configuring options like maximum message size.
    pub fn with_codec(reader: R, codec: DapCodec) -> Self {
        Self {
            inner: FramedRead::new(reader, codec),
        }
    }

    /// Get a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        self.inner.get_mut()
    }

    /// Consume the reader and return the underlying source.
    pub fn into_inner(self) -> R {
        self.inner.into_inner()
    }
}

impl<R> Stream for DapReader<R>
where
    R: AsyncRead + Unpin,
{
    type Item = Result<Message, CodecError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::io::Cursor;

    fn make_frame(json: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
    }

    #[tokio::test]
    async fn read_single_message() {
        let json = r#"{"seq":1,"type":"event","event":"initialized"}"#;
        let data = make_frame(json);
        let cursor = Cursor::new(data);

        let mut reader = DapReader::new(cursor);
        let msg = reader.next().await.unwrap().unwrap();

        assert!(matches!(msg, Message::Event(e) if e.event == "initialized"));
    }

    #[tokio::test]
    async fn read_multiple_messages() {
        let json1 = r#"{"seq":1,"type":"event","event":"initialized"}"#;
        let json2 =
            r#"{"seq":2,"type":"response","request_seq":1,"success":true,"command":"initialize"}"#;

        let mut data = make_frame(json1);
        data.extend(make_frame(json2));

        let cursor = Cursor::new(data);
        let mut reader = DapReader::new(cursor);

        let msg1 = reader.next().await.unwrap().unwrap();
        assert!(matches!(msg1, Message::Event(_)));

        let msg2 = reader.next().await.unwrap().unwrap();
        assert!(matches!(msg2, Message::Response(_)));
    }

    #[tokio::test]
    async fn read_eof() {
        let cursor = Cursor::new(Vec::new());
        let mut reader = DapReader::new(cursor);

        let result = reader.next().await;
        assert!(result.is_none());
    }
}
