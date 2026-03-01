use std::{
    io::{BufRead, BufReader, Read},
    process::Child,
    sync::mpsc,
    thread,
    time::Duration,
};

use eyre::WrapErr;
use transport::DEFAULT_DAP_PORT;

pub mod debugpy;
pub mod delve;

/// Default timeout for waiting for a server to become ready
const SERVER_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Wait for a server process to output a specific readiness string.
///
/// Spawns a background thread to read lines from the given reader,
/// looking for a line containing `needle`. Returns an error if the
/// timeout is exceeded or the child process exits before becoming ready.
fn wait_for_ready(
    reader: impl Read + Send + 'static,
    needle: &str,
    timeout: Duration,
    child: &mut Child,
) -> eyre::Result<()> {
    let needle_owned = needle.to_string();
    let (tx, rx) = mpsc::channel();

    let collected_output = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let collected_output_writer = collected_output.clone();

    thread::spawn(move || {
        let reader = BufReader::new(reader);
        let mut should_signal = true;
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => break,
            };
            if let Ok(mut output) = collected_output_writer.lock() {
                output.push(line.clone());
            }
            if should_signal && line.contains(&needle_owned) {
                should_signal = false;
                let _ = tx.send(());
            }
        }
    });

    match rx.recv_timeout(timeout) {
        Ok(()) => Ok(()),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Check if the child has already exited
            let exit_info = match child.try_wait() {
                Ok(Some(status)) => format!(" (process exited with status: {status})"),
                Ok(None) => " (process still running)".to_string(),
                Err(e) => format!(" (could not check process status: {e})"),
            };

            let output = collected_output
                .lock()
                .map(|o| o.join("\n"))
                .unwrap_or_default();

            eyre::bail!(
                "timed out after {timeout:?} waiting for server readiness (expected '{needle}'){exit_info}\nCollected output:\n{output}"
            )
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let exit_info = match child.try_wait() {
                Ok(Some(status)) => format!("process exited with status: {status}"),
                Ok(None) => "reader thread ended unexpectedly".to_string(),
                Err(e) => format!("could not check process status: {e}"),
            };

            let output = collected_output
                .lock()
                .map(|o| o.join("\n"))
                .unwrap_or_default();

            eyre::bail!(
                "server readiness detection failed: {exit_info}\nCollected output:\n{output}"
            )
        }
    }
}

pub enum Implementation {
    Debugpy,
    Delve,
}

pub trait Server {
    fn on_port(port: impl Into<u16>) -> eyre::Result<Self>
    where
        Self: Sized;

    fn new() -> eyre::Result<Self>
    where
        Self: Sized,
    {
        Self::on_port(DEFAULT_DAP_PORT)
    }
}

pub fn for_implementation(implementation: Implementation) -> eyre::Result<Box<dyn Server + Send>> {
    for_implementation_on_port(implementation, DEFAULT_DAP_PORT)
}

pub fn for_implementation_on_port(
    implementation: Implementation,
    port: impl Into<u16>,
) -> eyre::Result<Box<dyn Server + Send>> {
    match implementation {
        Implementation::Debugpy => {
            let server = crate::debugpy::DebugpyServer::on_port(port).context("creating server")?;
            Ok(Box::new(server))
        }
        Implementation::Delve => {
            let server = crate::delve::DelveServer::on_port(port).context("creating server")?;
            Ok(Box::new(server))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn test_wait_for_ready_timeout() {
        // Spawn a process that outputs something but never the expected readiness string
        let mut child = Command::new("sleep")
            .arg("60")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn sleep process");

        let stdout = child.stdout.take().unwrap();
        let result = wait_for_ready(stdout, "READY", Duration::from_millis(100), &mut child);

        // Clean up
        let _ = child.kill();
        let _ = child.wait();

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("timed out"),
            "expected timeout error, got: {err_msg}"
        );
    }

    #[test]
    fn test_wait_for_ready_success() {
        // Spawn a process that outputs the expected readiness string
        let mut child = Command::new("echo")
            .arg("Server is READY now")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn echo process");

        let stdout = child.stdout.take().unwrap();
        let result = wait_for_ready(stdout, "READY", Duration::from_secs(5), &mut child);

        let _ = child.wait();

        assert!(result.is_ok(), "expected success, got: {:?}", result.err());
    }

    #[test]
    fn test_wait_for_ready_process_exits_early() {
        // Spawn a process that exits immediately without the expected output
        let mut child = Command::new("echo")
            .arg("something else")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn echo process");

        let stdout = child.stdout.take().unwrap();
        let result = wait_for_ready(stdout, "NEVER_FOUND", Duration::from_secs(5), &mut child);

        let _ = child.wait();

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("readiness detection failed"),
            "expected readiness detection failure, got: {err_msg}"
        );
    }
}
