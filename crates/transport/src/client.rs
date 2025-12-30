use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicI64, Ordering};
use std::thread;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
// TODO: use internal error type
use eyre::{Context, Result};

use crate::io::{DapTransport, TcpTransport};
use crate::request_store::{RequestStore, WaitingRequest};
use crate::responses::Response;
use crate::{Reader, events, reader, requests, responses};

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
    // writer - now generic over any Write implementation
    output: Box<dyn Write + Send>,

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
    /// Create a new DAP client with a custom transport
    ///
    /// This is the generic constructor that accepts any type implementing
    /// [`DapTransport`]. Use this when you want to use alternative transports
    /// like in-memory channels for testing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use transport::{Client, io::TcpTransport};
    ///
    /// let transport = TcpTransport::connect("127.0.0.1:5678")?;
    /// let (tx, rx) = crossbeam_channel::unbounded();
    /// let client = Client::with_transport(transport, tx)?;
    /// # Ok::<(), eyre::Error>(())
    /// ```
    ///
    /// ```
    /// use transport::{Client, io::InMemoryTransport};
    ///
    /// let (client_transport, server_transport) = InMemoryTransport::pair();
    /// let (tx, rx) = crossbeam_channel::unbounded();
    /// let client = Client::with_transport(client_transport, tx)?;
    /// # Ok::<(), eyre::Error>(())
    /// ```
    pub fn with_transport<T>(
        transport: T,
        responses: crossbeam_channel::Sender<events::Event>,
    ) -> Result<Self>
    where
        T: DapTransport,
    {
        // internal state
        let sequence_number = Arc::new(AtomicI64::new(0));

        // Split transport into reader and writer
        let (input, output) = transport.split()?;

        let store = RequestStore::default();
        let store_clone = Arc::clone(&store);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Background poller to send responses and events
        thread::spawn(move || {
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
                    Err(e) => tracing::warn!("reader error: {e}"),
                }
            }
        });

        let internal = ClientInternals {
            output: Box::new(output),
            sequence_number,
            store,
            exit: Some(shutdown_tx),
        };

        Ok(Self {
            internals: Arc::new(Mutex::new(internal)),
        })
    }

    /// Create a new DAP client from a TCP stream
    ///
    /// This is a convenience constructor for the common case of connecting
    /// to a debug adapter over TCP. For more control or alternative transports,
    /// use [`Client::with_transport`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    /// use transport::Client;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:5678")?;
    /// let (tx, rx) = crossbeam_channel::unbounded();
    /// let client = Client::new(stream, tx)?;
    /// # Ok::<(), eyre::Error>(())
    /// ```
    pub fn new(
        stream: TcpStream,
        responses: crossbeam_channel::Sender<events::Event>,
    ) -> Result<Self> {
        let transport = TcpTransport::new(stream)?;
        Self::with_transport(transport, responses)
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
        // Use fetch_add return value to ensure atomicity
        let seq = self.sequence_number.fetch_add(1, Ordering::SeqCst) + 1;
        let message = requests::Request {
            seq,
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

        // Wait for response with timeout to prevent indefinite blocking
        let timeout = Duration::from_secs(30);
        let start = std::time::Instant::now();
        let mut attempts = 0;
        let res = loop {
            match rx.try_recv() {
                Ok(response) => break response,
                Err(oneshot::TryRecvError::Empty) => {
                    if start.elapsed() >= timeout {
                        eyre::bail!("Request timeout after {:?}", timeout);
                    }
                    attempts += 1;
                    // Use yield for the first many attempts to avoid latency,
                    // then switch to sleeping to avoid busy-waiting
                    if attempts < 1000 {
                        std::thread::yield_now();
                    } else {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Err(oneshot::TryRecvError::Disconnected) => {
                    eyre::bail!("Response sender disconnected");
                }
            }
        };
        Ok(res)
    }

    /// Execute a call on the client but do not wait for a response
    #[tracing::instrument(skip(self), level = "trace", fields(request))]
    pub fn execute(&mut self, body: requests::RequestBody) -> Result<()> {
        // Use fetch_add return value to ensure atomicity
        let seq = self.sequence_number.fetch_add(1, Ordering::SeqCst) + 1;
        let message = requests::Request {
            seq,
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
