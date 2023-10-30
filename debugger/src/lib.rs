//! Using the state machine concept from
//! https://hoverbear.org/blog/rust-state-machine-pattern/#generically-sophistication
use crossbeam_channel::Receiver;
use transport::{Client, Received};

mod waiters;

// Debugger States
pub struct Running;
pub struct Paused;

pub struct Debugger<S> {
    #[allow(dead_code)]
    client: Client,
    #[allow(dead_code)]
    state: S,

    event_handlers: Vec<Box<dyn FnMut(DebuggerEvent)>>,
}

// General functions
impl<S> Debugger<S> {
    pub fn on_state_change<F>(&mut self, f: F)
    where
        F: FnMut(DebuggerEvent) + 'static,
    {
        self.event_handlers.push(Box::new(f));
    }
}

impl Debugger<Running> {
    fn entered_event(&mut self) {
        for handler in self.event_handlers.iter_mut() {
            handler(DebuggerEvent::Running)
        }
    }
}

impl Debugger<Paused> {}

#[derive(Clone)]
pub enum DebuggerEvent {
    Running,
}

pub fn initialise(client: Client, _rx: Receiver<Received>) -> Debugger<Running> {
    // set up stuff

    // launch the debuggee

    // return the debugger in the running state
    let mut state = Debugger {
        client,
        state: Running {},
        event_handlers: Vec::new(),
    };

    // announce the debugger running state
    state.entered_event();

    state
}
