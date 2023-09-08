use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicI64, Ordering};

use serde::Deserialize;
use std::sync::Arc;
// TODO: use internal error type
use anyhow::{Context, Result};

use crate::request_store::{RequestStore, WaitingRequest};
use crate::{events, requests, responses};

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

/// DAP client
pub struct Client {
    // writer
    // TODO: trait implementor
    output: TcpStream,

    // common
    sequence_number: Arc<AtomicI64>,
    store: RequestStore,
}

impl Client {
    pub fn new(stream: TcpStream) -> Result<Self> {
        // internal state
        let sequence_number = Arc::new(AtomicI64::new(1));

        Ok(Self {
            output: stream,
            sequence_number,
            store: RequestStore::default(),
        })
    }

    pub fn reader<Handler>(&self, handler: Handler) -> Reader<Handler> {
        let input = self.output.try_clone().unwrap();
        Reader {
            input: BufReader::new(input),
            store: Arc::clone(&self.store),
            handler,
        }
    }

    pub fn send(&mut self, body: requests::RequestBody) -> Result<responses::Response> {
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
            self.output,
            "Content-Length: {}\r\n\r\n{}",
            resp_json.len(),
            resp_json
        )
        .unwrap();
        self.output.flush().unwrap();

        let (tx, rx) = oneshot::channel();
        let waiting_request = WaitingRequest {
            request: message.clone(),
            responder: tx,
        };

        {
            let mut store = self.store.lock().unwrap();
            store.insert(message.seq, waiting_request);
        }
        let reply = rx.recv().unwrap();
        Ok(reply)
    }
}

enum ClientState {
    Header,
    Content,
}

pub struct Reader<Handler> {
    input: BufReader<TcpStream>,
    store: RequestStore,
    handler: Handler,
}

impl<Handler> Reader<Handler>
where
    Handler: EventHandler,
{
    fn poll_message(&mut self) -> Result<Option<Message>> {
        let mut state = ClientState::Header;
        let mut buffer = String::new();
        let mut content_length: usize = 0;

        loop {
            match self.input.read_line(&mut buffer) {
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
                            self.input
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

    pub fn run_poll_loop(&mut self) -> Result<()> {
        loop {
            match self.poll_message() {
                Ok(Some(msg)) => match msg {
                    Message::Event(e) => {
                        tracing::debug!(event = ?e, "got event");
                        if let Err(e) = self.handler.on_event(e) {
                            tracing::warn!(error = %e, "error handling event");
                        };
                    }
                    Message::Response(r) => {
                        tracing::debug!(response = ?r, "got response");
                        let mut store = self.store.lock().unwrap();
                        match store.remove(&r.request_seq) {
                            Some(w) => {
                                let _ = w.responder.send(r);
                            }
                            None => todo!(),
                        }
                    }
                },
                Ok(None) => return Ok(()),
                Err(e) => eprintln!("reader error: {e}"),
            }
        }
    }
}

pub trait EventHandler {
    type Error: std::fmt::Display;
    fn on_event(&mut self, event: events::Event) -> Result<(), Self::Error>;
}

impl<E, F> EventHandler for F
where
    F: Fn(events::Event) -> Result<(), E>,
    E: std::fmt::Display,
{
    type Error = E;

    fn on_event(&mut self, event: events::Event) -> Result<(), Self::Error> {
        self(event)
    }
}
