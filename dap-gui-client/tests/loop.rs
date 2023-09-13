use std::{net::TcpStream, sync::mpsc};

use dap_gui_client::{
    events,
    requests::{self, Initialize, Launch},
    responses,
};

// Loop
// Initialize
// Launch
// Set function breakpoints
// Continue
#[test]
fn test_loop() {
    init_test_logger();

    let (tx, rx) = mpsc::channel();
    let mut client = default_client(tx);

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

    let Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse { stack_frames })) =
        res.body
    else {
        unreachable!();
    };
    for frame in stack_frames {
        // scopes
        let req = requests::RequestBody::Scopes(requests::Scopes { frame_id: frame.id });
        let res = client.send(req).unwrap();
        assert!(res.success);

        let Some(responses::ResponseBody::Scopes(responses::ScopesResponse { scopes })) = res.body
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
    let _ = tracing_subscriber::fmt::try_init();
}

fn default_client(tx: mpsc::Sender<events::Event>) -> dap_gui_client::Client {
    let stream = TcpStream::connect("127.0.0.1:5678").unwrap();
    dap_gui_client::Client::new(stream, tx).unwrap()
}
