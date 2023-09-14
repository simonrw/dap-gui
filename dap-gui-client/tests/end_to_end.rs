use anyhow::{Context, Result};
use std::{
    io::{BufRead, BufReader},
    net::{TcpListener, TcpStream},
    process::Stdio,
    sync::mpsc,
    thread,
};
use tracing_subscriber::EnvFilter;

use dap_gui_client::{
    events,
    requests::{self, Initialize, Launch},
    responses,
};

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
            tracing::debug!(%line);

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

// Loop
// Initialize
// Launch
// Set function breakpoints
// Continue
#[test]
fn test_loop() -> Result<()> {
    init_test_logger();

    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");

    let (tx, rx) = mpsc::channel();
    with_server(|port| {
        let span = tracing::debug_span!("with_server", %port);
        let _guard = span.enter();

        let mut client = build_client(port, tx);

        // initialize
        let req = requests::RequestBody::Initialize(Initialize {
            adapter_id: "dap gui".to_string(),
        });
        let res = client.send(req).unwrap();
        assert!(res.success);

        // launch
        client
            .emit(requests::RequestBody::Launch(Launch {
                program: "./test.py".to_string(),
            }))
            .unwrap();

        // wait for initialized event
        let _initialized_event =
            wait_for_event(&rx, |e| matches!(e, events::Event::Initialized { .. }));

        // set function breakpoints
        let req = requests::RequestBody::SetFunctionBreakpoints(requests::SetFunctionBreakpoints {
            breakpoints: vec![requests::Breakpoint {
                name: "main".to_string(),
            }],
        });
        let res = client.send(req).unwrap();
        assert!(res.success);

        // configuration done
        let req = requests::RequestBody::ConfigurationDone;
        let res = client.send(req).unwrap();
        assert!(res.success);

        // wait for stopped event
        let events::Event::Stopped(events::StoppedEventBody {
            reason,
            thread_id,
            hit_breakpoint_ids,
        }) = wait_for_event(&rx, |e| matches!(e, events::Event::Stopped { .. }))
        else {
            unreachable!();
        };

        tracing::debug!(
            ?reason,
            ?thread_id,
            ?hit_breakpoint_ids,
            "got stopped event"
        );

        // fetch thread info
        let req = requests::RequestBody::Threads;
        let res = client.send(req).unwrap();
        assert!(res.success);

        // fetch stack info
        let req = requests::RequestBody::StackTrace(requests::StackTrace { thread_id });
        let res = client.send(req).unwrap();
        assert!(res.success);

        let Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
            stack_frames,
        })) = res.body
        else {
            unreachable!();
        };
        for frame in stack_frames {
            // scopes
            let req = requests::RequestBody::Scopes(requests::Scopes { frame_id: frame.id });
            let res = client.send(req).unwrap();
            assert!(res.success);

            let Some(responses::ResponseBody::Scopes(responses::ScopesResponse { scopes })) =
                res.body
            else {
                unreachable!();
            };

            // variables
            for scope in scopes {
                let req = requests::RequestBody::Variables(requests::Variables {
                    variables_reference: scope.variables_reference,
                });
                let res = client.send(req).unwrap();
                assert!(res.success);
            }
        }

        // continue
        let req = requests::RequestBody::Continue(requests::Continue {
            thread_id,
            single_thread: false,
        });
        tracing::debug!(?req, "sending continue request");
        let res = client.send(req).unwrap();
        assert!(res.success);

        wait_for_event(&rx, |e| matches!(e, events::Event::Terminated));

        // terminate
        let req = requests::RequestBody::Terminate(requests::Terminate {
            restart: Some(false),
        });
        let res = client.send(req).unwrap();
        assert!(res.success);

        // disconnect
        let req = requests::RequestBody::Disconnect(requests::Disconnect {
            terminate_debugee: true,
        });
        let res = client.send(req).unwrap();
        assert!(res.success);
        Ok(())
    })
}

fn wait_for_event<F>(rx: &mpsc::Receiver<events::Event>, pred: F) -> events::Event
where
    F: Fn(&events::Event) -> bool,
{
    tracing::debug!("waiting for event");
    let mut n = 0;
    for msg in rx {
        if n >= 10 {
            panic!("did not receive event");
        }

        if pred(&msg) {
            tracing::debug!(event = ?msg, "received expected event");
            return msg;
        } else {
            n += 1;
        }
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

fn build_client(port: u16, tx: mpsc::Sender<events::Event>) -> dap_gui_client::Client {
    let stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    dap_gui_client::Client::new(stream, tx).unwrap()
}
