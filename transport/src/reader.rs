use std::io::{self, BufRead};

use anyhow::Context;

use crate::Message;

enum ReaderState {
    Header,
    Content,
}

pub struct Reader<R> {
    input: R,
}

impl<R> Reader<R>
where
    R: BufRead,
{
    pub fn new(input: R) -> Self {
        Self { input }
    }

    pub fn poll_message(
        &mut self,
        shutdown: &oneshot::Receiver<()>,
    ) -> anyhow::Result<Option<Message>> {
        let mut state = ReaderState::Header;
        let mut buffer = String::new();
        let mut content_length: usize = 0;

        loop {
            // check for shutdown
            match shutdown.try_recv() {
                Ok(_) => return Ok(None),
                Err(oneshot::TryRecvError::Empty) => {}
                Err(e) => {
                    tracing::error!(error = %e, "shutdown sender closed");
                    anyhow::bail!("shutdown sender closed");
                }
            }

            match self.input.read_line(&mut buffer) {
                Ok(read_size) => {
                    if read_size == 0 {
                        return Ok(None);
                    }

                    match state {
                        ReaderState::Header => {
                            let parts: Vec<&str> = buffer.trim_end().split(':').collect();
                            match parts[0] {
                                "Content-Length" => {
                                    content_length = match parts[1].trim().parse() {
                                        Ok(val) => val,
                                        Err(_) => {
                                            anyhow::bail!("failed to parse content length")
                                        }
                                    };
                                    buffer.clear();
                                    buffer.reserve(content_length);
                                    state = ReaderState::Content;
                                }
                                other => {
                                    anyhow::bail!("header {} not implemented", other);
                                }
                            }
                        }
                        ReaderState::Content => {
                            buffer.clear();
                            let mut content = vec![0; content_length];
                            self.input
                                .read_exact(content.as_mut_slice())
                                .map_err(|e| anyhow::anyhow!("failed to read: {:?}", e))?;
                            let content =
                                std::str::from_utf8(content.as_slice()).context("invalid utf8")?;
                            let message = serde_json::from_str(content).with_context(|| {
                                format!("could not construct message from: {content:?}")
                            })?;
                            return Ok(Some(message));
                        }
                    }
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

        let (_sender, shutdown) = oneshot::channel();

        let message = reader.poll_message(&shutdown).unwrap().unwrap();
        assert!(matches!(message, Message::Event(Event::Terminated)));
    }
}