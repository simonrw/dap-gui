use crate::Message;

pub(crate) mod hand_written_reader;
pub(crate) mod nom_reader;

pub trait Reader {
    fn poll_message(&mut self) -> anyhow::Result<Option<Message>>;
}
