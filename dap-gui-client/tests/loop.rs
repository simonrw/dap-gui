use std::{net::TcpStream, thread};

use dap_gui_client::requests::{self, Initialize};
use dap_gui_client::EventHandler;

// Loop
// Initialize
// Launch
// Set function breakpoints
// Continue
#[test]
fn test_initialize() {
    // TODO: automatic setup for running the server
    // for now assume the server is running
    init_test_logger();

    let stream = TcpStream::connect("127.0.0.1:5678").unwrap();
    let mut client = dap_gui_client::Client::new(stream).unwrap();
    let mut reader = client.reader(|_e| Ok::<_, anyhow::Error>(()));
    thread::spawn(move || {
        if let Err(e) = reader.run_poll_loop() {
            eprintln!("error running poll loop: {e}");
        }
    });

    let req = requests::RequestBody::Initialize(Initialize {
        adapter_id: "dap gui".to_string(),
    });
    let res = client.send(req).unwrap();
    assert_eq!(res.success, true);
}

fn init_test_logger() {
    let _ = tracing_subscriber::fmt::try_init();
}
