use anyhow::Context;
use debugger::Debugger;
use server::for_implementation_on_port;
use std::{io::IsTerminal, net::TcpStream};
use tracing_subscriber::EnvFilter;

use transport::bindings::get_random_tcp_port;

#[test]
fn test_debugger() -> anyhow::Result<()> {
    init_test_logger();

    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");
    let port = get_random_tcp_port().context("getting free port")?;
    let _server = for_implementation_on_port(server::Implementation::Debugpy, port)
        .context("creating server process")?;

    let (tx, rx) = spmc::channel();
    let span = tracing::debug_span!("with_server", %port);
    let _guard = span.enter();

    let stream = TcpStream::connect(format!("127.0.0.1:{port}")).context("connecting to server")?;
    let client = transport::Client::new(stream, tx).context("creating transport client")?;

    let debugger = Debugger::new(client, rx).context("creating debugger")?;
    let drx = debugger.events();

    let file_path = std::env::current_dir()
        .unwrap()
        .join("../test.py")
        .canonicalize()
        .context("invalid debug target")?;
    debugger
        .initialise(debugger::LaunchArguments {
            // tests are run from the test subdirectory
            program: file_path.clone(),
            working_directory: None,
            language: debugger::Language::DebugPy,
        })
        .context("initialising debugger")?;

    wait_for_event("initialised event", &drx, |e| {
        matches!(e, debugger::Event::Initialised)
    });

    debugger.add_breakpoint(debugger::Breakpoint {
        path: file_path.clone(),
        line: 4,
        ..Default::default()
    });
    debugger.launch().context("launching debugee")?;

    wait_for_event("running event", &drx, |e| {
        matches!(e, debugger::Event::Running { .. })
    });

    wait_for_event("paused event", &drx, |e| {
        matches!(e, debugger::Event::Paused { .. })
    });

    debugger.with_current_source(|source| {
        assert_eq!(
            source,
            Some(&debugger::FileSource {
                line: 4,
                contents: include_str!("../../test.py").to_string(),
            })
        );
    });

    debugger.r#continue().context("resuming debugee")?;

    wait_for_event("terminated debuggee", &drx, |e| {
        matches!(e, debugger::Event::Ended)
    });

    Ok(())
}

fn init_test_logger() {
    let in_ci = std::env::var("CI")
        .map(|val| val == "true")
        .unwrap_or(false);

    if std::io::stderr().is_terminal() || in_ci {
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

#[tracing::instrument(skip(rx, pred))]
fn wait_for_event<F>(
    message: &str,
    rx: &spmc::Receiver<debugger::Event>,
    pred: F,
) -> debugger::Event
where
    F: Fn(&debugger::Event) -> bool,
{
    tracing::debug!("waiting for {message} event");
    let mut n = 0;
    loop {
        let evt = rx.recv().unwrap();
        if n >= 100 {
            panic!("did not receive event");
        }

        if pred(&evt) {
            tracing::debug!(event = ?evt, "received expected event");
            return evt;
        } else {
            tracing::trace!(event = ?evt, "non-matching event");
        }
        n += 1;
    }
}
