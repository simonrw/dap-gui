use std::io::{self, BufRead};

use eyre::WrapErr;

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

    fn poll_message(&mut self) -> eyre::Result<Option<crate::Message>> {
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
                                            eyre::bail!("failed to parse content length")
                                        }
                                    };
                                    buffer.clear();
                                    buffer.reserve(content_length);
                                    state = ReaderState::Content;
                                }
                                other => {
                                    eyre::bail!("header {} not implemented", other);
                                }
                            }
                        }
                        ReaderState::Content => {
                            buffer.clear();
                            let mut content = vec![0; content_length];
                            self.input
                                .read_exact(content.as_mut_slice())
                                .map_err(|e| eyre::eyre!("failed to read: {:?}", e))?;
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
                    return Err(eyre::eyre!("error reading from buffer: {e:?}"));
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

    use crate::{Message, Reader, bindings::get_random_tcp_port, events, responses};

    use super::HandWrittenReader;

    macro_rules! execute_test {
        // multiple bodies for single message
        ($($body:expr_2021),+ => $match_expr:pat) => {{
            let port = get_random_tcp_port().expect("getting random port");
            let server =
                TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
            let mut client =
                TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
            let (conn, _) = server.accept().expect("accepting connection");

            let mut reader = HandWrittenReader::new(BufReader::new(conn));

            $(write!(&mut client, "{}", $body).expect("sending message");)+

            let message = reader.poll_message().expect("polling message");

            match message {
                Some(msg) => {
                    assert!(matches!(msg, $match_expr), "Got message {:?}", msg);
                }
                None => eyre::bail!("no message found"),
            }
        }};

        // multiple messages for single body
        ($body:expr_2021 => $($match_expr:pat),+) => {{
            let port = get_random_tcp_port().expect("getting random port");
            let server =
                TcpListener::bind(format!("127.0.0.1:{port}")).expect("binding to address");
            let mut client =
                TcpStream::connect(format!("127.0.0.1:{port}")).expect("connecting to server");
            let (conn, _) = server.accept().expect("accepting connection");

            let mut reader = HandWrittenReader::new(BufReader::new(conn));



            write!(&mut client, "{}", $body).expect("sending message");

            $(

            let message = reader.poll_message().expect("polling message");

            match message {
                Some(msg) => {
                    assert!(matches!(msg, $match_expr));
                }
                None => eyre::bail!("no message found"),
            }

            )+
        }};
    }

    #[test]
    fn single_message() -> eyre::Result<()> {
        let body = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}";

        execute_test!(body => Message::Event(events::Event::Terminated));

        Ok(())
    }

    #[test]
    fn split_between_requests() -> eyre::Result<()> {
        execute_test!(
            "Content-Length: 37\r\n\r\n{\"ty",
            "pe\":\"event\",\"event\":\"terminated\"}" => 
        Message::Event(events::Event::Terminated));

        Ok(())
    }

    #[test]
    fn multiple_messages() -> eyre::Result<()> {
        let body = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}";

        execute_test!(body => Message::Event(events::Event::Terminated), Message::Event(events::Event::Terminated));

        Ok(())
    }

    #[test]
    fn evaluate_error() -> eyre::Result<()> {
        let body = r#"Content-Length: 220"#.to_owned()
            + "\r\n\r\n"
            + r#"{"seq": 21, "type": "response", "request_seq": 13, "success": false, "command": "evaluate", "message": "Traceback (most recent call last):\n  File \"<string>\", line 1, in <module>\nNameError: name 'b' is not defined\n"}"#;

        execute_test!(body => Message::Response(responses::Response {
            message: Some(_),
            success: false,
            ..
        }));

        Ok(())
    }
}
