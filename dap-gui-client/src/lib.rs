use std::{
    io::{Cursor, Read, Write},
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};

use serde::Deserialize;
// TODO: use internal error type
use anyhow::Result;
use bytes::{Buf, BytesMut};
use serde::Serialize;

#[derive(Serialize)]
struct Request<Body>
where
    Body: Serialize,
{
    seq: i64,
    body: Body,
}

#[derive(Debug, Deserialize)]
pub enum Message {
    #[serde(rename = "event")]
    Event,
    #[serde(rename = "response")]
    Response,
}

impl Message {
    fn parse(buf: &mut Cursor<&[u8]>) -> std::result::Result<Self, ParseError> {
        todo!()
    }
}

pub struct Client<W> {
    seq: i64,
    stream: W,
    events: Receiver<Message>,
}

impl<W> Client<W>
where
    W: Write,
{
    pub fn new(stream: W, events: Receiver<Message>) -> Self {
        Self {
            seq: 1,
            stream,
            events,
        }
    }

    fn send_request(&mut self, w: impl Write, b: impl Serialize) {
        let request = Request {
            seq: self.seq,
            body: b,
        };

        serialize_request(w, &request);
        self.seq += 1;
    }

    pub fn send_initialize(&self) {}

    pub fn mainloop(&self) {
        for msg in &self.events {
            dbg!(msg);
        }
    }
}

fn serialize_request<Body>(mut w: impl Write, r: &Request<Body>)
where
    Body: Serialize,
{
    let body = serde_json::to_string(&r).expect("serializing payload");
    let n = body.len();
    write!(&mut w, "Content-Length: {n}\r\n\r\n{body}").unwrap();
}

#[derive(Debug)]
enum ParseError {
    Incomplete,
    Invalid,
    Other(anyhow::Error),
}

pub struct StreamReader {
    stream: TcpStream,
    buffer: BytesMut,
}

enum ClientState {
    Header,
    Content,
}

impl StreamReader {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buffer: BytesMut::new(),
        }
    }

    pub fn receive_messages(&mut self, events: Sender<Message>) {
        loop {
            match self.read_event() {
                Ok(Some(event)) => {
                    let _ = events.send(event);
                }
                Ok(None) => break, // EOF
                Err(e) => eprintln!("Error reading message: {e:?}"),
            }
        }
    }

    // dap crate's `poll_request`
    fn read_event(&mut self) -> Result<Option<Message>> {
        let mut state = ClientState::Header;
        loop {
            eprintln!("{}", self.buffer.len());
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            let mut buf = [0u8; 4096];
            let n = self.stream.read(&mut buf)?;
            self.buffer.extend_from_slice(&buf);
            if n == 0 {
                if self.buffer.is_empty() {
                    // EOF
                    return Ok(None);
                } else {
                    anyhow::bail!("connection reset by peer");
                }
            }
        }
    }

    fn parse_frame(&mut self) -> Result<Option<Message>> {
        let mut buf = Cursor::new(&self.buffer[..]);

        match dbg!(Message::parse(&mut buf)) {
            Ok(frame) => Ok(Some(frame)),
            Err(ParseError::Other(e)) => Err(e),
            Err(ParseError::Invalid) => Err(anyhow::anyhow!("invalid input")),
            Err(ParseError::Incomplete) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_message() {
        #[derive(Serialize)]
        struct Body {
            value: i32,
        }
        let request = Request {
            seq: 1,
            body: Body { value: 15 },
        };

        let mut s = Vec::new();
        serialize_request(&mut s, &request);

        assert_eq!(
            std::str::from_utf8(&s).unwrap(),
            "Content-Length: 29\r\n\r\n{\"seq\":1,\"body\":{\"value\":15}}"
        );
    }
}
