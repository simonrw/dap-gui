use anyhow::Result;
use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    thread,
};
use transport::{requests, Client, Received};

#[derive(Default)]
enum DebuggerState {
    #[default]
    Initialising,
}

impl DebuggerState {
    fn update_from(&mut self, _r: &Received) {}
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
        thread::spawn(move || {
            for msg in rx {
                background_state.lock().unwrap().update_from(&msg);
            }
        });

        Self {
            client,
            state,
            event_handlers,
        }
    }

    pub fn initialise(&mut self) -> Result<()> {
        // send initialize
        let req = requests::RequestBody::Initialize(requests::Initialize {
            adapter_id: "dap gui".to_string(),
            lines_start_at_one: false,
            path_format: requests::PathFormat::Path,
            supports_start_debugging_request: true,
            supports_variable_type: true,
            supports_variable_paging: true,
            supports_progress_reporting: true,
            supports_memory_event: true,
        });
        self.client.send(req).unwrap();

        // send launch
        let state = self.state.lock().unwrap();

        Ok(())
    }

    pub fn on_state_change(&mut self, handler: F) {
        let mut event_handlers = self.event_handlers.lock().unwrap();
        event_handlers.push(Box::new(handler));
    }
}
