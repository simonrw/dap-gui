use std::pin::Pin;

use eyre::Context;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

use std::io;
enum ReaderState {
    Header,
    Content,
}

struct HandWrittenReader<R> {
    input: Pin<Box<R>>,
}

impl<R> HandWrittenReader<R>
where
    R: AsyncBufRead,
{
    fn new(input: R) -> Self {
        Self {
            input: Box::pin(input),
        }
    }

    async fn poll_message(&mut self) -> eyre::Result<Option<crate::Message>> {
        let mut state = ReaderState::Header;
        let mut buffer = String::new();
        let mut content_length: usize = 0;

        loop {
            match self.input.read_line(&mut buffer).await {
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
                                .await
                                .map_err(|e| eyre::eyre!("failed to read: {:?}", e))?;
                            let content =
                                std::str::from_utf8(content.as_slice()).wrap_err("invalid utf8")?;
                            let message = serde_json::from_str(content).wrap_err_with(|| {
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
    use tokio::io::{AsyncWriteExt, BufReader};
    use tokio::net::{TcpListener, TcpStream};

    use crate::bindings::get_random_tcp_port;
    use crate::{events, responses, Message};

    macro_rules! execute_test {
        ($($body:expr),+ => $match_expr:pat) => {{
            let port = get_random_tcp_port().expect("getting random port");
            let server = TcpListener::bind(format!("127.0.0.1:{port}"))
                .await
                .expect("binding to address");
            let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))
                .await
                .expect("connecting to server");
            let (conn, _) = server.accept().await.expect("accepting connection");

            let mut reader = crate::codec::HandWrittenReader::new(BufReader::new(conn));

            $(client
                .write_all($body.as_bytes())
                .await
                .expect("sending message");)*

            let message = reader.poll_message().await.expect("polling message");

            match message {
                Some(msg) => {
                    assert!(
                        matches!(msg, $match_expr),
                        "Got message {:?}",
                        msg
                    );
                }
                None => eyre::bail!("no message found"),
            }
        }};
        // multiple messages for single body
        ($body:expr => $($match_expr:pat),+) => {{
            let port = get_random_tcp_port().expect("getting random port");
            let server =
                TcpListener::bind(format!("127.0.0.1:{port}")).await.expect("binding to address");
            let mut client =
                TcpStream::connect(format!("127.0.0.1:{port}")).await.expect("connecting to server");
            let (conn, _) = server.accept().await.expect("accepting connection");

            let mut reader = crate::codec::HandWrittenReader::new(BufReader::new(conn));


            client
                .write_all($body.as_bytes())
                .await
                .expect("sending message");

            $(

            let message = reader.poll_message().await.expect("polling message");

            match message {
                Some(msg) => {
                    assert!(matches!(msg, $match_expr));
                }
                None => eyre::bail!("no message found"),
            }

            )+
        }};
    }

    #[tokio::test]
    async fn single_message() -> eyre::Result<()> {
        let body = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}";

        execute_test!(body => Message::Event(events::Event::Terminated));

        Ok(())
    }

    #[tokio::test]
    async fn split_between_requests() -> eyre::Result<()> {
        execute_test!(
            "Content-Length: 37\r\n\r\n{\"ty",
            "pe\":\"event\",\"event\":\"terminated\"}" => 
        Message::Event(events::Event::Terminated));

        Ok(())
    }
    #[tokio::test]
    async fn multiple_messages() -> eyre::Result<()> {
        let body = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}";

        execute_test!(body => Message::Event(events::Event::Terminated), Message::Event(events::Event::Terminated));

        Ok(())
    }
    #[tokio::test]
    async fn evaluate_error() -> eyre::Result<()> {
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
