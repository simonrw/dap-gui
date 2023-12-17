use std::io::{self, BufRead};

use anyhow::Context;

use crate::Reader;

pub struct HandWrittenReader<R> {
    input: R,
}

enum ReaderState {
    Header,
    Content,
}

impl<R> Reader<R> for HandWrittenReader<R>
where
    R: BufRead,
{
    fn new(input: R) -> Self {
        Self { input }
    }

    fn poll_message(&mut self) -> anyhow::Result<Option<crate::Message>> {
        let mut state = ReaderState::Header;
        let mut buffer = String::new();
        let mut content_length: usize = 0;

        loop {
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
                                format!("could not construct message from: {content}")
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
    use std::{
        io::{BufReader, Write},
        net::{TcpListener, TcpStream},
    };

    use crate::{bindings::get_random_tcp_port, events, Message, Reader};

    use super::HandWrittenReader;

    macro_rules! execute_test {
        ($bodies:expr, $match_expr:pat) => {{
            let port = get_random_tcp_port().expect("getting random port");
            let server =
                TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
            let mut client =
                TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
            let (conn, _) = server.accept().expect("accepting connection");

            let mut reader = HandWrittenReader::new(BufReader::new(conn));

            // send message
            for body in $bodies {
                write!(&mut client, "{body}").expect("sending message");
            }

            let message = reader.poll_message().expect("polling message");

            match message {
                Some(msg) => {
                    assert!(matches!(msg, $match_expr));
                }
                None => anyhow::bail!("no message found"),
            }
        }};
    }

    #[test]
    fn single_message() -> anyhow::Result<()> {
        let bodies =
            vec!["Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}"];

        execute_test!(bodies, Message::Event(events::Event::Terminated));

        Ok(())
    }
}
