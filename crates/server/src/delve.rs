use std::process::{Child, Stdio};

use eyre::WrapErr;

use crate::Server;

pub struct DelveServer {
    child: Child,
}

impl Server for DelveServer {
    fn on_port(port: impl Into<u16>) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let port = port.into();

        tracing::debug!(port = ?port, "starting server process");

        // Validate dlv binary exists
        which::which("dlv").map_err(|_| {
            eyre::eyre!(
                "dlv not found in PATH. Install delve: https://github.com/go-delve/delve"
            )
        })?;

        let cwd = std::env::current_dir().context("getting current directory")?;
        let mut child = std::process::Command::new("dlv")
            .args(["dap", "--listen", &format!("127.0.0.1:{port}")])
            .stdout(Stdio::piped())
            .current_dir(&cwd)
            .spawn()
            .context("spawning background process")?;

        // wait until server is ready
        tracing::debug!("waiting until server is ready");
        let stdout = child.stdout.take().unwrap();
        crate::wait_for_ready(
            stdout,
            "DAP server listening",
            crate::SERVER_READY_TIMEOUT,
            &mut child,
        )
        .context("waiting for delve server readiness")?;

        tracing::debug!("server ready");
        Ok(Self { child })
    }
}

impl Drop for DelveServer {
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

    use crate::{Implementation, for_implementation_on_port};

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
            for_implementation_on_port(Implementation::Delve, port).context("creating server")?;

        // server should be running
        tracing::info!("making connection");
        let _conn =
            TcpStream::connect(format!("127.0.0.1:{port}")).context("connecting to server")?;
        Ok(())
    }
}
