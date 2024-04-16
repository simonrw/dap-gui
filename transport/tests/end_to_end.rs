use dap_codec::dap::requests::{Command, PathFormat};
use eyre::WrapErr;
use tokio::{net::TcpListener, sync::mpsc};

use server::for_implementation_on_port;
use transport::{handle_messages, Client, ClientMessage};

async fn get_random_tcp_port() -> eyre::Result<u16> {
    for _ in 0..50 {
        match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => {
                let addr = listener.local_addr().unwrap();
                let port = addr.port();
                return Ok(port);
            }
            Err(e) => {
                tracing::warn!(%e, "binding");
            }
        }
    }

    eyre::bail!("could not get free port");
}

#[tokio::test]
async fn test_loop() -> eyre::Result<()> {
    let port = get_random_tcp_port().await.wrap_err("getting free port")?;
    let _server = for_implementation_on_port(server::Implementation::Debugpy, port)
        .wrap_err("creating server process")?;

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .wrap_err("connecting to server")?;

    // events channel
    let (tx, _events) = mpsc::channel(10);

    let client = Client::new(stream, tx);

    // initialise
    let res = client
        .send(Command::Initialize(
            dap_codec::dap::requests::InitializeArguments {
                adapter_id: "test".to_string(),
                ..Default::default()
            },
        ))
        .await;
    dbg!(&res);
    assert!(res.success);

    Ok(())
}
