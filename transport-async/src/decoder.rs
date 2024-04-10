use tokio_util::codec::Decoder;

use crate::responses::Response;

pub(crate) struct MessageDecoder {}

impl Decoder for MessageDecoder {
    type Item = Response;

    type Error = eyre::Error;

    fn decode(&mut self, _src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        todo!()
    }
}
