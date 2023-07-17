use std::io::{BufReader, BufWriter};
use std::net::TcpStream;

use dap_gui_client::Client2;

fn main() {
    let input_stream = TcpStream::connect("127.0.0.1:5678").unwrap();
    let output_stream = input_stream.try_clone().unwrap();

    let mut client = Client2::new(BufReader::new(input_stream), BufWriter::new(output_stream));
    client.send_initialize();
    loop {
        client.receive();
    }
}
