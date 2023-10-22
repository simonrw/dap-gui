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
    fn update_from(&mut self, r: &Received) {}
    // fn on_event<F>(&mut self, _handler: F)
}

pub struct Debugger<F> {
    client: Client,
    state: Arc<Mutex<DebuggerState>>,
    event_handler: Arc<Mutex<Option<Box<F>>>>,
}

impl<F> Debugger<F>
where
    F: FnMut(&Received) + Send + Sync + 'static,
{
    pub fn new(client: Client, rx: Receiver<Received>) -> Self {
        let state: Arc<Mutex<DebuggerState>> = Default::default();
        let event_handler: Arc<Mutex<Option<Box<F>>>> = Arc::new(Mutex::new(None));

        // background watcher to poll for events
        let background_state = Arc::clone(&state);
        let background_event_handler = Arc::clone(&event_handler);
        thread::spawn(move || {
            for msg in rx {
                background_state.lock().unwrap().update_from(&msg);
                if let Some(ref mut handler) = *background_event_handler.lock().unwrap() {
                    handler(&msg)
                }
            }
        });

        Self {
            client,
            state,
            event_handler,
        }
    }

    pub fn initialise(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn on_event(&mut self, handler: F) {
        self.event_handler = Arc::new(Mutex::new(Some(Box::new(handler))));
    }
}
