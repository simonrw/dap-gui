use std::{net::TcpStream, time::Duration};
use std::sync::mpsc;
use std::thread;

use dap_gui_client::{Client, StreamReader};

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5678").unwrap();

    // background thread for messages
    let (tx, rx) = mpsc::channel();


    let mut reader = StreamReader::new(stream.try_clone().unwrap());
    let _handle = thread::spawn(move || {
        reader.receive_events(tx);
    });

    let client = Client::new(&mut stream, rx);
    client.send_initialize();
    client.mainloop();
}
