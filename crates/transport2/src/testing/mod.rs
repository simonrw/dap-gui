//! Testing utilities for the transport layer.
//!
//! This module provides helpers for testing code that uses the DAP transport,
//! including in-memory transports and message framing utilities.

mod memory;

pub use memory::MemoryTransport;

use serde::Serialize;

/// Construct a valid DAP message frame from a JSON-serializable message.
///
/// This is useful for constructing test data that can be fed to a [`DapReader`].
///
/// # Example
///
/// ```
/// use transport2::testing::frame_message;
/// use serde_json::json;
///
/// let bytes = frame_message(&json!({
///     "seq": 1,
///     "type": "event",
///     "event": "initialized"
/// }));
///
/// assert!(bytes.starts_with(b"Content-Length: "));
/// ```
pub fn frame_message(msg: &impl Serialize) -> Vec<u8> {
    let json = serde_json::to_string(msg).expect("failed to serialize message");
    format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
}

/// Construct multiple DAP message frames concatenated together.
///
/// # Example
///
/// ```
/// use transport2::testing::frame_messages;
/// use serde_json::json;
///
/// let bytes = frame_messages(&[
///     json!({"seq": 1, "type": "event", "event": "initialized"}),
///     json!({"seq": 2, "type": "event", "event": "stopped", "body": {}}),
/// ]);
/// ```
pub fn frame_messages<T: Serialize>(msgs: &[T]) -> Vec<u8> {
    msgs.iter().flat_map(|m| frame_message(m)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_frame_message() {
        let bytes = frame_message(&json!({"seq": 1, "type": "event", "event": "test"}));
        let s = String::from_utf8(bytes).unwrap();

        assert!(s.starts_with("Content-Length: "));
        assert!(s.contains("\r\n\r\n"));
        assert!(s.contains(r#""event":"test""#));
    }

    #[test]
    fn test_frame_messages() {
        let bytes = frame_messages(&[
            json!({"seq": 1, "type": "event", "event": "a"}),
            json!({"seq": 2, "type": "event", "event": "b"}),
        ]);
        let s = String::from_utf8(bytes).unwrap();

        // Should contain two Content-Length headers
        assert_eq!(s.matches("Content-Length:").count(), 2);
    }
}
