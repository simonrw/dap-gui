use eyre::Context;
use requests::RequestBody;
use responses::Response;
use serde::{Deserialize, Serialize};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{mpsc, oneshot},
};

pub mod bindings;
mod codec;
mod decoder;
pub mod events;
mod request_store;
pub mod requests;
pub mod responses;
pub mod types;

/// The default port the DAP protocol listens on
pub const DEFAULT_DAP_PORT: u16 = 5678;

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

pub async fn run_client(mut client: Client) {
    while let Some(msg) = client.receiver.recv().await {
        if let Err(e) = client.handle_message(&msg).await {
            tracing::warn!(error = %e, message = ?msg, "handling message");
        }
    }
}

struct Client {
    receiver: mpsc::Receiver<ClientMessage>,

    sequence_number: i64,
    store: request_store::RequestStore,
    stream: TcpStream,
}

impl Client {
    fn new(receiver: mpsc::Receiver<ClientMessage>, stream: TcpStream) -> eyre::Result<Self> {
        let sequence_number = 0;
        Ok(Self {
            receiver,
            sequence_number,
            store: request_store::RequestStore::default(),
            stream,
        })
    }

    async fn handle_message(&mut self, msg: &ClientMessage) -> eyre::Result<()> {
        match msg {
            ClientMessage::Send {
                body,
                respond_to: _,
            } => {
                let res = self
                    .send(body)
                    .await
                    .wrap_err("sending message to server")?;
                todo!()
            }
            ClientMessage::Execute { body: _ } => todo!(),
        }
    }

    async fn send(&mut self, body: &RequestBody) -> eyre::Result<()> {
        todo!()
    }
}

#[derive(Debug)]
pub enum ClientMessage {
    Send {
        body: requests::RequestBody,
        respond_to: oneshot::Sender<eyre::Result<Response>>,
    },
    Execute {
        body: requests::RequestBody,
    },
}

// handle

#[derive(Clone)]
pub struct ClientHandle {
    sender: mpsc::Sender<ClientMessage>,
}

impl ClientHandle {
    pub fn new(stream: TcpStream) -> eyre::Result<Self> {
        let (sender, receiver) = mpsc::channel(8);
        let client = Client::new(receiver, stream)?;
        tokio::spawn(run_client(client));
        Ok(Self { sender })
    }

    pub async fn send(&self, body: requests::RequestBody) -> eyre::Result<Response> {
        let (send, recv) = oneshot::channel();
        let msg = ClientMessage::Send {
            body,
            respond_to: send,
        };
        let _ = self.sender.send(msg).await;
        let response = recv.await.wrap_err("actor task has been killed")?;
        response.wrap_err("bad response")
    }

    pub async fn execute(&self, body: requests::RequestBody) -> eyre::Result<()> {
        let msg = ClientMessage::Execute { body };
        let _ = self.sender.send(msg).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_client() {}
}
