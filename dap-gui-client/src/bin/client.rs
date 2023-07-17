use std::io::{BufReader, BufWriter};
use std::sync::mpsc;
use std::thread;
use std::{net::TcpStream, time::Duration};

use dap_gui_client::Client2;

fn main() {
    let mut input_stream = TcpStream::connect("127.0.0.1:5678").unwrap();
    let mut output_stream = input_stream.try_clone().unwrap();

    let mut client = Client2::new(BufReader::new(input_stream), BufWriter::new(output_stream));
    client.send_initialize();
    loop {
        client.receive();
    }

    return;

    /*
    // background thread for messages
    let (tx, rx) = mpsc::channel();

    let mut reader = StreamReader::new(stream.try_clone().unwrap());
    let _handle = thread::spawn(move || {
        reader.receive_events(tx);
    });

    let client = Client::new(&mut stream, rx);
    client.send_initialize();
    client.mainloop();
    */
}
