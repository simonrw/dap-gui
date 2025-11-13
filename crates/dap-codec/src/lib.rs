use bytes::Buf;
use dap::base_message::{BaseMessage, Sendable};
use tokio_util::codec::Decoder;

#[derive(thiserror::Error, Debug)]
enum CodecError {
    #[error("invalid utf8")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("invalid integer")]
    InvalidInteger(#[from] std::num::ParseIntError),
    #[allow(dead_code)]
    #[error("missing content-length header")]
    MissingContentLengthHeader,
    #[error("deserializing message content")]
    Deserializing(#[from] serde_json::Error),
}

#[allow(dead_code)]
struct DapDecoder {}

impl Decoder for DapDecoder {
    type Item = Sendable;

    type Error = Box<dyn std::error::Error>;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // skip to the start of the first header
        // TODO: we assume Content-Length for now
        let Some(start_pos) = src
            .windows("Content-Length".len())
            .position(|s| s == b"Content-Length")
        else {
            return Ok(None);
        };

        src.advance(start_pos);

        let Some(split_point) = src.windows(4).position(|s| s == b"\r\n\r\n") else {
            // TODO: is this always lack of input?
            return Ok(None);
        };

        let headers = &src[..split_point];
        let header_len = headers.len();
        // TOOD: parse other headers when they are added
        let content_length = 'cl: {
            let headers_str = std::str::from_utf8(headers).map_err(CodecError::InvalidUtf8)?;
            for header_str in headers_str.split("\r\n") {
                let (key, value) = {
                    let mut raw_key_and_value = header_str.split(':').take(2);
                    let key = raw_key_and_value.next().unwrap().trim();
                    let value = raw_key_and_value.next().unwrap().trim();
                    (key, value)
                };
                if key == "Content-Length" {
                    break 'cl value.parse::<usize>().map_err(CodecError::InvalidInteger)?;
                };
            }
            return Err(CodecError::MissingContentLengthHeader.into());
        };

        // check the buffer has enough bytes (including \r\n\r\n)
        let message_len_bytes = header_len + 4 + content_length;
        if src.len() < message_len_bytes {
            return Ok(None);
        }

        // parse the body
        let base_message: BaseMessage =
            serde_json::from_slice(&src[header_len + 4..message_len_bytes])
                .map_err(CodecError::Deserializing)?;

        src.advance(message_len_bytes);
        Ok(Some(base_message.message))
    }
}

#[cfg(test)]
mod tests {
    use bytes::BufMut;
    use dap::events::Event;
    use futures::prelude::*;
    use tokio_util::codec::FramedRead;

    use super::*;

    fn construct_message(message: &serde_json::Value) -> Vec<u8> {
        let body = serde_json::to_string(message).unwrap();
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    macro_rules! create_test {
        ($name:ident, $extra:expr_2021, $($input:expr_2021 => $expected:pat),+) => {
            #[tokio::test]
            async fn $name() {
                let mut messages = bytes::BytesMut::new();
                $(
                    let input = construct_message(&$input);
                    messages.put(&input[..]);
                )+

                messages.put(&$extra[..]);

                let mut framed_read = FramedRead::new(&messages[..], DapDecoder {});

                $(
                    let Some(msg) = framed_read.next().await else {
                        panic!()
                    };

                    let msg = msg.unwrap();
                    dbg!(&msg);
                    assert!(matches!(msg, $expected));
                )+
            }
        };

        ($name:ident, $($input:expr_2021 => $expected:pat),+) => {
            create_test!($name, b"", $($input => $expected),+);
        };
    }

    create_test!(
        initialized,
        serde_json::json!({
            "seq": 1,
            "type": "event",
            "event": "initialized",
        }) => Sendable::Event(Event::Initialized)
    );

    create_test!(
        initialized_two,
        serde_json::json!({
            "seq": 1,
            "type": "event",
            "event": "initialized",
        }) => Sendable::Event(Event::Initialized),
        serde_json::json!({
            "seq": 1,
            "type": "event",
            "event": "initialized",
        }) => Sendable::Event(Event::Initialized)
    );

    create_test!(
        remaining_data,
        b"abc",
        serde_json::json!({
            "seq": 1,
            "type": "event",
            "event": "initialized",
        }) => Sendable::Event(Event::Initialized)
    );
}
