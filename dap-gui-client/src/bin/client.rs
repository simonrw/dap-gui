use std::io::{BufReader, BufWriter};
use std::net::TcpStream;
use std::sync::{Arc, Mutex, mpsc};
use std::collections::HashMap;
use std::thread;

use dap_gui_client::{Reader, Writer};

fn main() {
    let input_stream = TcpStream::connect("127.0.0.1:5678").unwrap();
    let output_stream = input_stream.try_clone().unwrap();

    let (tx, rx) = mpsc::channel();

    let store = Arc::new(Mutex::new(HashMap::new()));
    let mut reader = Reader::new(BufReader::new(input_stream), tx, Arc::clone(&store));
    let mut sender = Writer::new(BufWriter::new(output_stream), Arc::clone(&store));

    thread::spawn(move || {
        reader.poll_loop();
    });

    sender.send_initialize();
    for msg in rx {
        dbg!(msg);
    }
}
