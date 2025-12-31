use debugger::{Debugger, PausedFrame, ProgramState};
use eyre::WrapErr;
use std::{
    cell::RefCell, collections::VecDeque, io::IsTerminal, process::Child, thread, time::Duration,
};
use tracing_subscriber::EnvFilter;

use transport::{
    bindings::get_random_tcp_port,
    types::{Source, StackFrame},
};

// Thread-local event buffer to persist across wait_for_event calls
thread_local! {
    static EVENT_BUFFER: RefCell<VecDeque<debugger::Event>> = RefCell::new(VecDeque::new());
}

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

#[test]
fn test_remote_attach() -> eyre::Result<()> {
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

    // Give Python process time to start and bind to port
    thread::sleep(Duration::from_secs(1));

    // Check if child process is still alive
    if let Some(status) = child.try_wait().context("checking child status")? {
        let stderr = child.stderr.take().map(|mut s| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut s, &mut buf).ok();
            buf
        });
        eyre::bail!(
            "Python process exited early with status: {:?}\nstderr: {:?}",
            status,
            stderr
        );
    }

    let launch_args = debugger::AttachArguments {
        working_directory: cwd.clone(),
        port: Some(port),
        language: debugger::Language::DebugPy,
        path_mappings: None,
    };

    let debugger = Debugger::on_port(port, launch_args).context("creating debugger")?;
    let drx = debugger.events();

    let file_path = std::env::current_dir()
        .unwrap()
        .join("../../attach.py")
        .canonicalize()
        .context("invalid debug target")?;

    wait_for_event("initialised event", &drx, |e| {
        matches!(e, debugger::Event::Initialised)
    });

    let breakpoint_line = 9;
    debugger
        .add_breakpoint(&debugger::Breakpoint {
            path: file_path.clone(),
            line: breakpoint_line,
            ..Default::default()
        })
        .context("adding breakpoint")?;
    debugger.start().context("launching debugee")?;

    wait_for_event("running event", &drx, |e| {
        matches!(e, debugger::Event::Running)
    });

    let debugger::Event::Paused(ProgramState { paused_frame, .. }) =
        wait_for_event("paused event", &drx, |e| {
            matches!(e, debugger::Event::Paused { .. })
        })
    else {
        unreachable!();
    };

    assert!(matches!(
        paused_frame,
        PausedFrame {
            frame: StackFrame {
                source: Some(Source {
                    path: Some(file_path),
                    ..
                }),
                line: breakpoint_line,
                ..
            },
            ..
        } if file_path == file_path && breakpoint_line == breakpoint_line
    ));

    debugger.r#continue().context("resuming debugee")?;

    wait_for_event("terminated debuggee", &drx, |e| {
        matches!(e, debugger::Event::Ended)
    });

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

#[test]
fn test_debugger() -> eyre::Result<()> {
    let cwd = std::env::current_dir().unwrap();
    tracing::warn!(current_dir = ?cwd, "current_dir");
    let port = get_random_tcp_port().context("getting free port")?;

    let file_path = std::env::current_dir()
        .unwrap()
        .join("../../test.py")
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

    let breakpoint_line = 4;
    debugger
        .add_breakpoint(&debugger::Breakpoint {
            path: file_path.clone(),
            line: breakpoint_line,
            ..Default::default()
        })
        .context("adding breakpoint")?;
    debugger.start().context("launching debugee")?;

    wait_for_event("running event", &drx, |e| {
        matches!(e, debugger::Event::Running)
    });

    let debugger::Event::Paused(ProgramState { paused_frame, .. }) =
        wait_for_event("paused event", &drx, |e| {
            matches!(e, debugger::Event::Paused { .. })
        })
    else {
        unreachable!();
    };

    assert!(matches!(
        paused_frame,
        PausedFrame {
            frame: StackFrame {
                source: Some(Source {
                    path: Some(file_path),
                    ..
                }),
                line: breakpoint_line,
                ..
            },
            ..
        } if file_path == file_path && breakpoint_line == breakpoint_line
    ));

    debugger.r#continue().context("resuming debugee")?;

    wait_for_event("terminated debuggee", &drx, |e| {
        matches!(e, debugger::Event::Ended)
    });

    Ok(())
}

#[tracing::instrument(skip(rx, pred))]
fn wait_for_event<F>(
    message: &str,
    rx: &crossbeam_channel::Receiver<debugger::Event>,
    pred: F,
) -> debugger::Event
where
    F: Fn(&debugger::Event) -> bool,
{
    tracing::debug!("waiting for {message} event");
    let mut n = 0;

    loop {
        // First check if any buffered events match (using thread-local buffer)
        let buffered_event = EVENT_BUFFER.with(|buffer| {
            let mut buffer = buffer.borrow_mut();
            if let Some(pos) = buffer.iter().position(&pred) {
                Some(buffer.remove(pos).unwrap())
            } else {
                None
            }
        });

        if let Some(evt) = buffered_event {
            tracing::debug!(event = ?evt, "received expected event from buffer");
            return evt;
        }

        // Then receive new event from channel
        let evt = match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(evt) => evt,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                panic!("timeout waiting for {message} event after 10 seconds");
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                panic!("channel disconnected while waiting for {message} event");
            }
        };

        if n >= 100 {
            panic!("did not receive {message} event after 100 iterations");
        }

        if pred(&evt) {
            tracing::debug!(event = ?evt, "received expected event");
            return evt;
        } else {
            tracing::trace!(event = ?evt, "non-matching event, buffering for later");
            EVENT_BUFFER.with(|buffer| {
                buffer.borrow_mut().push_back(evt);
            });
        }
        n += 1;
    }
}
