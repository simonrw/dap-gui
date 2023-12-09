use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicI64, Ordering};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use std::sync::{Arc, Mutex, MutexGuard};
// TODO: use internal error type
use anyhow::Result;

use crate::request_store::{RequestStore, WaitingRequest};
use crate::responses::ResponseBody;
use crate::{events, requests, responses, Reader};

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
    pub fn new(stream: TcpStream, mut responses: spmc::Sender<events::Event>) -> Result<Self> {
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
            let mut reader = Reader::new(input);

            // poll loop
            loop {
                match reader.poll_message(&shutdown_rx) {
                    Ok(Some(msg)) => {
                        match msg {
                            Message::Event(evt) => {
                                let _ = responses.send(evt);
                            }
                            Message::Response(r) => {
                                with_lock("Reader.store", store_clone.as_ref(), |mut store| {
                                    match store.remove(&r.request_seq) {
                                        Some(WaitingRequest(_, tx)) => {
                                            let _ = tx.send(r.body);
                                        }
                                        None => {
                                            tracing::warn!(response = ?r, "no message in request store")
                                        }
                                    }
                                });
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("ok none");
                        return;
                    }
                    Err(e) => eprintln!("reader error: {e}"),
                }
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
    pub fn send(&self, body: requests::RequestBody) -> Result<Option<ResponseBody>> {
        with_lock(
            "Client.internals",
            self.internals.as_ref(),
            |mut internals| internals.send(body),
        )
    }

    #[tracing::instrument(skip(self, body))]
    pub fn execute(&self, body: requests::RequestBody) -> Result<()> {
        with_lock(
            "Client.internals",
            self.internals.as_ref(),
            |mut internals| internals.execute(body),
        )
    }
}

fn with_lock<T, F, R>(name: &str, lock: &Mutex<T>, f: F) -> R
where
    F: FnOnce(MutexGuard<'_, T>) -> R,
{
    tracing::trace!(%name, "taking lock");
    let inner = lock.lock().unwrap();
    let res = f(inner);
    tracing::trace!(%name, "releasing lock");
    res
}

impl ClientInternals {
    pub fn send(&mut self, body: requests::RequestBody) -> Result<Option<ResponseBody>> {
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

        let (tx, rx) = oneshot::channel();
        let waiting_request = WaitingRequest(body, tx);

        with_lock("ClientInternals.store", self.store.as_ref(), |mut store| {
            store.insert(message.seq, waiting_request);
        });

        let res = rx.recv().expect("sender dropped");
        Ok(res)
    }

    /// Execute a call on the client but do not wait for a response
    pub fn execute(&mut self, body: requests::RequestBody) -> Result<()> {
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

#[derive(Clone, Debug)]
pub enum Received {
    Event(events::Event),
    Response(requests::RequestBody, responses::Response),
}
