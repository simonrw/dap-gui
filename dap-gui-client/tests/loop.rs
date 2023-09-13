use std::{net::TcpStream, sync::mpsc};

use dap_gui_client::{
    events,
    requests::{self, Initialize, Launch},
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

    tracing::debug!(?reason, ?thread_id, ?hit_breakpoint_ids, "got stopped event");

    // restart
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

#[test]
fn test_initialize() {
    // TODO: automatic setup for running the server
    // for now assume the server is running
    init_test_logger();

    let (tx, _) = mpsc::channel();
    let mut client = default_client(tx);
    let req = requests::RequestBody::Initialize(Initialize {
        adapter_id: "dap gui".to_string(),
    });
    let res = client.send(req).unwrap();
    assert_eq!(res.success, true);
}

fn init_test_logger() {
    let _ = tracing_subscriber::fmt::try_init();
}

fn default_client(tx: mpsc::Sender<events::Event>) -> dap_gui_client::Client {
    let stream = TcpStream::connect("127.0.0.1:5678").unwrap();
    dap_gui_client::Client::new(stream, tx).unwrap()
}
