use std::{
    io::{self, BufRead},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use eyre::WrapErr;
use launch_configuration::LaunchConfiguration;
use retry::{delay::Exponential, retry};
use server::Implementation;
use transport::{
    DEFAULT_DAP_PORT, Message, Reader, TransportConnection,
    requests::{self, Disconnect},
    responses,
    types::{BreakpointLocation, StackFrameId, Variable},
};
use uuid::Uuid;

use crate::{
    AttachArguments, Event, Language, LaunchArguments,
    commands::Command,
    internals::DebuggerInternals,
    state::{self, DebuggerState},
    types::{self, EvaluateResult},
};

/// How to launch a debugging session
#[derive(Debug)]
pub enum InitialiseArguments {
    /// Launch a new process with a debugger and connect to the session immediately
    Launch(state::LaunchArguments),

    /// Attach to a running process
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

impl From<LaunchConfiguration> for InitialiseArguments {
    fn from(value: LaunchConfiguration) -> Self {
        match value {
            LaunchConfiguration::Debugpy(debugpy) | LaunchConfiguration::Python(debugpy) => {
                match debugpy.request.as_str() {
                    "launch" => InitialiseArguments::Launch(LaunchArguments {
                        program: debugpy.program.expect("program must be specified"),
                        working_directory: None,
                        language: crate::Language::DebugPy,
                    }),
                    "attach" => InitialiseArguments::Attach(AttachArguments {
                        port: debugpy.connect.map(|c| c.port),
                        language: Language::DebugPy,
                        path_mappings: debugpy.path_mappings,
                        working_directory: debugpy.cwd.expect("TODO: cwd must be specified"),
                    }),
                    other => todo!("{other}"),
                }
            }
            LaunchConfiguration::LLDB(lldb) => match lldb.request.as_str() {
                "launch" =>
                {
                    #[allow(unreachable_code)]
                    InitialiseArguments::Launch(LaunchArguments {
                        working_directory: None,
                        language: crate::Language::DebugPy,
                        program: todo!(),
                    })
                }
                other => todo!("{other}"),
            },
        }
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

/// Spawn a polling thread that reads messages from a reader and forwards them to channels
///
/// This helper function eliminates code duplication between Launch and Attach initialization paths.
/// The polling thread:
/// - Continuously reads messages from the reader
/// - Forwards events to the event channel
/// - Forwards all messages to the message channel for request/response matching
/// - Terminates when the connection closes or an error occurs
fn spawn_polling_thread(
    mut reader: transport::reader::hand_written_reader::HandWrittenReader<Box<dyn BufRead + Send>>,
    event_tx: crossbeam_channel::Sender<transport::events::Event>,
    message_tx: crossbeam_channel::Sender<transport::Message>,
) {
    thread::spawn(move || {
        loop {
            match reader.poll_message() {
                Ok(Some(message)) => {
                    tracing::debug!(?message, "received message in polling thread");
                    // Forward events to event channel
                    if let Message::Event(ref event) = message {
                        if event_tx.send(event.clone()).is_err() {
                            tracing::debug!("event channel closed, terminating polling thread");
                            break;
                        }
                    }
                    // Forward ALL messages to the message channel
                    if message_tx.send(message).is_err() {
                        tracing::debug!("message channel closed, terminating polling thread");
                        break;
                    }
                }
                Ok(None) => {
                    tracing::debug!("connection closed, terminating polling thread");
                    break;
                }
                Err(e) => {
                    tracing::error!(error = %e, "error receiving message in polling thread, terminating");
                    break;
                }
            }
        }
        tracing::debug!("polling thread terminated");
    });
}

/// Represents a debugging session
pub struct Debugger {
    internals: Arc<Mutex<DebuggerInternals>>,
    rx: crossbeam_channel::Receiver<Event>,
    command_tx: crossbeam_channel::Sender<Command>,
}

impl Debugger {
    /// Connect to an existing DAP session on the given port.
    ///
    /// Takes [`InitialiseArguments`] for configuration of the debugging session
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
            InitialiseArguments::Launch(state::LaunchArguments {
                program, language, ..
            }) => {
                eyre::ensure!(
                    program.is_file(),
                    "Program {} does not exist",
                    program.display()
                );

                // let implementation = language.into();
                let implementation: Implementation = match language {
                    crate::Language::DebugPy => Implementation::Debugpy,
                    crate::Language::Delve => Implementation::Delve,
                };

                let s = server::for_implementation_on_port(implementation, port)
                    .context("creating background server process")?;

                let connection = TransportConnection::connect(format!("127.0.0.1:{port}"))
                    .context("connecting to server")?;

                // Split the connection into reader and writer to avoid mutex contention
                let (mut reader, writer, sequence_number) = connection.split_connection();

                let (ttx, trx) = crossbeam_channel::unbounded();
                let (message_tx, message_rx) = crossbeam_channel::unbounded();

                // Wrap writer in Arc<Mutex<>> for shared access
                let writer_arc = Arc::new(Mutex::new(writer));

                // Spawn polling thread with direct ownership of the reader (no mutex needed)
                spawn_polling_thread(reader, ttx, message_tx);

                let internals = DebuggerInternals::from_split_connection(
                    writer_arc,
                    sequence_number,
                    tx,
                    message_rx,
                    Some(s),
                );
                (internals, trx)
            }
            InitialiseArguments::Attach(_) => {
                let connection = TransportConnection::connect(format!("127.0.0.1:{port}"))
                    .context("connecting to server")?;

                // Split the connection into reader and writer to avoid mutex contention
                let (mut reader, writer, sequence_number) = connection.split_connection();

                let (ttx, trx) = crossbeam_channel::unbounded();
                let (message_tx, message_rx) = crossbeam_channel::unbounded();

                // Wrap writer in Arc<Mutex<>> for shared access
                let writer_arc = Arc::new(Mutex::new(writer));

                // Spawn polling thread with direct ownership of the reader (no mutex needed)
                spawn_polling_thread(reader, ttx, message_tx);

                let internals = DebuggerInternals::from_split_connection(
                    writer_arc,
                    sequence_number,
                    tx,
                    message_rx,
                    None,
                );
                (internals, trx)
            }
        };

        internals.initialise(args).context("initialising")?;

        let internals = Arc::new(Mutex::new(internals));

        // Create command channel for main thread -> background thread communication
        let (command_tx, command_rx) = crossbeam_channel::unbounded();

        // background thread reading transport events and commands
        let background_internals = Arc::clone(&internals);
        let background_events = events.clone();
        thread::spawn(move || {
            use crate::internals::FollowUpRequest;

            let mut follow_up_queue: Vec<FollowUpRequest> = Vec::new();

            loop {
                crossbeam_channel::select! {
                    recv(background_events) -> msg => {
                        let event = match msg {
                            Ok(event) => event,
                            Err(_) => {
                                tracing::debug!("event channel closed, terminating background thread");
                                break;
                            }
                        };

                        let lock_id = Uuid::new_v4().to_string();
                        let span = tracing::trace_span!("event", %lock_id);
                        let _guard = span.enter();

                        tracing::trace!(is_poisoned = %background_internals.is_poisoned(), "trying to unlock background internals");

                        match background_internals.lock() {
                            Ok(mut internals) => {
                                tracing::trace!(?event, "handling event");

                                // Use non-blocking event processing
                                let follow_ups = internals.on_event_nonblocking(event);
                                follow_up_queue.extend(follow_ups);

                                drop(internals);
                                tracing::trace!("unlocked background internals");
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "mutex poisoned, terminating background thread");
                                break;
                            }
                        }
                    }
                    recv(command_rx) -> msg => {
                        let command = match msg {
                            Ok(command) => command,
                            Err(_) => {
                                tracing::debug!("command channel closed, terminating background thread");
                                break;
                            }
                        };

                        let lock_id = Uuid::new_v4().to_string();
                        let span = tracing::trace_span!("command", %lock_id);
                        let _guard = span.enter();

                        match command {
                            Command::SendRequest { body, response_tx } => {
                                tracing::trace!(?body, "handling send request command");
                                match background_internals.lock() {
                                    Ok(mut internals) => {
                                        match internals.send(body) {
                                            Ok(response) => {
                                                let _ = response_tx.send(Ok(response));
                                            }
                                            Err(e) => {
                                                let _ = response_tx.send(Err(e));
                                            }
                                        }
                                        drop(internals);
                                    }
                                    Err(e) => {
                                        let _ = response_tx.send(Err(eyre::eyre!("mutex poisoned: {}", e)));
                                    }
                                }
                            }
                            Command::SendExecute { body, response_tx } => {
                                tracing::trace!(?body, "handling send execute command");
                                match background_internals.lock() {
                                    Ok(mut internals) => {
                                        match internals.execute(body) {
                                            Ok(()) => {
                                                let _ = response_tx.send(Ok(()));
                                            }
                                            Err(e) => {
                                                let _ = response_tx.send(Err(e));
                                            }
                                        }
                                        drop(internals);
                                    }
                                    Err(e) => {
                                        let _ = response_tx.send(Err(eyre::eyre!("mutex poisoned: {}", e)));
                                    }
                                }
                            }
                            Command::Shutdown => {
                                tracing::debug!("received shutdown command");
                                break;
                            }
                        }
                    }
                }

                // Process follow-up requests
                while let Some(follow_up) = follow_up_queue.pop() {
                    match background_internals.lock() {
                        Ok(mut internals) => {
                            let body = follow_up.to_request_body();
                            match internals.send(body) {
                                Ok(response) => {
                                    let more_follow_ups =
                                        internals.on_follow_up_response(follow_up, response);
                                    follow_up_queue.extend(more_follow_ups);
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "failed to send follow-up request");
                                }
                            }
                            drop(internals);
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "mutex poisoned while processing follow-up");
                            break;
                        }
                    }
                }
            }
            tracing::debug!("background thread terminated");
        });

        Ok(Self {
            internals,
            rx: internals_rx,
            command_tx,
        })
    }

    /// Create a new debugging session on the default DAP port (5678)
    ///
    /// Note: the debugging session does not start until [`Debugger::start`] is called
    #[tracing::instrument(skip(initialise_arguments))]
    pub fn new(initialise_arguments: impl Into<InitialiseArguments>) -> eyre::Result<Self> {
        Self::on_port(DEFAULT_DAP_PORT, initialise_arguments)
    }

    /// Read a VS Code style launch configuration file and create a debugger suitable for one of
    /// the launch configurations
    pub fn from_launch_configuration(
        configuration_path: impl AsRef<Path>,
        configuration_name: impl Into<String>,
    ) -> eyre::Result<Self> {
        use launch_configuration::ChosenLaunchConfiguration;

        let name = configuration_name.into();
        let config = launch_configuration::load_from_path(Some(&name), configuration_path)
            .context("loading launch configuration")?;
        match config {
            ChosenLaunchConfiguration::Specific(config) => Debugger::new(config),
            _ => Err(eyre::eyre!("specified configuration {name} not found")),
        }
    }

    /// Return a [`crossbeam_channel::Receiver<Event>`] to subscribe to debugging events
    pub fn events(&self) -> crossbeam_channel::Receiver<Event> {
        self.rx.clone()
    }

    /// Add a breakpoint for the current debugging session
    #[tracing::instrument(skip(self))]
    pub fn add_breakpoint(
        &self,
        breakpoint: &types::Breakpoint,
    ) -> eyre::Result<types::BreakpointId> {
        self.with_internals(|internals| {
            internals
                .add_breakpoint(breakpoint)
                .context("adding breakpoint")
        })
    }

    pub fn get_breakpoint_locations(
        &self,
        path: impl Into<PathBuf>,
    ) -> eyre::Result<Vec<BreakpointLocation>> {
        let locations = self
            .with_internals(|internals| internals.get_breakpoint_locations(path))
            .context("getting breakpoint locations")?;
        Ok(locations)
    }

    /// Return the list of breakpoints configured
    pub fn breakpoints(&self) -> Vec<types::Breakpoint> {
        self.with_internals(|internals| {
            Ok(internals.breakpoints.clone().values().cloned().collect())
        })
        .unwrap()
    }

    /// Launch a debugging session
    pub fn start(&self) -> eyre::Result<()> {
        self.send_request(requests::RequestBody::ConfigurationDone)
            .context("completing configuration")?;

        self.with_internals(|internals| {
            internals.set_state(DebuggerState::Running);
            Ok(())
        })
        .context("completing configuration")?;
        Ok(())
    }

    /// Perform a code/variable evaluation within a debugging session
    pub fn evaluate(
        &self,
        input: &str,
        frame_id: StackFrameId,
    ) -> eyre::Result<Option<EvaluateResult>> {
        let req = requests::RequestBody::Evaluate(requests::Evaluate {
            expression: input.to_string(),
            frame_id: Some(frame_id),
            context: Some("repl".to_string()),
        });
        let res = self.send_request(req).context("sending evaluate request")?;
        match res {
            responses::Response {
                body:
                    Some(responses::ResponseBody::Evaluate(responses::EvaluateResponse {
                        result, ..
                    })),
                success: true,
                ..
            } => Ok(Some(EvaluateResult {
                output: result,
                error: false,
            })),
            responses::Response {
                message: Some(msg),
                success: false,
                ..
            } => Ok(Some(EvaluateResult {
                output: msg,
                error: true,
            })),
            other => {
                tracing::warn!(response = ?other, "unhandled response");
                Ok(None)
            }
        }
    }

    /// Resume execution of the debugee
    #[tracing::instrument(skip(self))]
    pub fn r#continue(&self) -> eyre::Result<()> {
        let thread_id = self.with_internals(|internals| {
            internals
                .current_thread_id
                .ok_or_else(|| eyre::eyre!("logic error: no current thread id"))
        })?;

        self.send_execute(requests::RequestBody::Continue(requests::Continue {
            thread_id,
            single_thread: false,
        }))
        .context("sending continue request")
    }

    /// Step over a statement
    pub fn step_over(&self) -> eyre::Result<()> {
        let thread_id = self.with_internals(|internals| {
            internals
                .current_thread_id
                .ok_or_else(|| eyre::eyre!("logic error: no current thread id"))
        })?;

        self.send_execute(requests::RequestBody::Next(requests::Next { thread_id }))
            .context("sending step_over request")
    }

    /// Step into a statement
    pub fn step_in(&self) -> eyre::Result<()> {
        let thread_id = self.with_internals(|internals| {
            internals
                .current_thread_id
                .ok_or_else(|| eyre::eyre!("logic error: no current thread id"))
        })?;

        self.send_execute(requests::RequestBody::StepIn(requests::StepIn {
            thread_id,
        }))
        .context("sending step_in request")
    }

    /// Step out of a statement
    pub fn step_out(&self) -> eyre::Result<()> {
        let thread_id = self.with_internals(|internals| {
            internals
                .current_thread_id
                .ok_or_else(|| eyre::eyre!("logic error: no current thread id"))
        })?;

        self.send_execute(requests::RequestBody::StepOut(requests::StepOut {
            thread_id,
        }))
        .context("sending step_out request")
    }

    pub fn variables(&self, variables_reference: i64) -> eyre::Result<Vec<Variable>> {
        self.with_internals(|internals| internals.variables(variables_reference))
    }

    /// Send a request and wait for a response via the command channel
    fn send_request(&self, body: requests::RequestBody) -> eyre::Result<responses::Response> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(Command::SendRequest { body, response_tx })
            .map_err(|_| eyre::eyre!("command channel closed"))?;
        response_rx
            .recv()
            .map_err(|_| eyre::eyre!("response channel closed"))?
    }

    /// Send a request without waiting for a response (fire-and-forget) via the command channel
    fn send_execute(&self, body: requests::RequestBody) -> eyre::Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(Command::SendExecute { body, response_tx })
            .map_err(|_| eyre::eyre!("command channel closed"))?;
        response_rx
            .recv()
            .map_err(|_| eyre::eyre!("response channel closed"))?
    }

    fn execute(&self, body: requests::RequestBody) -> eyre::Result<()> {
        self.send_execute(body)
    }

    /// Pause the debugging session waiting for a specific event, where the predicate returns true
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

    /// Change the current scope to a new stack frame
    pub fn change_scope(&self, stack_frame_id: StackFrameId) -> eyre::Result<()> {
        self.with_internals(|internals| {
            internals
                .change_scope(stack_frame_id)
                .wrap_err("changing scope")?;
            Ok(())
        })
    }

    #[tracing::instrument(skip_all, fields(lock_id = Uuid::new_v4().to_string()))]
    fn with_internals<F, T>(&self, f: F) -> eyre::Result<T>
    where
        F: FnOnce(&mut DebuggerInternals) -> eyre::Result<T>,
    {
        tracing::trace!(poisoned = %self.internals.is_poisoned(), "trying to lock internals");
        let mut internals = self
            .internals
            .lock()
            .map_err(|e| eyre::eyre!("debugger mutex poisoned: {}", e))?;
        tracing::trace!("executing operation");
        let res = f(&mut internals);
        drop(internals);
        tracing::trace!("unlocked internals");
        res
    }
}

impl Drop for Debugger {
    fn drop(&mut self) {
        tracing::debug!("dropping debugger");
        if let Err(e) = self.execute(requests::RequestBody::Disconnect(Disconnect {
            terminate_debugee: true,
        })) {
            tracing::warn!(error = %e, "failed to disconnect debugger during drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Debugger;
    use crate::{Language, LaunchArguments};
    use std::path::PathBuf;
    use transport::bindings::get_random_tcp_port;

    #[test]
    fn error_missing_configuration() {
        let manifest_dir = dbg!(std::env::var_os("CARGO_MANIFEST_DIR")).unwrap();
        let root = PathBuf::from(manifest_dir)
            .join("..")
            .join("..")
            .join("test.py")
            .canonicalize()
            .unwrap();

        let tdir = tempfile::tempdir().unwrap();
        let temp_script_path = tdir.path().join("script.py");
        std::fs::copy(root, &temp_script_path).unwrap();

        let config = serde_json::json!({
            "version": "0.2.0",
            "configurations": [
            {
                "name": "Launch",
                "type": "debugpy",
                "request": "launch",
                "program": format!("{}", temp_script_path.display()),
            },
            ],
        });
        let config_file_path = tdir.path().join("launch.json");
        let config_file_obj = std::fs::File::create(&config_file_path).unwrap();
        serde_json::to_writer(config_file_obj, &config).unwrap();

        let bad_name = "abc";

        assert!(Debugger::from_launch_configuration(&config_file_path, bad_name).is_err());
    }

    #[test]
    fn error_program_does_not_exist() {
        let tdir = tempfile::tempdir().unwrap();
        let non_existent_program = tdir.path().join("nonexistent.py");

        // Verify the file doesn't exist
        assert!(!non_existent_program.is_file());

        let port = get_random_tcp_port().expect("getting free port");
        let launch_args = LaunchArguments {
            program: non_existent_program.clone(),
            working_directory: None,
            language: Language::DebugPy,
        };

        let result = Debugger::on_port(port, launch_args);

        let error = match result {
            Ok(_) => panic!("Expected error when program does not exist"),
            Err(e) => e,
        };
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("does not exist"),
            "Error message should mention 'does not exist', got: {}",
            error_msg
        );
        assert!(
            error_msg.contains(non_existent_program.display().to_string().as_str()),
            "Error message should contain the program path, got: {}",
            error_msg
        );
    }
}
