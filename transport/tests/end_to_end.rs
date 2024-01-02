use eyre::{Result, WrapErr};
use server::for_implementation_on_port;
use std::{io::IsTerminal, net::TcpStream, path::PathBuf};
use tracing_subscriber::EnvFilter;

use transport::{
    bindings::get_random_tcp_port,
    events,
    requests::{self, DebugpyLaunchArguments, Initialize, Launch, LaunchArguments, PathFormat},
    responses,
};

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

    let (tx, rx) = crossbeam_channel::unbounded();
    let port = get_random_tcp_port().context("getting free port")?;
    let _server = for_implementation_on_port(server::Implementation::Debugpy, port)
        .context("creating server process")?;
    let span = tracing::debug_span!("with_server", %port);
    let _guard = span.enter();

    let stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    let client = transport::Client::new(stream, tx).unwrap();

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
        .execute(requests::RequestBody::Launch(Launch {
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
    let _ = client.send(req).unwrap();

    // configuration done
    let req = requests::RequestBody::ConfigurationDone;
    let _ = client.send(req).unwrap();

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
    let _ = client.send(req).unwrap();

    // fetch stack info
    let req = requests::RequestBody::StackTrace(requests::StackTrace {
        thread_id,
        ..Default::default()
    });
    let Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse { stack_frames })) =
        client.send(req).unwrap()
    else {
        unreachable!()
    };

    for frame in stack_frames {
        // scopes
        let req = requests::RequestBody::Scopes(requests::Scopes { frame_id: frame.id });

        let Some(responses::ResponseBody::Scopes(responses::ScopesResponse { scopes })) =
            client.send(req).unwrap()
        else {
            unreachable!()
        };

        // variables
        for scope in scopes {
            let req = requests::RequestBody::Variables(requests::Variables {
                variables_reference: scope.variables_reference,
            });

            let _ = client.send(req).unwrap();
        }
    }

    // continue
    let req = requests::RequestBody::Continue(requests::Continue {
        thread_id,
        single_thread: false,
    });
    tracing::debug!(?req, "sending continue request");
    let _ = client.send(req).unwrap();

    wait_for_event("continued", &rx, |e| {
        matches!(e, events::Event::Continued(_))
    });

    wait_for_event("terminated", &rx, |e| {
        matches!(e, events::Event::Terminated)
    });

    // terminate
    let req = requests::RequestBody::Terminate(requests::Terminate {
        restart: Some(false),
    });
    let _ = client.send(req).unwrap();

    // disconnect
    let req = requests::RequestBody::Disconnect(requests::Disconnect {
        terminate_debugee: true,
    });
    let _ = client.send(req).unwrap();
    Ok(())
}

#[tracing::instrument(skip(rx, pred))]
fn wait_for_event<F>(
    message: &str,
    rx: &crossbeam_channel::Receiver<events::Event>,
    pred: F,
) -> events::Event
where
    F: Fn(&events::Event) -> bool,
{
    tracing::debug!("waiting for event");
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
