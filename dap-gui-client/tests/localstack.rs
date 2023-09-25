// Test connecting to and debugging LocalStack
//
// Run `DEVELOP=1 localstack start` in another terminal

use std::{
    io::{BufRead, BufReader},
    net::{TcpListener, TcpStream},
    process::Stdio,
    sync::mpsc,
    thread,
};

use anyhow::{Context, Result};
use dap_gui_client::{
    events,
    requests::{self, Attach, ConnectInfo, Initialize, PathMapping},
    responses, Received,
};
use tracing_subscriber::EnvFilter;

#[test]
#[ignore]
fn localstack() -> Result<()> {
    init_test_logger();

    let (tx, rx) = mpsc::channel();
    with_server(|port| {
        let span = tracing::debug_span!("with_server", %port);
        let _guard = span.enter();

        let stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        let mut client = dap_gui_client::Client::new(stream, tx).unwrap();

        // initialize
        let req = requests::RequestBody::Initialize(Initialize {
            adapter_id: "dap gui".to_string(),
        });
        client.send(req).unwrap();

        let _ = wait_for_response(&rx, |r| matches!(r, responses::ResponseBody::Initialize(_)));

        // attach
        let req = requests::RequestBody::Attach(Attach {
            connect: ConnectInfo {
                host: "localhost".to_string(),
                port: 5678,
            },
            path_mappings: vec![PathMapping {
                local_root: "/home/simon/work/localstack/localstack-ext/localstack_ext".to_string(),
                remote_root:
                    "/opt/code/localstack/.venv/lib/python3.10/site-packages/localstack_ext"
                        .to_string(),
            }],
            just_my_code: false,
            workspace_folder: "/home/simon/work/localstack/localstack-ext".to_string(),
        });
        client.send(req).unwrap();

        // wait for initialized event
        let _waiting_for_server_event = wait_for_event(&rx, |e| {
            tracing::debug!(event = ?e, "got event");
            if let events::Event::Unknown(s) = e {
                if s == "debugpyWaitingForServer" {
                    return true;
                }
            }
            return false;
        });

        Ok(())
    })
}

fn wait_for_response<F>(rx: &mpsc::Receiver<Received>, pred: F) -> responses::ResponseBody
where
    F: Fn(&responses::ResponseBody) -> bool,
{
    tracing::debug!("waiting for response");
    let mut n = 0;
    for msg in rx {
        if n >= 10 {
            panic!("did not receive response");
        }

        if let Received::Response(_, response) = msg {
            assert!(response.success);
            if let Some(body) = response.body {
                if pred(&body) {
                    tracing::debug!(response = ?body, "received expected response");
                    return body;
                }
            }
        }

        n += 1;
    }

    unreachable!()
}

fn wait_for_event<F>(rx: &mpsc::Receiver<Received>, pred: F) -> events::Event
where
    F: Fn(&events::Event) -> bool,
{
    tracing::debug!("waiting for event");
    let mut n = 0;
    for msg in rx {
        if n >= 10 {
            panic!("did not receive event");
        }

        if let Received::Event(evt) = msg {
            if pred(&evt) {
                tracing::debug!(event = ?evt, "received expected event");
                return evt;
            }
        } else {
            tracing::warn!(message = ?msg, "unhandled message");
        }

        n += 1;
    }

    unreachable!()
}

fn init_test_logger() {
    if atty::is(atty::Stream::Stderr) {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .json()
            .init();
    }
}

fn get_random_tcp_port() -> Result<u16> {
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

    anyhow::bail!("could not get free port");
}

// Function to start the server in the background
fn with_server<F>(f: F) -> Result<()>
where
    F: FnOnce(u16) -> Result<()>,
{
    let port = get_random_tcp_port().context("finding random tcp port")?;
    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");
    let mut child = std::process::Command::new("python")
        .args(&[
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
            tracing::trace!(%line);

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
