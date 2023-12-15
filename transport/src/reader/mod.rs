use crate::Message;

use std::io::BufRead;

#[cfg(not(nom))]
pub mod hand_written_reader;
#[cfg(nom)]
pub mod nom_reader;

pub trait Reader<R> {
    fn new(input: R) -> Self;
    fn poll_message(&mut self) -> anyhow::Result<Option<Message>>;
}

#[cfg(nom)]
pub fn get<R>(input: R) -> impl Reader<R>
where
    R: BufRead,
{
    tracing::debug!("getting nom reader");
    nom_reader::NomReader::new(input)
}

#[cfg(not(nom))]
pub fn get<R>(input: R) -> impl Reader<R>
where
    R: BufRead,
{
    tracing::debug!("getting hand written reader");
    hand_written_reader::HandWrittenReader::new(input)
}
