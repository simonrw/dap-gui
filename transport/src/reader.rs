use std::io::{self, BufRead};

use crate::parse::parse_message;
use crate::Message;

pub struct Reader<R> {
    input: R,
    buffer: String,
}

impl<R> Reader<R>
where
    R: BufRead,
{
    pub fn new(input: R) -> Self {
        Self {
            input,
            buffer: String::new(),
        }
    }

    pub fn poll_message(&mut self) -> anyhow::Result<Option<Message>> {
        loop {
            if !self.buffer.is_empty() {
                // try to parse from the buffer
                match parse_message(&self.buffer) {
                    Ok((input, message)) => {
                        tracing::trace!(rest = %input, "parsed message");
                        // overwrite the buffer with the remaining input from parsing the message
                        self.buffer = input.to_owned();
                        return Ok(Some(message));
                    }
                    Err(nom::Err::Incomplete(why)) => {
                        tracing::trace!(?why, "incomplete input");
                    }
                    Err(nom::Err::Failure(e)) | Err(nom::Err::Error(e)) => {
                        tracing::trace!(error = %e, %self.buffer, "error parsing message");
                    }
                }
            }

            match self.input.read_line(&mut self.buffer) {
                Ok(read_size) => {
                    if read_size == 0 {
                        return Ok(None);
                    }

                    tracing::trace!(read_size, "read bytes from socket");
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Err(anyhow::anyhow!("error reading from buffer: {e:?}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor, IsTerminal};

    use tracing_subscriber::EnvFilter;

    use crate::{events::Event, Message};

    use super::Reader;

    fn init_test_logger() {
        let in_ci = std::env::var("CI")
            .map(|val| val == "true")
            .unwrap_or(false);

        if std::io::stderr().is_terminal() || in_ci {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .try_init();
        } else {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .json()
                .try_init();
        }
    }

    #[test]
    fn empty_messsage() {
        init_test_logger();
        let message = Cursor::new(
            "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}\n",
        );

        let mut reader = Reader::new(BufReader::new(message));

        let message = reader.poll_message().unwrap().unwrap();
        assert!(matches!(message, Message::Event(Event::Terminated)));
    }

    #[test]
    fn multiple_messages() {
        init_test_logger();
        let message = Cursor::new(
            "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}\nContent-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}\n",
        );

        let mut reader = Reader::new(BufReader::new(message));

        let message = reader.poll_message().unwrap().unwrap();
        assert!(matches!(message, Message::Event(Event::Terminated)));

        let message = reader.poll_message().unwrap().unwrap();
        assert!(matches!(message, Message::Event(Event::Terminated)));
    }
}
