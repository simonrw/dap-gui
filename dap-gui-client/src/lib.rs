use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::Receiver;

use serde::Deserialize;
use std::sync::{mpsc::Sender, Arc, Mutex};
use types::{Seq, StackFrameId, ThreadId, VariablesReference};
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

/// A container for the requests awaiting responses
pub type RequestStore = Arc<Mutex<HashMap<Seq, requests::Request>>>;

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

/*
pub fn send(&mut self, body: RequestBody) -> Result<()> {
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

enum ClientState {
    Header,
    Content,
}
*/
