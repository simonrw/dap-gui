use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicI64, Ordering};

use serde::Deserialize;
use std::sync::{mpsc::Sender, Arc, Mutex};
use types::{StackFrameId, ThreadId, VariablesReference};
// TODO: use internal error type
use anyhow::{Context, Result};

use crate::requests::{
    Breakpoint, Continue, Initialize, Launch, RequestBody, Scopes, SetFunctionBreakpoints,
    StackTrace, Variables,
};

pub mod events;
pub mod requests;
pub mod responses;
pub mod types;

pub type RequestStore = Arc<Mutex<HashMap<i64, requests::Request>>>;

#[derive(Debug)]
pub struct Reply {
    pub message: Message,
    pub request: Option<requests::Request>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum Message {
    Event(events::Event),
    Response(responses::Response),
}

#[derive(Debug)]
pub struct Writer {
    output_buffer: TcpStream,
    sequence_number: Arc<AtomicI64>,
    store: RequestStore,
}

impl Clone for Writer {
    fn clone(&self) -> Self {
        Self {
            output_buffer: self.output_buffer.try_clone().unwrap(),
            sequence_number: Arc::clone(&self.sequence_number),
            store: self.store.clone(),
        }
    }
}

impl Writer {
    pub fn new(output_buffer: TcpStream, store: RequestStore) -> Self {
        Self {
            output_buffer,
            sequence_number: Arc::new(AtomicI64::new(0)),
            store,
        }
    }

    fn send(&mut self, body: RequestBody) -> Result<()> {
        // thread::sleep(Duration::from_secs(1));
        self.sequence_number.fetch_add(1, Ordering::SeqCst);
        let message = requests::Request {
            seq: self.sequence_number.load(Ordering::SeqCst),
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
        let mut store = self.store.lock().unwrap();
        store.insert(message.seq, message);
        Ok(())
    }

    pub fn send_stacktrace_request(&mut self, thread_id: ThreadId) {
        log::debug!("sending stacktrace request");
        self.send(RequestBody::StackTrace(StackTrace { thread_id }))
            .unwrap();
    }

    pub fn send_threads_request(&mut self) {
        log::debug!("sending configuration done");
        self.send(RequestBody::Threads).unwrap();
    }

    pub fn send_configuration_done(&mut self) {
        log::debug!("sending configuration done");
        self.send(RequestBody::ConfigurationDone).unwrap();
    }

    pub fn send_initialize(&mut self) {
        log::debug!("sending initialize");
        self.send(RequestBody::Initialize(Initialize {
            adapter_id: "dap-gui".to_string(),
        }))
        .unwrap();
    }

    pub fn send_continue(&mut self, thread_id: i64) {
        log::debug!("sending continue");
        self.send(RequestBody::Continue(Continue {
            thread_id,
            single_thread: false,
        }))
        .unwrap();
    }

    pub fn send_set_function_breakpoints(&mut self, breakpoints: Vec<Breakpoint>) {
        log::debug!("sending set function breakpoints");
        self.send(RequestBody::SetFunctionBreakpoints(
            SetFunctionBreakpoints {
                breakpoints,
                // breakpoints: vec![Breakpoint {
                //     name: "foo".to_string(),
                // }],
            },
        ))
        .unwrap();
        // TOOD: how to set multiple breakpoints
    }

    pub fn send_launch(&mut self) {
        log::debug!("sending launch");
        self.send(RequestBody::Launch(Launch {
            program: concat!(env!("HOME"), "/dev/dap-gui/test.py").to_string(),
        }))
        .unwrap();
    }

    pub fn send_scopes(&mut self, id: StackFrameId) {
        log::debug!("sending scopes for stack frame {id}");
        self.send(RequestBody::Scopes(Scopes { frame_id: id }))
            .unwrap();
    }

    pub fn send_variables(&mut self, vref: VariablesReference) {
        log::debug!("sending variables for reference {vref}");
        self.send(RequestBody::Variables(Variables {
            variables_reference: vref,
        }))
        .unwrap();
    }
}

pub struct Reader<R>
where
    R: Read,
{
    input_buffer: BufReader<R>,
    dest: Sender<Reply>,
    store: RequestStore,
}

impl<R> Reader<R>
where
    R: Read,
{
    pub fn new(input: BufReader<R>, dest: Sender<Reply>, store: RequestStore) -> Self {
        Self {
            input_buffer: input,
            dest,
            store,
        }
    }

    pub fn poll_loop(&mut self) {
        loop {
            self.receive();
        }
    }

    fn receive(&mut self) {
        match self.poll_message() {
            Ok(Some(msg)) => match msg {
                Message::Event(_) => {
                    let _ = self.dest.send(Reply {
                        message: msg,
                        request: None,
                    });
                }
                Message::Response(ref r) => {
                    let mut store = self.store.lock().unwrap();
                    let request = store.get(&r.request_seq);
                    let _ = self.dest.send(Reply {
                        message: msg.clone(),
                        request: request.cloned(),
                    });
                    if request.is_some() {
                        store.remove(&r.request_seq);
                    }
                }
            },
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
                            let message = serde_json::from_str(content).with_context(|| {
                                format!("could not construct message from: {content:?}")
                            })?;
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
