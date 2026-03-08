use std::net::TcpListener;

use eyre::Result;

pub fn get_random_tcp_port() -> Result<u16> {
    for _ in 0..50 {
        match TcpListener::bind("127.0.0.1:0") {
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
