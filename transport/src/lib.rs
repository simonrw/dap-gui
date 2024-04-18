use std::collections::HashMap;

pub const DEFAULT_DAP_PORT: u16 = 5678;

use dap_codec::{
    dap::{
        base_message::{BaseMessage, Sendable},
        events::Event,
        requests::Command,
        responses::Response,
    },
    DapDecoder,
};
use futures::stream::StreamExt;
use tokio::{
    io::AsyncWriteExt,
    net::{tcp::OwnedReadHalf, TcpStream},
    sync::{mpsc, oneshot},
};

use tokio_util::codec::FramedRead;
use tracing::Instrument;

#[derive(thiserror::Error, Debug)]
pub enum TransportError {
    #[error("server went away")]
    ServerDropped,
}

type Result<T> = std::result::Result<T, TransportError>;

#[derive(Debug)]
pub enum ClientMessage {
    Send {
        request: Command,
        response_chan: oneshot::Sender<Response>,
    },
    Execute {
        request: Command,
    },
}

fn encode_message(seq_num: i64, request: Command) -> Vec<u8> {
    let message = BaseMessage {
        seq: seq_num,
        message: Sendable::Request(request),
    };
    let encoded_message = serde_json::to_string(&message).unwrap();
    let message_length = encoded_message.len();
    format!("Content-Length: {message_length}\r\n\r\n{encoded_message}").into_bytes()
}

// the "handle" type, or a more OO wrapper around the actor running in a background task
#[derive(Clone)]
pub struct Client {
    sender: mpsc::Sender<ClientMessage>,
}

impl Client {
    pub fn new(stream: TcpStream, events: mpsc::Sender<Event>) -> Self {
        let (sender, receiver) = mpsc::channel(100);
        tokio::spawn(handle_messages(stream, receiver, events));
        Self { sender }
    }

    // TODO: return a Result where the error variant represents a response that was unsuccessful
    pub async fn send(&self, request: Command) -> Result<Response> {
        let (tx, rx) = oneshot::channel();
        let client_message = ClientMessage::Send {
            request,
            response_chan: tx,
        };
        self.sender
            .send(client_message)
            .await
            .map_err(|_| TransportError::ServerDropped)?;
        rx.await.map_err(|_| TransportError::ServerDropped)
    }

    pub async fn execute(&self, request: Command) -> Result<()> {
        let client_message = ClientMessage::Execute { request };
        self.sender
            .send(client_message)
            .await
            .map_err(|_| TransportError::ServerDropped)?;
        Ok(())
    }
}

#[tracing::instrument(skip(stream, incoming, events))]
pub async fn handle_messages(
    stream: TcpStream,
    mut incoming: mpsc::Receiver<ClientMessage>,
    events: mpsc::Sender<Event>,
) {
    let (tx, mut rx) = mpsc::channel(100);
    let (read, mut write) = stream.into_split();

    tracing::debug!("spawning receiver task");
    tokio::spawn(async { receive_messages(read, tx).await });

    let mut responses = HashMap::<i64, oneshot::Sender<Response>>::new();
    let mut seq_num = 0;
    loop {
        tokio::select! {
            cmd = incoming.recv() => {
                let cmd = cmd.unwrap();

                let span = tracing::debug_span!("received command", command = ?cmd);

                // async block to ensure correct spans
                async {
                    match cmd {
                        ClientMessage::Send { request, response_chan } => {
                            tracing::debug!("received send request");

                            let current_seq_num = seq_num;
                            let bytes_to_send = encode_message(current_seq_num, request);
                            tracing::trace!(bytes = ?std::str::from_utf8(&bytes_to_send).unwrap(), "sending bytes");
                            write.write_all(&bytes_to_send).await.unwrap();
                            responses.insert(current_seq_num, response_chan);
                            seq_num += 1;
                        },
                        ClientMessage::Execute { request } => {
                            tracing::debug!("received execute request");

                            let current_seq_num = seq_num;
                            let bytes_to_send = encode_message(current_seq_num, request);
                            tracing::trace!(bytes = ?std::str::from_utf8(&bytes_to_send).unwrap(), "sending bytes");
                            write.write_all(&bytes_to_send).await.unwrap();
                            seq_num += 1;
                        },
                    }
                }.instrument(span).await
            }
            event = rx.recv() => {
                match event {
                    Some(Sendable::Event(evt)) => {
                        tracing::debug!(event = ?evt, "received event");
                        let _ = events.send(evt).await;
                    },
                    Some(Sendable::Response(resp)) => {
                        let span = tracing::debug_span!("received response", response = ?resp);
                        let _guard = span.enter();

                        if !resp.success {
                            tracing::warn!("response was unsuccessful");
                        }

                        // lookup response channel in responses hashmap
                        if let Some(tx) = responses.remove(&resp.request_seq) {
                            let _ = tx.send(resp);
                        }
                    },
                    Some(other) => {
                        tracing::warn!(event = ?other, "unexpected event type");
                    },
                    None => {
                        tracing::error!("no message received");
                        break;
                    }
                }
            }
        }
    }
}

#[tracing::instrument(skip(incoming, outbox))]
async fn receive_messages(
    incoming: OwnedReadHalf,
    outbox: tokio::sync::mpsc::Sender<Sendable>,
) -> eyre::Result<()> {
    let stream = FramedRead::new(incoming, DapDecoder {});
    tokio::pin!(stream);

    while let Some(msg) = stream.next().await {
        match msg {
            Ok(msg) => {
                tracing::trace!("received message from server");
                let _ = outbox.send(msg).await;
            }
            Err(e) => {
                tracing::error!(error = %e, "error receiving from stream");
                break;
            }
        }
    }
    Ok(())
}
