use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::Stdio;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use transport::bindings::get_random_tcp_port;

use debugger::*;

#[test]
#[ignore = "wip"]
fn test_debugger() -> Result<()> {
    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");
    let (tx, rx) = mpsc::channel();
    let messages = Arc::new(Mutex::new(Vec::new()));
    with_server(|port| {
        let span = tracing::debug_span!("with_server", %port);
        let _guard = span.enter();

        let stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        let client = transport::Client::new(stream, tx).unwrap();
        let mut debugger = Debugger::new(client, rx);

        let background_messages = Arc::clone(&messages);
        debugger.on_state_change(move |r| {
            background_messages.lock().unwrap().push(r.clone());
        });

        debugger.initialise().unwrap();

        Ok(())
    })
}

fn with_server<F>(f: F) -> Result<()>
where
    F: FnOnce(u16) -> Result<()>,
{
    let port = get_random_tcp_port().context("finding random tcp port")?;
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

    tracing::debug!("server started, waiting for completion");

    // wait until server is ready
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

    let result = f(port);

    child.kill().context("killing background process")?;
    child.wait().context("waiting for server to exit")?;
    result
}
