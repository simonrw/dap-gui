use std::{
    sync::{Arc, Mutex},
    thread,
};

use anyhow::Context;
use spmc::Receiver;
use transport::{
    requests::{self, Disconnect, Initialize, PathFormat},
    Client,
};

use crate::{
    internals::{DebuggerInternals, FileSource},
    state::{self, DebuggerState},
    types, Event,
};

pub struct Debugger {
    events: spmc::Receiver<transport::events::Event>,
    internals: Arc<Mutex<DebuggerInternals>>,
}

impl Debugger {
    #[tracing::instrument(skip(client, events, publisher))]
    pub fn new(
        client: Client,
        events: Receiver<transport::events::Event>,
        mut publisher: Option<spmc::Sender<Event>>,
    ) -> anyhow::Result<Self> {
        tracing::debug!("creating new client");

        if let Some(ref mut tx) = publisher {
            let _ = tx.send(Event::Uninitialised);
        }

        let events2 = events.clone();

        let internals = Arc::new(Mutex::new(DebuggerInternals::new(client, publisher)));

        // background thread reading transport events, and handling the event with our internal state
        let background_internals = Arc::clone(&internals);
        thread::spawn(move || loop {
            let event = events.recv().unwrap();
            background_internals.lock().unwrap().on_event(event);
        });
        Ok(Self {
            events: events2,
            internals,
        })
    }

    fn emit(&self, event: Event) {
        self.internals.lock().unwrap().emit(event)
    }

    pub fn initialise(&self, launch_arguments: state::LaunchArguments) -> anyhow::Result<()> {
        // initialise
        let req = requests::RequestBody::Initialize(Initialize {
            adapter_id: "dap gui".to_string(),
            lines_start_at_one: false,
            path_format: PathFormat::Path,
            supports_start_debugging_request: true,
            supports_variable_type: true,
            supports_variable_paging: true,
            supports_progress_reporting: true,
            supports_memory_event: true,
        });

        let DebuggerInternals { ref client, .. } = *self.internals.lock().unwrap();

        // TODO: deal with capabilities from the response
        let _ = client.send(req).context("sending initialize event")?;

        // send launch event
        let req = launch_arguments.to_request();
        client.execute(req).context("sending launch request")?;
        Ok(())
    }

    pub fn add_breakpoint(&self, breakpoint: types::Breakpoint) -> types::BreakpointId {
        let mut internals = self.internals.lock().unwrap();
        internals.add_breakpoint(breakpoint)
    }

    pub fn launch(&self) -> anyhow::Result<()> {
        let mut internals = self.internals.lock().unwrap();
        let _ = internals
            .client
            .send(requests::RequestBody::ConfigurationDone)
            .context("completing configuration")?;
        internals.set_state(DebuggerState::Running);
        Ok(())
    }

    /// Resume execution of the debugee
    pub fn r#continue(&self) -> anyhow::Result<()> {
        let internals = self.internals.lock().unwrap();
        match internals.current_thread_id {
            Some(thread_id) => {
                internals
                    .client
                    .execute(requests::RequestBody::Continue(requests::Continue {
                        thread_id,
                        single_thread: false,
                    }))
                    .context("sending continue request")?;
            }
            None => anyhow::bail!("logic error: no current thread id"),
        }
        Ok(())
    }

    pub fn with_current_source<F>(&self, f: F)
    where
        F: Fn(Option<&FileSource>),
    {
        let internals = self.internals.lock().unwrap();
        f(internals.current_source.as_ref())
    }

    fn execute(&self, body: requests::RequestBody) -> anyhow::Result<()> {
        self.internals.lock().unwrap().client.execute(body)
    }

    pub fn wait_for_transport_event<F>(&self, pred: F) -> transport::events::Event
    where
        F: Fn(&transport::events::Event) -> bool,
    {
        let mut n = 0;
        loop {
            let evt = self.events.recv().unwrap();
            if n >= 100 {
                panic!("did not receive event");
            }

            if pred(&evt) {
                tracing::debug!(event = ?evt, "received expected event");
                return evt;
            } else {
                tracing::trace!(event = ?evt, "non-matching event");
            }
            n += 1;
        }
    }
}

impl Drop for Debugger {
    fn drop(&mut self) {
        tracing::debug!("dropping debugger");
        self.execute(requests::RequestBody::Disconnect(Disconnect {
            terminate_debugee: true,
        }))
        .unwrap();
    }
}
