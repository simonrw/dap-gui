use eyre::Context;
use responses::Response;
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
};

mod bindings;
mod codec;
mod decoder;
mod events;
mod request_store;
mod requests;
mod responses;
mod types;

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
        client.handle_message(msg);
    }
}

pub struct Client {
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

    fn handle_message(&mut self, msg: ClientMessage) {
        match msg {
            ClientMessage::Send {
                body: _,
                respond_to: _,
            } => todo!(),
            ClientMessage::Execute { body: _ } => todo!(),
        }
    }
}

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
