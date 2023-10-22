use anyhow::Result;
use std::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, RwLock,
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
    event_handlers: Arc<Mutex<Vec<Box<F>>>>,
}

impl<F> Debugger<F>
where
    F: FnMut(&Received) + Send + Sync + 'static,
{
    pub fn new(client: Client, rx: Receiver<Received>) -> Self {
        let state: Arc<Mutex<DebuggerState>> = Default::default();
        let event_handlers: Arc<Mutex<Vec<Box<F>>>> = Arc::new(Mutex::new(Vec::new()));

        // background watcher to poll for events
        let background_state = Arc::clone(&state);
        let background_event_handlers = Arc::clone(&event_handlers);
        thread::spawn(move || {
            for msg in rx {
                background_state.lock().unwrap().update_from(&msg);
                let mut event_handlers = background_event_handlers.lock().unwrap();
                for event_handler in event_handlers.iter_mut() {
                    event_handler(&msg)
                }
            }
        });

        Self {
            client,
            state,
            event_handlers,
        }
    }

    pub fn initialise(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn on_event(&mut self, handler: F) {
        let mut event_handlers = self.event_handlers.lock().unwrap();
        event_handlers.push(Box::new(handler));
    }
}
