use debugger::{PausedFrame, ProgramState};
use eyre::WrapErr;
use std::{
    io::{BufRead, BufReader, IsTerminal},
    process::Child,
    sync::mpsc,
    time::Duration,
};
use tracing_subscriber::EnvFilter;

use dap_types::{Source, StackFrame};
use server::util::get_random_tcp_port;

/// RAII guard to ensure child process is killed when dropped
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        tracing::debug!("killing child process");
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl std::ops::Deref for ChildGuard {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// test suite "constructor"
#[ctor::ctor]
fn init() {
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

    // error traces
    let _ = color_eyre::install();
}

#[tokio::test]
async fn test_remote_attach() -> eyre::Result<()> {
    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");

    let attach_script = cwd
        .join("..")
        .join("..")
        .join("attach.py")
        .canonicalize()
        .unwrap();

    let port = get_random_tcp_port().context("getting free port")?;

    // run background process with stdout/stderr captured for debugging
    let child = std::process::Command::new("python")
        .args([
            "-Xfrozen_modules=off",
            attach_script.display().to_string().as_str(),
            "-p",
            &format!("{port}"),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("running python process")?;

    // Wrap in guard to ensure cleanup
    let mut child = ChildGuard(child);

    // Wait for the Python process to start and bind to the port by watching
    // stderr for the readiness marker. This replaces a naive fixed sleep that
    // was too short on CI, causing "Connection refused" errors.
    let stderr = child.stderr.take().expect("stderr should be piped");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            tracing::debug!(line = %line, "attach.py stderr");
            if line.contains("DAP server listening") {
                let _ = tx.send(());
                break;
            }
        }
    });
    let readiness_timeout = Duration::from_secs(30);
    rx.recv_timeout(readiness_timeout).map_err(|_| {
        eyre::eyre!("timed out after {readiness_timeout:?} waiting for attach.py to become ready")
    })?;

    let file_path = std::env::current_dir()
        .unwrap()
        .join("../../attach.py")
        .canonicalize()
        .context("invalid debug target")?;

    let attach_args = debugger::AttachArguments {
        working_directory: cwd.clone(),
        port: Some(port),
        host: None,
        language: debugger::Language::DebugPy,
        path_mappings: None,
        just_my_code: None,
    };

    let mut debugger = debugger::TcpAsyncDebugger::connect(
        port,
        debugger::Language::DebugPy,
        debugger::SessionArgs::Attach(attach_args),
        debugger::StartMode::Staged,
    )
    .await
    .context("creating async debugger (attach)")?;

    let mut event_rx = debugger.take_events();

    // Wait for initialised event (may get Output/Thread events first)
    loop {
        let evt = event_rx.recv().await.unwrap();
        match evt {
            debugger::Event::Initialised => break,
            debugger::Event::Output { .. } | debugger::Event::Thread { .. } => continue,
            other => panic!("unexpected event while waiting for Initialised: {other:?}"),
        }
    }

    let breakpoint_line = 11;
    debugger
        .add_breakpoint(&debugger::Breakpoint {
            path: file_path.clone(),
            line: breakpoint_line,
            ..Default::default()
        })
        .await
        .context("adding breakpoint")?;
    debugger.start().await.context("launching debugee")?;

    // Wait for paused event (may get Running first)
    loop {
        let evt = event_rx.recv().await.unwrap();
        match evt {
            debugger::Event::Paused(ProgramState { paused_frame, .. }) => {
                assert!(matches!(
                    paused_frame,
                    PausedFrame {
                        frame: StackFrame {
                            source: Some(Source {
                                path: Some(ref path),
                                ..
                            }),
                            line,
                            ..
                        },
                        ..
                    } if *path == file_path && line as usize == breakpoint_line
                ));
                break;
            }
            debugger::Event::Running
            | debugger::Event::Output { .. }
            | debugger::Event::Thread { .. } => continue,
            other => panic!("unexpected event: {:?}", other),
        }
    }

    debugger.continue_().await.context("resuming debugee")?;

    // Wait for terminated
    loop {
        let evt = event_rx.recv().await.unwrap();
        if matches!(evt, debugger::Event::Ended) {
            break;
        }
    }

    // Wait for child process to exit
    let status = child.wait().context("waiting for child")?;
    if !status.success() {
        let stderr = child.stderr.take().map(|mut s| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut s, &mut buf).ok();
            buf
        });
        eyre::bail!(
            "Python process failed with status: {:?}\nstderr: {:?}",
            status,
            stderr
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_debugger() -> eyre::Result<()> {
    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");
    let port = get_random_tcp_port().context("getting free port")?;

    let file_path = std::env::current_dir()
        .unwrap()
        .join("../../test.py")
        .canonicalize()
        .context("invalid debug target")?;

    let _server = server::for_implementation_on_port(server::Implementation::Debugpy, port)
        .context("creating background server process")?;

    // Small delay to let the server start
    tokio::time::sleep(Duration::from_millis(500)).await;

    let launch_args = debugger::LaunchArguments {
        program: Some(file_path.clone()),
        module: None,
        args: None,
        env: None,
        working_directory: None,
        language: debugger::Language::DebugPy,
        just_my_code: None,
        stop_on_entry: None,
    };
    let mut debugger = debugger::TcpAsyncDebugger::connect(
        port,
        debugger::Language::DebugPy,
        debugger::SessionArgs::Launch(launch_args),
        debugger::StartMode::Staged,
    )
    .await
    .context("creating async debugger")?;

    let mut event_rx = debugger.take_events();

    // Wait for initialised event (may get Output/Thread events first)
    loop {
        let evt = event_rx.recv().await.unwrap();
        match evt {
            debugger::Event::Initialised => break,
            debugger::Event::Output { .. } | debugger::Event::Thread { .. } => continue,
            other => panic!("unexpected event while waiting for Initialised: {other:?}"),
        }
    }

    let breakpoint_line = 4;
    debugger
        .add_breakpoint(&debugger::Breakpoint {
            path: file_path.clone(),
            line: breakpoint_line,
            ..Default::default()
        })
        .await
        .context("adding breakpoint")?;
    debugger.start().await.context("launching debugee")?;

    // Wait for paused event (may get Running first)
    loop {
        let evt = event_rx.recv().await.unwrap();
        match evt {
            debugger::Event::Paused(ProgramState { paused_frame, .. }) => {
                assert!(matches!(
                    paused_frame,
                    PausedFrame {
                        frame: StackFrame {
                            source: Some(Source {
                                path: Some(ref path),
                                ..
                            }),
                            line,
                            ..
                        },
                        ..
                    } if *path == file_path && line as usize == breakpoint_line
                ));
                break;
            }
            debugger::Event::Running
            | debugger::Event::Output { .. }
            | debugger::Event::Thread { .. } => continue,
            other => panic!("unexpected event: {:?}", other),
        }
    }

    debugger.continue_().await.context("resuming debugee")?;

    // Wait for terminated
    loop {
        let evt = event_rx.recv().await.unwrap();
        if matches!(evt, debugger::Event::Ended) {
            break;
        }
    }

    Ok(())
}
