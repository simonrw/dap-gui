use std::{
    io,
    net::{TcpStream, ToSocketAddrs},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use eyre::WrapErr;
use retry::{delay::Exponential, retry};
use server::Implementation;
use transport::{
    requests::{self, Disconnect},
    responses,
    types::StackFrameId,
    DEFAULT_DAP_PORT,
};

use crate::{
    internals::{DebuggerInternals, FileSource},
    state::{self, DebuggerState},
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

fn retry_scale() -> impl Iterator<Item = Duration> {
    Exponential::from_millis(200).take(5)
}

fn reliable_tcp_stream<A>(addr: A) -> Result<TcpStream, retry::Error<io::Error>>
where
    A: ToSocketAddrs + Clone,
{
    retry(retry_scale(), || {
        tracing::debug!("trying to make connection");
        match TcpStream::connect(addr.clone()) {
            Ok(stream) => {
                tracing::debug!("connection made");
                Ok(stream)
            }
            Err(e) => {
                tracing::debug!(error = %e, "error making connection");
                Err(e)
            }
        }
    })
}

pub struct Debugger {
    internals: Arc<Mutex<DebuggerInternals>>,
    rx: crossbeam_channel::Receiver<Event>,
}

impl Debugger {
    #[tracing::instrument(skip(initialise_arguments))]
    pub fn on_port(
        port: u16,
        initialise_arguments: impl Into<InitialiseArguments>,
    ) -> eyre::Result<Self> {
        tracing::debug!("creating new client");

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
                let stream = reliable_tcp_stream(format!("127.0.0.1:{port}"))
                    .context("connecting to server")?;

                let (ttx, trx) = crossbeam_channel::unbounded();
                let client =
                    transport::Client::new(stream, ttx).context("creating transport client")?;

                let internals = DebuggerInternals::new(client, tx, Some(s));
                (internals, trx)
            }
            InitialiseArguments::Attach(_) => {
                let stream = reliable_tcp_stream(format!("127.0.0.1:{port}"))
                    .context("connecting to server")?;

                let (ttx, trx) = crossbeam_channel::unbounded();
                let client =
                    transport::Client::new(stream, ttx).context("creating transport client")?;

                let internals = DebuggerInternals::new(client, tx, None);
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
        breakpoint: types::Breakpoint,
    ) -> eyre::Result<types::BreakpointId> {
        let mut internals = self.internals.lock().unwrap();
        internals.add_breakpoint(breakpoint)
    }

    pub fn launch(&self) -> eyre::Result<()> {
        let mut internals = self.internals.lock().unwrap();
        let _ = internals
            .client
            .send(requests::RequestBody::ConfigurationDone)
            .context("completing configuration")?;
        internals.set_state(DebuggerState::Running);
        Ok(())
    }

    pub fn evaluate(&self, input: &str, frame_id: StackFrameId) -> eyre::Result<EvaluateResult> {
        let internals = self.internals.lock().unwrap();
        let req = requests::RequestBody::Evaluate(requests::Evaluate {
            expression: input.to_string(),
            frame_id: Some(frame_id),
            context: Some("repl".to_string()),
        });
        let res = internals
            .client
            .send(req)
            .context("sending evaluate request")?;
        match res {
            Some(responses::ResponseBody::Evaluate(responses::EvaluateResponse {
                result, ..
            })) => Ok(EvaluateResult { output: result }),
            _ => unreachable!(),
        }
    }

    /// Resume execution of the debugee
    pub fn r#continue(&self) -> eyre::Result<()> {
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
            None => eyre::bail!("logic error: no current thread id"),
        }
        Ok(())
    }

    /// Step over a statement
    pub fn step_over(&self) -> eyre::Result<()> {
        let internals = self.internals.lock().unwrap();
        match internals.current_thread_id {
            Some(thread_id) => {
                internals
                    .client
                    .execute(requests::RequestBody::Next(requests::Next { thread_id }))
                    .context("sending step_over request")?;
            }
            None => eyre::bail!("logic error: no current thread id"),
        }
        Ok(())
    }

    /// Step into a statement
    pub fn step_in(&self) -> eyre::Result<()> {
        let internals = self.internals.lock().unwrap();
        match internals.current_thread_id {
            Some(thread_id) => {
                internals
                    .client
                    .execute(requests::RequestBody::StepIn(requests::StepIn {
                        thread_id,
                    }))
                    .context("sending step_in` request")?;
            }
            None => eyre::bail!("logic error: no current thread id"),
        }
        Ok(())
    }

    /// Step out of a statement
    pub fn step_out(&self) -> eyre::Result<()> {
        let internals = self.internals.lock().unwrap();
        match internals.current_thread_id {
            Some(thread_id) => {
                internals
                    .client
                    .execute(requests::RequestBody::StepOut(requests::StepOut {
                        thread_id,
                    }))
                    .context("sending `step_out` request")?;
            }
            None => eyre::bail!("logic error: no current thread id"),
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

    fn execute(&self, body: requests::RequestBody) -> eyre::Result<()> {
        self.internals.lock().unwrap().client.execute(body)
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
