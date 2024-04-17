use bytes::Buf;
use dap::base_message::{BaseMessage, Sendable};
use tokio_util::codec::Decoder;

pub use dap;

#[derive(thiserror::Error, Debug)]
pub enum CodecError {
    #[error("invalid utf8")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("invalid integer")]
    InvalidInteger(#[from] std::num::ParseIntError),
    #[error("missing content-length header")]
    MissingContentLengthHeader,
    #[error("deserializing message content")]
    Deserializing(#[from] serde_json::Error),
    #[error("io error")]
    IO(#[from] std::io::Error),
    #[error("parsing header {0}")]
    ParseHeaderError(String),
}

struct Header {
    key: String,
    value: String,
}

fn parse_header(input: &str) -> Result<Header, CodecError> {
    let mut parts = input.splitn(2, ':');
    let key = parts
        .next()
        .ok_or(CodecError::ParseHeaderError(input.to_owned()))?;
    let value = parts
        .next()
        .ok_or(CodecError::ParseHeaderError(input.to_owned()))?;
    Ok(Header {
        key: key.trim().to_owned(),
        value: value.trim().to_owned(),
    })
}
pub struct DapDecoder {}

impl Decoder for DapDecoder {
    type Item = Sendable;

    type Error = CodecError;

    #[tracing::instrument(skip(self, src))]
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let span = tracing::debug_span!("parsing header");
        let guard = span.enter();

        // skip to the start of the first header
        // TODO: we assume Content-Length for now
        let Some(start_pos) = src
            .windows("Content-Length".len())
            .position(|s| s == b"Content-Length")
        else {
            tracing::debug!("no Content-Length header found");
            return Ok(None);
        };

        if start_pos > 0 {
            tracing::debug!(num_bytes = %start_pos, "skipping junk bytes");
            src.advance(start_pos);
        }

        let Some(split_point) = src.windows(4).position(|s| s == b"\r\n\r\n") else {
            // TODO: is this always lack of input?
            tracing::debug!("no end of headers found");
            return Ok(None);
        };

        let headers = &src[..split_point];
        let header_len = headers.len();
        tracing::trace!(num_header_bytes = %header_len, "found headers");
        // TOOD: parse other headers when they are added
        let content_length = 'cl: {
            let headers_str = std::str::from_utf8(headers).map_err(CodecError::InvalidUtf8)?;
            for header_str in headers_str.split("\r\n") {
                let header = parse_header(header_str)?;
                if header.key == "Content-Length" {
                    break 'cl header
                        .value
                        .parse::<usize>()
                        .map_err(CodecError::InvalidInteger)?;
                };
            }
            return Err(CodecError::MissingContentLengthHeader);
        };
        drop(guard);

        let span = tracing::debug_span!("parsing body", content_length);
        let _guard = span.enter();

        // check the buffer has enough bytes (including \r\n\r\n)
        let message_len_bytes = header_len + 4 + content_length;
        if src.len() < message_len_bytes {
            tracing::debug!(buffer_len = %src.len(), "not enough bytes for body");
            return Ok(None);
        }

        // parse the body
        let base_message: BaseMessage =
            serde_json::from_slice(&src[header_len + 4..message_len_bytes])
                .map_err(CodecError::Deserializing)?;

        tracing::debug!("body parsed");

        src.advance(message_len_bytes);
        Ok(Some(base_message.message))
    }
}

#[cfg(test)]
mod tests {
    use bytes::BufMut;
    use dap::{
        events::Event,
        requests::{Command, ContinueArguments},
        responses::{ContinueResponse, ResponseBody},
    };
    use futures::prelude::*;
    use tokio_util::codec::FramedRead;

    use super::*;

    fn construct_message(message: &serde_json::Value) -> Vec<u8> {
        let body = serde_json::to_string(message).unwrap();
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    macro_rules! create_test {
        ($name:ident, $extra:expr, $($input:expr => $expected:pat),+) => {
            #[tokio::test]
            async fn $name() {
                let _ = tracing_subscriber::fmt::try_init();

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

        ($name:ident, $($input:expr => $expected:pat),+) => {
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

    create_test!(
        request,
        serde_json::json!({
            "seq": 1,
            "type": "request",
            "command": "continue",
            "arguments": {
                "threadId": 1,
                "singleThread": true,
            },
        }) =>
        Sendable::Request(Command::Continue(ContinueArguments {
            thread_id: 1,
            single_thread: Some(true),
        }))
    );

    create_test!(
        response,
        serde_json::json!({
            "seq": 2,
            "type": "response",
            "request_seq": 1,
            "success": true,
            "command": "continue",
            "body": {
                "allThreadsContinued": true,
            },
        }) =>
        Sendable::Response(dap::responses::Response {
            request_seq: 1,
            success: true,
            message: None,
            error: None,
            body: Some(ResponseBody::Continue(ContinueResponse {
                all_threads_continued: Some(true),
            })),
        })
    );
}
