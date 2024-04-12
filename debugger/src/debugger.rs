use std::{
    sync::{Arc, Mutex},
    thread,
};

use eyre::WrapErr;
use server::Implementation;
use tokio::runtime::Runtime;
use transport::{types::StackFrameId, DEFAULT_DAP_PORT};

use crate::{
    internals::{DebuggerInternals, FileSource},
    state,
    types::{self, EvaluateResult},
    Event,
};

pub enum InitialiseArguments {
    Launch(state::LaunchArguments),
    Attach(state::AttachArguments),
}

impl From<state::LaunchArguments> for InitialiseArguments {
    fn from(value: state::LaunchArguments) -> Self {
        Self::Launch(value)
    }
}

impl From<state::AttachArguments> for InitialiseArguments {
    fn from(value: state::AttachArguments) -> Self {
        Self::Attach(value)
    }
}

pub struct Debugger {
    internals: Arc<Mutex<DebuggerInternals>>,
    rx: crossbeam_channel::Receiver<Event>,
    _runtime: Runtime,
}

impl Debugger {
    #[tracing::instrument(skip(initialise_arguments))]
    pub fn on_port(
        port: u16,
        initialise_arguments: impl Into<InitialiseArguments>,
    ) -> eyre::Result<Self> {
        tracing::debug!("creating new client");

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("creating tokio runtime");

        // notify our subscribers
        let (tx, rx) = crossbeam_channel::unbounded();
        let _ = tx.send(Event::Uninitialised);

        let args: InitialiseArguments = initialise_arguments.into();
        let internals_rx = rx.clone();
        let (mut internals, events) = match &args {
            InitialiseArguments::Launch(state::LaunchArguments { language, .. }) => {
                // let implementation = language.into();
                let implementation: Implementation = match language {
                    crate::Language::DebugPy => Implementation::Debugpy,
                    crate::Language::Delve => Implementation::Delve,
                };

                let s = server::for_implementation_on_port(implementation, port)
                    .context("creating background server process")?;
                let stream = runtime
                    .block_on(tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")))
                    .wrap_err("connecting to server")?;

                let (ttx, trx) = crossbeam_channel::unbounded();
                let client =
                    transport::ClientHandle::new(stream).context("creating transport client")?;

                let internals =
                    DebuggerInternals::new(client, tx, Some(s), runtime.handle().to_owned());
                (internals, trx)
            }
            InitialiseArguments::Attach(_) => {
                let stream = runtime
                    .block_on(tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")))
                    .wrap_err("connecting to server")?;

                let (ttx, trx) = crossbeam_channel::unbounded();
                let client =
                    transport::ClientHandle::new(stream).context("creating transport client")?;

                let internals =
                    DebuggerInternals::new(client, tx, None, runtime.handle().to_owned());
                (internals, trx)
            }
        };

        internals.initialise(args).context("initialising")?;

        let internals = Arc::new(Mutex::new(internals));

        // background thread reading transport events, and handling the event with our internal state
        let background_internals = Arc::clone(&internals);
        let background_events = events.clone();
        thread::spawn(move || loop {
            let event = background_events.recv().unwrap();
            background_internals.lock().unwrap().on_event(event);
        });

        Ok(Self {
            internals,
            rx: internals_rx,
            _runtime: runtime,
        })
    }
    #[tracing::instrument(skip(initialise_arguments))]
    pub fn new(initialise_arguments: impl Into<InitialiseArguments>) -> eyre::Result<Self> {
        Self::on_port(DEFAULT_DAP_PORT, initialise_arguments)
    }

    pub fn events(&self) -> crossbeam_channel::Receiver<Event> {
        self.rx.clone()
    }

    pub fn add_breakpoint(
        &self,
        breakpoint: &types::Breakpoint,
    ) -> eyre::Result<types::BreakpointId> {
        let mut internals = self.internals.lock().unwrap();
        internals.add_breakpoint(breakpoint)
    }

    pub fn launch(&self) -> eyre::Result<()> {
        let mut internals = self.internals.lock().unwrap();
        internals.launch().wrap_err("launching")?;
        Ok(())
    }

    pub fn evaluate(
        &self,
        input: &str,
        frame_id: StackFrameId,
    ) -> eyre::Result<Option<EvaluateResult>> {
        let mut internals = self.internals.lock().unwrap();
        internals.evaluate(input, frame_id)
    }

    /// Resume execution of the debugee
    pub fn r#continue(&self) -> eyre::Result<()> {
        self.internals.lock().unwrap().r#continue()
    }

    /// Step over a statement
    pub fn step_over(&self) -> eyre::Result<()> {
        self.internals.lock().unwrap().step_over()
    }

    /// Step into a statement
    pub fn step_in(&self) -> eyre::Result<()> {
        self.internals.lock().unwrap().step_in()
    }

    /// Step out of a statement
    pub fn step_out(&self) -> eyre::Result<()> {
        self.internals.lock().unwrap().step_out()
    }

    pub fn with_current_source<F>(&self, f: F)
    where
        F: Fn(Option<&FileSource>),
    {
        let internals = self.internals.lock().unwrap();
        f(internals.current_source.as_ref())
    }

    pub fn wait_for_event<F>(&self, pred: F) -> Event
    where
        F: Fn(&Event) -> bool,
    {
        let mut n = 0;
        loop {
            let evt = self.rx.recv().unwrap();
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

    pub fn change_scope(&self, stack_frame_id: StackFrameId) -> eyre::Result<()> {
        self.internals
            .lock()
            .unwrap()
            .change_scope(stack_frame_id)
            .wrap_err("changing scope")?;
        Ok(())
    }
}
