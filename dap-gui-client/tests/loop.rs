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

    let (tx, _) = mpsc::channel();
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
    // assert!(res.success);
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
