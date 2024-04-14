use dap::base_message::Sendable;
use tokio_util::codec::Decoder;

struct DapDecoder {}

impl Decoder for DapDecoder {
    type Item = Sendable;

    type Error = Box<dyn std::error::Error>;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use dap::events::Event;
    use futures::prelude::*;
    use tokio_util::codec::FramedRead;

    use super::*;

    #[tokio::test]
    async fn ping() {
        let input = br#"Content-Length: 78\r\n\r\n{"seq":1,"type":"event","body":"Initialized"}"#;
        let mut framed_read = FramedRead::new(&input[..], DapDecoder {});
        let message = framed_read.next().await.unwrap().unwrap();
        assert!(matches!(message, Sendable::Event(Event::Initialized)));
    }
}
