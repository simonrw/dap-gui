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
