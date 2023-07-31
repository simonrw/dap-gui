use std::io::{BufRead, BufReader, BufWriter, Read, Write};

use serde::Deserialize;
use std::sync::mpsc::Sender;
// TODO: use internal error type
use anyhow::{Context, Result};
use serde::Serialize;

pub mod events;
pub mod responses;
pub mod types;

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

pub struct Writer<W>
where
    W: Write,
{
    output_buffer: BufWriter<W>,
    sequence_number: i64,
}

impl<W> Writer<W>
where
    W: Write,
{
    pub fn new(output_buffer: BufWriter<W>) -> Self {
        Self {
            output_buffer,
            sequence_number: 0,
        }
    }

    fn send(&mut self, body: serde_json::Value) -> Result<()> {
        // thread::sleep(Duration::from_secs(1));
        self.sequence_number += 1;
        let message = BaseMessage {
            seq: self.sequence_number,
            r#type: "request".to_string(),
            body,
        };
        let resp_json = serde_json::to_string(&message).unwrap();
        log::trace!("sending message {resp_json}");
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

    pub fn send_stacktrace_request(&mut self, thread_id: u64) {
        log::debug!("sending stacktrace request");
        self.send(serde_json::json!({
            "command": "stackTrace",
            "arguments": {
                "threadId": thread_id,
            },
        })).unwrap();
    }

    pub fn send_threads_request(&mut self) {
        log::debug!("sending configuration done");
        self.send(serde_json::json!({
            "command": "threads",
        }))
        .unwrap();
    }

    pub fn send_configuration_done(&mut self) {
        log::debug!("sending configuration done");
        self.send(serde_json::json!({
            "command": "configurationDone",
        }))
        .unwrap();
    }

    pub fn send_initialize(&mut self) {
        log::debug!("sending initialize");
        self.send(serde_json::json!({
            "command": "initialize",
            "arguments": {
                "adapterID": "dap-gui",
                }

        }))
        .unwrap();
    }

    pub fn send_continue(&mut self, thread_id: i64) {
        log::debug!("sending continue");
        self.send(serde_json::json!({
            "command": "continue",
            "arguments": {
                "threadId": thread_id,  // TODO
                "singleThread": false,
            },
        }))
        .unwrap();
    }

    pub fn send_set_function_breakpoints(&mut self) {
        log::debug!("sending set function breakpoints");
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

    pub fn send_launch(&mut self) {
        log::debug!("sending launch");
        self.send(serde_json::json!({
            "command": "launch",
            "arguments": {
                "program": concat!(env!("HOME"), "/dev/dap-gui/test.py"),
            }
        }))
        .unwrap();
    }
}

pub struct Reader<R>
where
    R: Read,
{
    input_buffer: BufReader<R>,
    dest: Sender<Message>,
}

impl<R> Reader<R>
where
    R: Read,
{
    pub fn new(input: BufReader<R>, dest: Sender<Message>) -> Self {
        Self {
            input_buffer: input,
            dest,
        }
    }

    pub fn poll_loop(&mut self) {
        loop {
            self.receive();
        }
    }

    fn receive(&mut self) {
        match self.poll_message() {
            Ok(Some(msg)) => {
                let _ = self.dest.send(msg);
            }
            // match msg {
            // Message::Event(m) => match m {
            //     events::Event::Initialized => {
            //         eprintln!("server ready to receive breakpoint commands");
            //         self.send_set_function_breakpoints();
            //     }
            //     events::Event::Output(o) => {
            //         eprintln!("{}", o.output);
            //     }
            // },
            // Message::Response(r) => {
            //     if let Some(body) = r.body {
            //         match body {
            //             responses::ResponseBody::Initialize(_init) => {
            //                 self.send_launch();
            //             }
            //             responses::ResponseBody::SetFunctionBreakpoints(bps) => {
            //                 dbg!(bps);
            //             }
            //         }
            //     }
            // }
            // },
            Ok(None) => (),
            Err(e) => log::warn!("error parsing response: {}", e),
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
                            let message = serde_json::from_str(content).with_context(|| format!("could not construct message from: {content:?}"))?;
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
