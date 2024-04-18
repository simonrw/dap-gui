use std::{path::PathBuf, time::Duration};

use dap_codec::dap::{
    events,
    requests::{self, Command},
    responses, types,
};
use eyre::WrapErr;
use tokio::{net::TcpListener, sync::mpsc};

use server::for_implementation_on_port;
use tracing_subscriber::EnvFilter;
use transport::Client;

#[tokio::test]
async fn test_loop() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let port = get_random_tcp_port().await.wrap_err("getting free port")?;
    let _server = for_implementation_on_port(server::Implementation::Debugpy, port)
        .wrap_err("creating server process")?;

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .wrap_err("connecting to server")?;

    // events channel
    let (tx, mut events_rx) = mpsc::channel(10);

    let client = Client::new(stream, tx);

    // initialise
    let res = client
        .send(Command::Initialize(requests::InitializeArguments {
            adapter_id: "test".to_string(),
            ..Default::default()
        }))
        .await
        .expect("sending initialize request");
    assert!(res.success);

    // launch
    client
        .execute(Command::Launch(requests::LaunchRequestArguments {
            additional_data: Some(serde_json::json!({
                "name": "Python: Current File",
                "type": "python",
                "request": "launch",
                "program": PathBuf::from("./test.py"),
                "justMyCode": false,
                "cwd": std::env::current_dir().unwrap().join(".."),
                "showReturnValue": true,
            })),
            ..Default::default()
        }))
        .await
        .expect("executing launch request");

    wait_for_event(&mut events_rx, Duration::from_secs(2), |e| {
        matches!(e, events::Event::Initialized { .. })
    })
    .await;

    let res = client
        .send(Command::SetFunctionBreakpoints(
            requests::SetFunctionBreakpointsArguments {
                breakpoints: vec![types::FunctionBreakpoint {
                    name: "main".to_string(),
                    ..Default::default()
                }],
            },
        ))
        .await
        .expect("sending set function breakpoints request");
    assert!(res.success);

    // configuration done
    let res = client
        .send(Command::ConfigurationDone)
        .await
        .expect("sending configuration done request");
    assert!(res.success);

    // wait for stopped event
    let event = wait_for_event(&mut events_rx, Duration::from_secs(2), |e| {
        matches!(e, events::Event::Stopped(_))
    })
    .await;

    let events::Event::Stopped(events::StoppedEventBody {
        thread_id: Some(thread_id),
        ..
    }) = event
    else {
        panic!("unexpected event");
    };

    // fetch thread info
    let res = client
        .send(Command::Threads)
        .await
        .expect("sending threads request");
    let Some(responses::ResponseBody::Threads(responses::ThreadsResponse { threads })) = res.body
    else {
        panic!("no thread info");
    };
    assert_eq!(threads.len(), 1);

    // fetch stack info
    let res = client
        .send(Command::StackTrace(requests::StackTraceArguments {
            thread_id,
            ..Default::default()
        }))
        .await
        .expect("sending stacktrace request");
    assert!(res.success);

    let Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
        stack_frames,
        ..
    })) = res.body
    else {
        panic!("no stack info");
    };

    for frame in stack_frames {
        // fetch scopes
        let res = client
            .send(Command::Scopes(requests::ScopesArguments {
                frame_id: frame.id,
            }))
            .await
            .unwrap_or_else(|_| panic!("sending scopes request for frame {frame:?}"));
        assert!(res.success);
        let Some(responses::ResponseBody::Scopes(responses::ScopesResponse { scopes })) = res.body
        else {
            panic!("no scopes");
        };

        // fetch variables
        for scope in scopes {
            let res = client
                .send(Command::Variables(requests::VariablesArguments {
                    variables_reference: scope.variables_reference,
                    ..Default::default()
                }))
                .await
                .unwrap_or_else(|_| panic!("sending variables request for scope {scope:?}"));
            assert!(res.success);

            let Some(responses::ResponseBody::Variables(responses::VariablesResponse {
                ..
                // variables,
            })) = res.body
            else {
                panic!("no variables");
            };

            // for variable in variables {
            //     let res = client
            //         .send(Command::Evaluate(requests::EvaluateArguments {
            //             expression: variable.name.clone(),
            //             frame_id: Some(frame.id),
            //             context: None,
            //             format: Some(PathFormat::Auto),
            //         }))
            //         .await;
            //     assert!(res.success);
            //     let Some(responses::ResponseBody::Evaluate(responses::EvaluateResponse {
            //         result,
            //         ..
            //     })) = res.body
            //     else {
            //         panic!("no evaluation result");
            //     };
            //     assert_eq!(result, variable.value);
            // }
        }
    }

    // continue
    let res = client
        .send(Command::Continue(requests::ContinueArguments {
            thread_id,
            single_thread: Some(false),
        }))
        .await
        .expect("sending continue request");
    assert!(res.success);

    wait_for_event(&mut events_rx, Duration::from_secs(2), |e| {
        matches!(e, events::Event::Continued(_))
    })
    .await;

    wait_for_event(&mut events_rx, Duration::from_secs(2), |e| {
        matches!(e, events::Event::Terminated(_))
    })
    .await;

    // terminate
    let res = client
        .send(Command::Terminate(requests::TerminateArguments {
            ..Default::default()
        }))
        .await
        .expect("sending terminate request");
    assert!(res.success);

    // disconnect
    let res = client
        .send(Command::Disconnect(requests::DisconnectArguments {
            terminate_debuggee: Some(true),
            ..Default::default()
        }))
        .await
        .expect("sending disconnect request");
    assert!(res.success);

    Ok(())
}

async fn wait_for_event<F>(
    rx: &mut mpsc::Receiver<events::Event>,
    timeout: Duration,
    pred: F,
) -> events::Event
where
    F: Fn(&events::Event) -> bool,
{
    let poll_fut = async {
        let mut n = 0;
        loop {
            tokio::select! {
                evt = rx.recv() => {
                    let evt = evt.unwrap();
                    if n >= 100 {
                        panic!("did not receive event");
                    }

                    if pred(&evt) {
                        return evt;
                    }
                    n += 1;
                }
            }
        }
    };

    tokio::time::timeout(timeout, poll_fut)
        .await
        .expect("didn't receive message")
}
