use crate::Message;

use std::io::BufRead;

pub mod hand_written_reader;

pub trait Reader<R> {
    fn new(input: R) -> Self;
    fn poll_message(&mut self) -> eyre::Result<Option<Message>>;
}

pub fn get<R>(input: R) -> impl Reader<R>
where
    R: BufRead,
{
    tracing::debug!("getting hand written reader");
    hand_written_reader::HandWrittenReader::new(input)
}
