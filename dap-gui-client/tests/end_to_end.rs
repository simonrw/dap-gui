use anyhow::{Context, Result};
use std::{
    io::{BufRead, BufReader},
    net::TcpStream,
    path::PathBuf,
    process::Stdio,
    sync::mpsc,
    thread,
};
use tracing_subscriber::EnvFilter;

use dap_gui_client::{
    bindings::get_random_tcp_port,
    events,
    requests::{self, DebugpyLaunchArguments, Initialize, Launch, LaunchArguments, PathFormat},
    responses, Received,
};

// Function to start the server in the background
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

        let stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        let client = dap_gui_client::Client::new(stream, tx).unwrap();

        // initialize
        let req = requests::RequestBody::Initialize(Initialize {
            adapter_id: "dap gui".to_string(),
            lines_start_at_one: false,
            path_format: PathFormat::Path,
            supports_start_debugging_request: true,
            supports_variable_type: true,
            supports_variable_paging: true,
            supports_progress_reporting: true,
            supports_memory_event: true,
        });
        client.send(req).unwrap();

        // launch
        client
            .send(requests::RequestBody::Launch(Launch {
                program: PathBuf::from("./test.py"),
                launch_arguments: Some(LaunchArguments::Debugpy(DebugpyLaunchArguments {
                    just_my_code: true,
                    // console: "integratedTerminal".to_string(),
                    // tests are run from the package they are from
                    cwd: std::env::current_dir().unwrap().join(".."),
                    show_return_value: true,
                    debug_options: vec!["DebugStdLib".to_string(), "ShowReturnValue".to_string()],
                    stop_on_entry: false,
                    is_output_redirected: false,
                })),
            }))
            .unwrap();

        // wait for initialized event
        let _initialized_event = wait_for_event("initialized", &rx, |e| {
            matches!(e, events::Event::Initialized { .. })
        });

        // set function breakpoints
        let req = requests::RequestBody::SetFunctionBreakpoints(requests::SetFunctionBreakpoints {
            breakpoints: vec![requests::Breakpoint {
                name: "main".to_string(),
            }],
        });
        client.send(req).unwrap();
        let _ = wait_for_response("setFunctionBreakpoints", &rx, |r| {
            matches!(r, responses::ResponseBody::SetFunctionBreakpoints { .. })
        });

        // configuration done
        let req = requests::RequestBody::ConfigurationDone;
        client.send(req).unwrap();
        let _ = wait_for_response("configurationDone", &rx, |r| {
            matches!(r, responses::ResponseBody::ConfigurationDone)
        });

        // wait for stopped event
        let events::Event::Stopped(events::StoppedEventBody {
            reason,
            thread_id,
            hit_breakpoint_ids,
            ..
        }) = wait_for_event("stopped", &rx, |e| {
            matches!(e, events::Event::Stopped { .. })
        })
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
        client.send(req).unwrap();
        let _ = wait_for_response("threads", &rx, |r| {
            matches!(r, responses::ResponseBody::Threads(_))
        });

        // fetch stack info
        let req = requests::RequestBody::StackTrace(requests::StackTrace {
            thread_id,
            ..Default::default()
        });
        client.send(req).unwrap();

        let responses::ResponseBody::StackTrace(responses::StackTraceResponse { stack_frames }) =
            wait_for_response("stackTrace", &rx, |r| {
                matches!(
                    r,
                    responses::ResponseBody::StackTrace(responses::StackTraceResponse { .. })
                )
            })
        else {
            unreachable!()
        };

        for frame in stack_frames {
            // scopes
            let req = requests::RequestBody::Scopes(requests::Scopes { frame_id: frame.id });
            client.send(req).unwrap();

            let responses::ResponseBody::Scopes(responses::ScopesResponse { scopes }) =
                wait_for_response("scopes", &rx, |r| {
                    matches!(
                        r,
                        responses::ResponseBody::Scopes(responses::ScopesResponse { .. })
                    )
                })
            else {
                unreachable!()
            };

            // variables
            for scope in scopes {
                let req = requests::RequestBody::Variables(requests::Variables {
                    variables_reference: scope.variables_reference,
                });
                client.send(req).unwrap();

                let responses::ResponseBody::Variables(responses::VariablesResponse { .. }) =
                    wait_for_response("variables", &rx, |r| {
                        matches!(
                            r,
                            responses::ResponseBody::Variables(responses::VariablesResponse { .. })
                        )
                    })
                else {
                    unreachable!()
                };
            }
        }

        // continue
        let req = requests::RequestBody::Continue(requests::Continue {
            thread_id,
            single_thread: false,
        });
        tracing::debug!(?req, "sending continue request");
        client.send(req).unwrap();
        let _ = wait_for_response("continue", &rx, |r| {
            matches!(r, responses::ResponseBody::Continue(_))
        });

        wait_for_event("terminated", &rx, |e| {
            matches!(e, events::Event::Terminated)
        });

        // terminate
        let req = requests::RequestBody::Terminate(requests::Terminate {
            restart: Some(false),
        });
        client.send(req).unwrap();
        let _ = wait_for_response("terminate", &rx, |r| {
            matches!(r, responses::ResponseBody::Terminate)
        });

        // disconnect
        let req = requests::RequestBody::Disconnect(requests::Disconnect {
            terminate_debugee: true,
        });
        client.send(req).unwrap();
        let _ = wait_for_response("disconnect", &rx, |r| {
            matches!(r, responses::ResponseBody::Disconnect)
        });
        Ok(())
    })
}

#[tracing::instrument(skip(rx, pred))]
fn wait_for_response<F>(
    message: &str,
    rx: &mpsc::Receiver<Received>,
    pred: F,
) -> responses::ResponseBody
where
    F: Fn(&responses::ResponseBody) -> bool,
{
    tracing::debug!("waiting for response");
    for (n, msg) in rx.iter().enumerate() {
        if n >= 100 {
            panic!("did not receive response");
        }

        if let Received::Response(_, response) = msg {
            assert!(response.success);
            if let Some(body) = response.body {
                if pred(&body) {
                    tracing::debug!(response = ?body, "received expected response");
                    return body;
                } else {
                    tracing::trace!(event = ?body, "non-matching event");
                }
            }
        }
    }

    unreachable!()
}

#[tracing::instrument(skip(rx, pred))]
fn wait_for_event<F>(message: &str, rx: &mpsc::Receiver<Received>, pred: F) -> events::Event
where
    F: Fn(&events::Event) -> bool,
{
    tracing::debug!("waiting for event");
    for (n, msg) in rx.iter().enumerate() {
        if n >= 100 {
            panic!("did not receive event");
        }

        if let Received::Event(evt) = msg {
            if pred(&evt) {
                tracing::debug!(event = ?evt, "received expected event");
                return evt;
            } else {
                tracing::trace!(event = ?evt, "non-matching event");
            }
        }
    }

    unreachable!()
}

fn init_test_logger() {
    let in_ci = std::env::var("CI")
        .map(|val| val == "true")
        .unwrap_or(false);

    if atty::is(atty::Stream::Stderr) || in_ci {
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
