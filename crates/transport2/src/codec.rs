//! DAP codec implementation using tokio-util.
//!
//! This module provides [`DapCodec`], which implements both the `Encoder` and
//! `Decoder` traits from tokio-util for DAP messages.

use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::error::CodecError;
use crate::message::{Message, OutgoingMessage};

/// Default maximum message size (16 MB).
const DEFAULT_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Codec for encoding and decoding DAP messages.
///
/// DAP uses a simple Content-Length header protocol:
/// ```text
/// Content-Length: <length>\r\n
/// \r\n
/// <JSON body>
/// ```
///
/// # Example
///
/// ```ignore
/// use tokio_util::codec::{FramedRead, FramedWrite};
/// use transport2::DapCodec;
///
/// let framed = FramedRead::new(reader, DapCodec::new());
/// ```
#[derive(Debug, Clone)]
pub struct DapCodec {
    /// Maximum allowed message size in bytes.
    max_message_size: usize,
}

impl DapCodec {
    /// Create a new codec with default settings.
    pub fn new() -> Self {
        Self {
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
        }
    }

    /// Create a new codec with a custom maximum message size.
    ///
    /// Messages larger than this will be rejected with [`CodecError::MessageTooLarge`].
    pub fn with_max_size(max_message_size: usize) -> Self {
        Self { max_message_size }
    }
}

impl Default for DapCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for DapCodec {
    type Item = Message;
    type Error = CodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Look for the header/body separator (\r\n\r\n)
        let Some(header_end) = find_header_end(src) else {
            // Need more data
            return Ok(None);
        };

        // Parse the Content-Length header
        let header_bytes = &src[..header_end];
        let content_length = parse_content_length(header_bytes)?;

        // Check message size limit
        if content_length > self.max_message_size {
            return Err(CodecError::MessageTooLarge {
                size: content_length,
                max: self.max_message_size,
            });
        }

        // Calculate total message length (header + \r\n\r\n + body)
        let total_length = header_end + 4 + content_length;

        // Check if we have the complete message
        if src.len() < total_length {
            // Need more data - reserve space for efficiency
            src.reserve(total_length - src.len());
            return Ok(None);
        }

        // Extract and parse the JSON body
        let body_start = header_end + 4;
        let body_bytes = &src[body_start..total_length];
        let message: Message =
            serde_json::from_slice(body_bytes).map_err(CodecError::JsonDeserialize)?;

        // Consume the processed bytes
        src.advance(total_length);

        Ok(Some(message))
    }
}

impl Encoder<OutgoingMessage> for DapCodec {
    type Error = CodecError;

    fn encode(&mut self, item: OutgoingMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // Serialize the message to JSON
        let json = serde_json::to_vec(&item).map_err(CodecError::JsonSerialize)?;

        // Write the header
        dst.reserve(32 + json.len()); // "Content-Length: " + digits + "\r\n\r\n" + body
        dst.put_slice(b"Content-Length: ");
        dst.put_slice(json.len().to_string().as_bytes());
        dst.put_slice(b"\r\n\r\n");

        // Write the body
        dst.put_slice(&json);

        Ok(())
    }
}

/// Find the position of the header/body separator (\r\n\r\n).
///
/// Returns the index of the first `\r` in the separator, or None if not found.
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parse the Content-Length value from the header section.
fn parse_content_length(header: &[u8]) -> Result<usize, CodecError> {
    let header_str = std::str::from_utf8(header).map_err(|_| CodecError::InvalidUtf8)?;

    for line in header_str.split("\r\n") {
        if let Some(value) = line.strip_prefix("Content-Length:") {
            return value
                .trim()
                .parse()
                .map_err(|_| CodecError::MalformedContentLength);
        }
    }

    Err(CodecError::MissingContentLength)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(json: &str) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.put_slice(format!("Content-Length: {}\r\n\r\n{}", json.len(), json).as_bytes());
        buf
    }

    #[test]
    fn decode_complete_message() {
        let mut codec = DapCodec::new();
        let json = r#"{"seq":1,"type":"event","event":"initialized"}"#;
        let mut buf = make_frame(json);

        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_some());
        assert!(buf.is_empty());
    }

    #[test]
    fn decode_incomplete_header() {
        let mut codec = DapCodec::new();
        let mut buf = BytesMut::from("Content-Length: 10");

        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
        assert!(!buf.is_empty()); // Data preserved
    }

    #[test]
    fn decode_incomplete_body() {
        let mut codec = DapCodec::new();
        let mut buf = BytesMut::from("Content-Length: 100\r\n\r\n{\"partial\":");

        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn decode_multiple_messages() {
        let mut codec = DapCodec::new();
        let json1 = r#"{"seq":1,"type":"event","event":"initialized"}"#;
        let json2 = r#"{"seq":2,"type":"event","event":"stopped","body":{}}"#;

        let mut buf = BytesMut::new();
        buf.put_slice(&make_frame(json1));
        buf.put_slice(&make_frame(json2));

        let msg1 = codec.decode(&mut buf).unwrap().unwrap();
        assert!(matches!(msg1, Message::Event(e) if e.seq == 1));

        let msg2 = codec.decode(&mut buf).unwrap().unwrap();
        assert!(matches!(msg2, Message::Event(e) if e.seq == 2));

        assert!(buf.is_empty());
    }

    #[test]
    fn decode_message_too_large() {
        let mut codec = DapCodec::with_max_size(10);
        let mut buf = BytesMut::from("Content-Length: 100\r\n\r\n");

        let result = codec.decode(&mut buf);
        assert!(matches!(result, Err(CodecError::MessageTooLarge { .. })));
    }

    #[test]
    fn encode_request() {
        let mut codec = DapCodec::new();
        let msg = OutgoingMessage::Request(crate::message::Request {
            seq: 1,
            command: "initialize".to_string(),
            arguments: None,
        });

        let mut buf = BytesMut::new();
        codec.encode(msg, &mut buf).unwrap();

        let s = std::str::from_utf8(&buf).unwrap();
        assert!(s.starts_with("Content-Length: "));
        assert!(s.contains("\r\n\r\n"));
        assert!(s.contains(r#""command":"initialize""#));
    }
}
