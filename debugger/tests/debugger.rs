use anyhow::Context;
use debugger::Debugger;
use std::{io::IsTerminal, thread, time::Duration};
use tracing_subscriber::EnvFilter;

use transport::bindings::get_random_tcp_port;

#[test]
fn test_remote_attach() -> anyhow::Result<()> {
    init_test_logger();

    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");

    let port = get_random_tcp_port().context("getting free port")?;

    // run background process
    let mut child = std::process::Command::new("python")
        .args(&[
            "-Xfrozen_modules=off",
            "../attach.py",
            "-p",
            &format!("{port}"),
        ])
        .spawn()
        .context("running python process")?;

    // TODO
    thread::sleep(Duration::from_secs(1));

    let launch_args = debugger::AttachArguments {
        working_directory: cwd.clone(),
        port: Some(port),
        language: debugger::Language::DebugPy,
    };

    let debugger = Debugger::on_port(port, launch_args).context("creating debugger")?;
    let drx = debugger.events();

    let file_path = std::env::current_dir()
        .unwrap()
        .join("../attach.py")
        .canonicalize()
        .context("invalid debug target")?;

    wait_for_event("initialised event", &drx, |e| {
        matches!(e, debugger::Event::Initialised)
    });

    debugger.add_breakpoint(debugger::Breakpoint {
        path: file_path.clone(),
        line: 9,
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
                line: 9,
                contents: include_str!("../../attach.py").to_string(),
            })
        );
    });

    debugger.r#continue().context("resuming debugee")?;

    wait_for_event("terminated debuggee", &drx, |e| {
        matches!(e, debugger::Event::Ended)
    });

    let status = child.wait().context("waiting for child")?;
    assert_eq!(status.code().unwrap(), 0);

    Ok(())
}

#[test]
fn test_debugger() -> anyhow::Result<()> {
    init_test_logger();

    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");
    let port = get_random_tcp_port().context("getting free port")?;

    let file_path = std::env::current_dir()
        .unwrap()
        .join("../test.py")
        .canonicalize()
        .context("invalid debug target")?;

    let launch_args = debugger::LaunchArguments {
        // tests are run from the test subdirectory
        program: file_path.clone(),
        working_directory: None,
        language: debugger::Language::DebugPy,
    };
    let debugger = Debugger::on_port(port, launch_args).context("creating debugger")?;
    let drx = debugger.events();

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
