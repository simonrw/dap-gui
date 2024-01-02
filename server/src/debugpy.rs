use std::{
    io::{BufRead, BufReader},
    process::{Child, Stdio},
    sync::mpsc,
    thread,
};

use eyre::WrapErr;

use crate::Server;

pub struct DebugpyServer {
    child: Child,
}

impl Server for DebugpyServer {
    fn on_port(port: impl Into<u16>) -> eyre::Result<Self> {
        let port = port.into();

        tracing::debug!(port = ?port, "starting server process");
        let cwd = std::env::current_dir().unwrap();
        let mut child = std::process::Command::new("python")
            .args([
                "-m",
                "debugpy.adapter",
                "--host",
                "127.0.0.1",
                "--port",
                &format!("{port}"),
                "--log-stderr",
            ])
            .stderr(Stdio::piped())
            .current_dir(cwd.join("..").canonicalize().unwrap())
            .spawn()
            .context("spawning background process")?;

        // wait until server is ready
        tracing::debug!("waiting until server is ready");
        let stderr = child.stderr.take().unwrap();
        let reader = BufReader::new(stderr);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut should_signal = true;
            for line in reader.lines() {
                let line = line.unwrap();
                if should_signal && line.contains("Listening for incoming Client connections") {
                    should_signal = false;
                    let _ = tx.send(());
                }
            }
        });
        let _ = rx.recv();

        tracing::debug!("server ready");
        Ok(Self { child })
    }
}

impl Drop for DebugpyServer {
    fn drop(&mut self) {
        tracing::debug!("terminating server");
        match self.child.kill() {
            Ok(_) => {
                tracing::debug!("server terminated");
                let _ = self.child.wait();
            }
            Err(e) => tracing::warn!(error = %e, "could not terminate server process"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io::IsTerminal, net::TcpStream};

    use eyre::WrapErr;
    use tracing_subscriber::EnvFilter;
    use transport::bindings::get_random_tcp_port;

    use crate::{for_implementation_on_port, Implementation};

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
    fn test_create() -> eyre::Result<()> {
        init_test_logger();

        let port = get_random_tcp_port().context("reserving custom port")?;
        let _server =
            for_implementation_on_port(Implementation::Debugpy, port).context("creating server")?;

        // server should be running
        tracing::info!("making connection");
        let _conn =
            TcpStream::connect(format!("127.0.0.1:{port}")).context("connecting to server")?;
        Ok(())
    }
}
