use std::{
    collections::HashMap,
    io::BufReader,
    net::{TcpStream, ToSocketAddrs},
    sync::{mpsc, Arc, Mutex},
    thread,
};

use anyhow::{Context, Result};

use dap_gui_client::{Reader, Reply, Writer, WriterProxy};

pub struct Debugger {
    sender: WriterProxy,
    // internal: DebuggerInternal,
}

impl Debugger {
    pub fn new<'a, F>(
        scope: &'a thread::Scope<'a, '_>,
        addr: impl ToSocketAddrs,
        mut callback: F,
    ) -> Result<Self>
    where
        F: FnMut(Reply) + Send + 'a,
    {
        let input_stream = TcpStream::connect(addr).context("connecting to DAP server")?;
        let output_stream = input_stream.try_clone().unwrap();

        let (tx, rx) = mpsc::channel();
        let store = Arc::new(Mutex::new(HashMap::new()));
        let mut reader = Reader::new(BufReader::new(input_stream), tx, Arc::clone(&store));
        let mut sender = Writer::new(output_stream, Arc::clone(&store));
        let (wtx, wrx) = mpsc::channel();
        let writer_proxy = WriterProxy::new(wtx);

        scope.spawn(move || {
            for msg in wrx {
                match sender.send(msg) {
                    Ok(_) => {}
                    Err(e) => tracing::warn!("sending message to writer: {e}"),
                }
            }
        });

        scope.spawn(move || {
            reader.poll_loop();
        });

        scope.spawn(move || {
            for msg in rx {
                callback(msg);
            }
        });

        Ok(Self {
            sender: writer_proxy,
        })
    }
}

enum DebuggerInternal {}
