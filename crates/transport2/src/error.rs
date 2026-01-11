//! Error types for the transport layer.

use std::io;

/// Errors that can occur during DAP message encoding/decoding.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// An I/O error occurred while reading or writing.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// The header section contained invalid UTF-8.
    #[error("invalid UTF-8 in header")]
    InvalidUtf8,

    /// The Content-Length header value could not be parsed as an integer.
    #[error("malformed Content-Length header value")]
    MalformedContentLength,

    /// No Content-Length header was found in the message.
    #[error("missing Content-Length header")]
    MissingContentLength,

    /// The message body exceeds the configured maximum size.
    #[error("message size {size} exceeds maximum allowed {max}")]
    MessageTooLarge {
        /// The actual message size.
        size: usize,
        /// The maximum allowed size.
        max: usize,
    },

    /// Failed to deserialize the JSON message body.
    #[error("JSON deserialization failed: {0}")]
    JsonDeserialize(#[source] serde_json::Error),

    /// Failed to serialize the outgoing message to JSON.
    #[error("JSON serialization failed: {0}")]
    JsonSerialize(#[source] serde_json::Error),
}
