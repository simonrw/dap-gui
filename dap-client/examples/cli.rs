use std::{
    collections::HashMap,
    io::BufReader,
    net::TcpStream,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use anyhow::{Context, Result};
use dap_client::{Reader, Writer, WriterProxy};

fn main() -> Result<()> {
    let input_stream = TcpStream::connect("127.0.0.1:5678").context("connecting to DAP server")?;
    let output_stream = input_stream.try_clone().unwrap();

    let (tx, rx) = mpsc::channel();
    let store = Store::new();
    let (rw_tx, rw_rx) = mpsc::channel();
    let mut sender = Writer::new(output_stream, Arc::clone(&store), rw_tx);
    let mut reader = Reader::new(BufReader::new(input_stream), tx, Arc::clone(&store), rw_rx);
    let writer_proxy = sender.handle();

    thread::spawn(move || {
        reader.poll_loop();
    });

    Ok(())
}
