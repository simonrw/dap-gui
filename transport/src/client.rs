use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicI64, Ordering};
use std::thread;
use std::time::Duration;

use oneshot::Receiver;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
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

pub struct ClientInternals {
    // writer
    // TODO: trait implementor
    output: TcpStream,

    // common
    sequence_number: Arc<AtomicI64>,
    store: RequestStore,

    // Option because of drop and take
    exit: Option<oneshot::Sender<()>>,
}

/// DAP client
#[derive(Clone)]
pub struct Client {
    internals: Arc<Mutex<ClientInternals>>,
}

impl Client {
    pub fn new(stream: TcpStream, responses: spmc::Sender<Received>) -> Result<Self> {
        // internal state
        let sequence_number = Arc::new(AtomicI64::new(0));

        // Background poller to send responses and events
        let input_stream = stream.try_clone().unwrap();
        input_stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let store = RequestStore::default();
        let store_clone = Arc::clone(&store);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        thread::spawn(move || {
            let input = BufReader::new(input_stream);
            let mut reader = Reader {
                input,
                store: store_clone,
                responses,
            };
            if let Err(e) = reader.run_poll_loop(shutdown_rx) {
                tracing::warn!(error = ?e, "running poll loop");
            }
        });

        let internal = ClientInternals {
            output: stream,
            sequence_number,
            store,
            exit: Some(shutdown_tx),
        };

        Ok(Self {
            internals: Arc::new(Mutex::new(internal)),
        })
    }

    #[tracing::instrument(skip(self, body))]
    pub fn send(&self, body: requests::RequestBody) -> Result<()> {
        self.internals.lock().unwrap().send(body)
    }
}

impl ClientInternals {
    pub fn send(&mut self, body: requests::RequestBody) -> Result<()> {
        self.sequence_number.fetch_add(1, Ordering::SeqCst);
        let message = requests::Request {
            seq: self.sequence_number.load(Ordering::SeqCst),
            r#type: "request".to_string(),
            body: body.clone(),
        };
        let resp_json = serde_json::to_string(&message).unwrap();
        tracing::debug!(request = ?message, "sending message");
        write!(
            self.output,
            "Content-Length: {}\r\n\r\n{}",
            resp_json.len(),
            resp_json
        )
        .unwrap();
        self.output.flush().unwrap();

        let waiting_request = WaitingRequest(body);

        {
            let mut store = self.store.lock().unwrap();
            store.insert(message.seq, waiting_request);
        }
        Ok(())
    }
}

impl Drop for ClientInternals {
    fn drop(&mut self) {
        tracing::debug!("shutting down client");
        // Shutdown the background thread
        let _ = self.exit.take().unwrap().send(());
    }
}

enum ClientState {
    Header,
    Content,
}

struct Reader {
    input: BufReader<TcpStream>,
    store: RequestStore,
    responses: spmc::Sender<Received>,
}

impl Reader {
    fn poll_message(&mut self, shutdown: &Receiver<()>) -> Result<Option<Message>> {
        let mut state = ClientState::Header;
        let mut buffer = String::new();
        let mut content_length: usize = 0;

        loop {
            // check for shutdown
            match shutdown.try_recv() {
                Ok(_) => return Ok(None),
                Err(oneshot::TryRecvError::Empty) => {}
                Err(e) => {
                    tracing::error!(error = %e, "shutdown sender closed");
                    anyhow::bail!("shutdown sender closed");
                }
            }

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
                    if e.kind() == io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Err(anyhow::anyhow!("error reading from buffer: {e:?}"));
                }
            }
        }
    }

    fn run_poll_loop(&mut self, shutdown: Receiver<()>) -> Result<()> {
        loop {
            match self.poll_message(&shutdown) {
                Ok(Some(msg)) => match msg {
                    Message::Event(evt) => {
                        let _ = self.responses.send(Received::Event(evt));
                    }
                    Message::Response(r) => {
                        let mut store = self.store.lock().unwrap();
                        match store.remove(&r.request_seq) {
                            Some(request) => {
                                let _ = self.responses.send(Received::Response(request.0, r));
                            }
                            None => {
                                tracing::warn!(response = ?r, "no message in request store")
                            }
                        }
                    }
                },
                Ok(None) => {
                    tracing::debug!("ok none");
                    return Ok(());
                }
                Err(e) => eprintln!("reader error: {e}"),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Received {
    Event(events::Event),
    Response(requests::RequestBody, responses::Response),
}
