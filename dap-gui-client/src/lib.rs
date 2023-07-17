use std::io::{BufRead, BufReader, BufWriter, Read, Write};

use serde::Deserialize;
// TODO: use internal error type
use anyhow::{Context, Result};
use serde::Serialize;

mod events;
mod responses;
mod types;

#[derive(Serialize)]
struct BaseMessage<Body>
where
    Body: Serialize,
{
    seq: i64,
    r#type: String,
    #[serde(flatten)]
    body: Body,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum Message {
    Event(events::Event),
    Response(responses::Response),
}

pub struct Client2<R, W>
where
    R: Read,
    W: Write,
{
    input_buffer: BufReader<R>,
    output_buffer: BufWriter<W>,
    sequence_number: i64,
}

impl<R, W> Client2<R, W>
where
    R: Read,
    W: Write,
{
    pub fn new(input: BufReader<R>, output: BufWriter<W>) -> Self {
        Self {
            input_buffer: input,
            output_buffer: output,
            sequence_number: 0,
        }
    }

    pub fn send_initialize(&mut self) {
        self.send(serde_json::json!({
            "command": "initialize",
            "arguments": {
                "adapterID": "dap-gui",
                }

        }))
        .unwrap();
    }

    pub fn receive(&mut self) {
        match self.poll_message() {
            Ok(Some(msg)) => match msg {
                Message::Event(m) => match m {
                    events::Event::Initialized => {
                        eprintln!("server ready to receive breakpoint commands");
                        self.send(serde_json::json!({
                            "command": "setFunctionBreakpoints",
                            "arguments": {
                                "breakpoints": [{
                                    "name": "main",
                                }],
                            },
                        }))
                        .unwrap();
                    }
                    events::Event::Output(o) => {
                        eprintln!("{}", o.output);
                    }
                },
                Message::Response(r) => {
                    if let Some(body) = r.body {
                        match body {
                            responses::ResponseBody::Initialize(_init) => {
                                // TODO: send launch event
                                self.send(serde_json::json!({
                                    "command": "launch",
                                    "arguments": {
                                        "program": "/home/simon/dev/dap-gui/test.py",
                                    }
                                }))
                                .unwrap();
                            }
                            responses::ResponseBody::SetFunctionBreakpoints(bps) => {
                                dbg!(bps);
                            }
                        }
                    }
                }
            },
            Ok(None) => (),
            Err(e) => todo!("{}", e),
        }
    }

    pub fn poll_message(&mut self) -> Result<Option<Message>> {
        let mut state = ClientState::Header;
        let mut buffer = String::new();
        let mut content_length: usize = 0;

        loop {
            match self.input_buffer.read_line(&mut buffer) {
                Ok(read_size) => {
                    if read_size == 0 {
                        return Ok(None);
                    }

                    match state {
                        ClientState::Header => {
                            let parts: Vec<&str> = buffer.trim_end().split(':').collect();
                            if parts.len() == 2 {
                                match parts[0] {
                                    "Content-Length" => {
                                        content_length = match parts[1].trim().parse() {
                                            Ok(val) => val,
                                            Err(_) => {
                                                anyhow::bail!("failed to parse content length")
                                            }
                                        };
                                        buffer.clear();
                                        buffer.reserve(content_length);
                                        state = ClientState::Content;
                                    }
                                    other => {
                                        anyhow::bail!("header {} not implemented", other);
                                    }
                                }
                            }
                        }
                        ClientState::Content => {
                            buffer.clear();
                            let mut content = vec![0; content_length];
                            self.input_buffer
                                .read_exact(content.as_mut_slice())
                                .map_err(|e| anyhow::anyhow!("failed to read: {:?}", e))?;
                            let content =
                                std::str::from_utf8(content.as_slice()).context("invalid utf8")?;
                            dbg!(&content);
                            let message = serde_json::from_str(content).context("invalid JSON")?;
                            return Ok(Some(message));
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("error reading from buffer: {e:?}"));
                }
            }
        }
    }

    pub fn send(&mut self, body: serde_json::Value) -> Result<()> {
        self.sequence_number += 1;
        let message = BaseMessage {
            seq: self.sequence_number,
            r#type: "request".to_string(),
            body,
        };
        let resp_json = serde_json::to_string(&message).unwrap();
        write!(
            self.output_buffer,
            "Content-Length: {}\r\n\r\n{}",
            resp_json.len(),
            resp_json
        )
        .unwrap();
        self.output_buffer.flush().unwrap();
        Ok(())
    }
}

enum ClientState {
    Header,
    Content,
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn serialize_message() {
//         #[derive(Serialize)]
//         struct Body {
//             value: i32,
//         }
//         let request = Request {
//             seq: 1,
//             body: Body { value: 15 },
//         };

//         let mut s = Vec::new();
//         serialize_request(&mut s, &request);

//         assert_eq!(
//             std::str::from_utf8(&s).unwrap(),
//             "Content-Length: 29\r\n\r\n{\"seq\":1,\"body\":{\"value\":15}}"
//         );
//     }
// }
