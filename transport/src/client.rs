use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicI64, Ordering};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
// TODO: use internal error type
use eyre::{Context, Result};

use crate::request_store::{RequestStore, WaitingRequest};
use crate::responses::Response;
use crate::{events, reader, requests, responses, Reader};

#[allow(dead_code)]
#[derive(Debug)]
pub struct Reply {
    pub message: Message,
    pub request: Option<requests::Request>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum Message {
    Event(events::Event),
    Response(responses::Response),
    Request(requests::Request),
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
    pub fn new(
        stream: TcpStream,
        responses: crossbeam_channel::Sender<events::Event>,
    ) -> Result<Self> {
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
            let mut reader = reader::get(input);

            // poll loop
            loop {
                // check for shutdown
                match shutdown_rx.try_recv() {
                    Ok(_) => return,
                    Err(oneshot::TryRecvError::Empty) => {}
                    Err(e) => {
                        tracing::error!(error = %e, "shutdown sender closed");
                        return;
                    }
                }

                match reader.poll_message() {
                    Ok(Some(msg)) => match msg {
                        Message::Event(evt) => {
                            let _ = responses.send(evt);
                        }
                        Message::Response(r) => {
                            with_lock(
                                "Reader.store",
                                store_clone.as_ref(),
                                |mut store| match store.remove(&r.request_seq) {
                                    Some(WaitingRequest(_, tx)) => {
                                        let _ = tx.send(r);
                                    }
                                    None => {
                                        tracing::warn!(response = ?r, "no message in request store")
                                    }
                                },
                            );
                        }
                        Message::Request(_) => unreachable!("we should not be parsing requests"),
                    },
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

    #[tracing::instrument(skip(self, body), level = "debug")]
    pub fn send(&self, body: requests::RequestBody) -> Result<Response> {
        with_lock(
            "Client.internals",
            self.internals.as_ref(),
            |mut internals| internals.send(body),
        )
    }

    #[tracing::instrument(skip(self, body), level = "debug")]
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
    #[tracing::instrument(skip(self), level = "trace", fields(request))]
    pub fn send(&mut self, body: requests::RequestBody) -> Result<Response> {
        self.sequence_number.fetch_add(1, Ordering::SeqCst);
        let message = requests::Request {
            seq: self.sequence_number.load(Ordering::SeqCst),
            r#type: "request".to_string(),
            body: body.clone(),
        };
        let resp_json = serde_json::to_string(&message).wrap_err("encoding json body")?;
        tracing::Span::current().record("request", &resp_json);
        tracing::debug!("sending message");
        let (tx, rx) = oneshot::channel();
        let waiting_request = WaitingRequest(body, tx);

        with_lock("ClientInternals.store", self.store.as_ref(), |mut store| {
            store.insert(message.seq, waiting_request);
        });

        write!(
            self.output,
            "Content-Length: {}\r\n\r\n{}",
            resp_json.len(),
            resp_json
        )
        .wrap_err("writing message to output buffer")?;
        self.output.flush().wrap_err("flushing output buffer")?;

        let res = rx.recv().expect("sender dropped");
        Ok(res)
    }

    /// Execute a call on the client but do not wait for a response
    #[tracing::instrument(skip(self), level = "trace", fields(request))]
    pub fn execute(&mut self, body: requests::RequestBody) -> Result<()> {
        self.sequence_number.fetch_add(1, Ordering::SeqCst);
        let message = requests::Request {
            seq: self.sequence_number.load(Ordering::SeqCst),
            r#type: "request".to_string(),
            body: body.clone(),
        };
        let resp_json = serde_json::to_string(&message).unwrap();
        tracing::Span::current().record("request", &resp_json);
        tracing::debug!("sending message");
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
    Response(requests::RequestBody, Box<responses::Response>),
}
