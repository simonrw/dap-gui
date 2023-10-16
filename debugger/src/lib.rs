use anyhow::Result;
use std::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};
use transport::{Client, Received};

#[derive(Default)]
enum DebuggerState {
    #[default]
    Initialising,
}

impl DebuggerState {
    fn update_from(&mut self, r: Received) {}
    // fn on_event<F>(&mut self, _handler: F)
}

pub struct Debugger {
    client: Client,
    state: Arc<Mutex<DebuggerState>>,
    watchers: Vec<Sender<Received>>,
}

impl Debugger {
    pub fn new(client: Client, rx: Receiver<Received>) -> Self {
        let state: Arc<Mutex<DebuggerState>> = Default::default();

        // background watcher to poll for events
        let background_state = Arc::clone(&state);
        thread::spawn(move || {
            for msg in rx {
                background_state.lock().unwrap().update_from(msg);
            }
        });

        Self {
            client,
            state,
            watchers: Vec::new(),
        }
    }

    pub fn initialise(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn on_event<F>(&mut self, _handler: F) -> Result<()>
    where
        F: Fn(Received) -> Result<()>,
    {
        // let (tx, rx) = mpsc::channel();
        // self.watchers.push(tx);

        // thread::spawn(move || {

        // });

        todo!()
    }
}
