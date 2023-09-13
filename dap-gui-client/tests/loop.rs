use std::{net::TcpStream, sync::mpsc};

use dap_gui_client::{
    events,
    requests::{self, Initialize, Launch},
    types,
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
    let req = requests::RequestBody::Initialize(Initialize {
        adapter_id: "dap gui".to_string(),
    });
    let res = client.send(req).unwrap();
    assert!(res.success);

    client
        .emit(requests::RequestBody::Launch(Launch {
            program: "./test.py".to_string(),
        }))
        .unwrap();

    let _ = wait_for_event(&rx, |e| matches!(e, events::Event::Initialized));

    let req = requests::RequestBody::SetFunctionBreakpoints(requests::SetFunctionBreakpoints {
        breakpoints: vec![requests::Breakpoint {
            name: "main".to_string(),
        }],
    });
    let res = client.send(req).unwrap();
    assert!(res.success);

    let req = requests::RequestBody::Continue(requests::Continue {
        thread_id: todo!(),
        single_thread: todo!(),
    });
    let res = client.send(req).unwrap();
    assert!(res.success);

    let stopped_event = wait_for_event(&rx, |e| matches!(e, events::Event::Stopped { .. }));

    // terminate
    let res = client
        .send(requests::RequestBody::Disconnect(requests::Disconnect {
            terminate_debugee: true,
        }))
        .unwrap();
    assert!(res.success);
}

fn wait_for_event<F>(rx: &mpsc::Receiver<events::Event>, pred: F) -> events::Event
where
    F: Fn(&events::Event) -> bool,
{
    let mut n = 0;
    for msg in rx {
        if n >= 10 {
            panic!("did not receive event");
        }

        if pred(&msg) {
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
