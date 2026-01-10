use std::{
    io::{self, BufRead, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
    thread,
    time::Duration,
};

use eyre::WrapErr;
use launch_configuration::LaunchConfiguration;
use retry::{delay::Exponential, retry};
use server::Implementation;
use transport::{
    DEFAULT_DAP_PORT, Message, PollResult, Reader, TransportConnection,
    reader::hand_written_reader::HandWrittenReader,
    requests::{self, Disconnect},
    responses,
    types::{BreakpointLocation, Seq, StackFrameId, Variable},
};
use uuid::Uuid;

use crate::{
    AttachArguments, Event, Language, LaunchArguments,
    commands::Command,
    internals::DebuggerInternals,
    pending_requests::{PendingItem, PendingRequests},
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

#[allow(dead_code)] // Legacy code
fn retry_scale() -> impl Iterator<Item = Duration> {
    Exponential::from_millis(200).take(5)
}

#[allow(dead_code)] // Legacy code
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

        // Set up connection and server based on args
        let (reader, writer, sequence_number, server) = match &args {
            InitialiseArguments::Launch(state::LaunchArguments {
                program, language, ..
            }) => {
                eyre::ensure!(
                    program.is_file(),
                    "Program {} does not exist",
                    program.display()
                );

                let implementation: Implementation = match language {
                    crate::Language::DebugPy => Implementation::Debugpy,
                    crate::Language::Delve => Implementation::Delve,
                };

                let s = server::for_implementation_on_port(implementation, port)
                    .context("creating background server process")?;

                let connection = TransportConnection::connect(format!("127.0.0.1:{port}"))
                    .context("connecting to server")?;

                let (reader, writer, sequence_number) = connection.split_connection();
                (reader, writer, sequence_number, Some(s))
            }
            InitialiseArguments::Attach(_) => {
                let connection = TransportConnection::connect(format!("127.0.0.1:{port}"))
                    .context("connecting to server")?;

                let (reader, writer, sequence_number) = connection.split_connection();
                (reader, writer, sequence_number, None)
            }
        };

        // Wrap writer in Arc<Mutex<>> for shared access
        let writer_arc = Arc::new(Mutex::new(writer));

        // Perform synchronous initialization with direct reader polling
        let (reader, queued_events) =
            Self::initialise_sync(&sequence_number, &writer_arc, reader, args)?;

        // Create message channel for legacy send() support
        // The background thread will forward untracked responses here
        let (message_tx, message_rx) = crossbeam_channel::unbounded();

        let internals = DebuggerInternals::from_split_connection(
            Arc::clone(&writer_arc),
            Arc::clone(&sequence_number),
            tx,
            message_rx,
            server,
        );

        let internals = Arc::new(Mutex::new(internals));

        // Create command channel for main thread -> background thread communication
        let (command_tx, command_rx) = crossbeam_channel::unbounded();

        // Single background thread that owns the reader and handles everything
        let background_internals = Arc::clone(&internals);
        let background_writer = Arc::clone(&writer_arc);
        let background_seq = Arc::clone(&sequence_number);

        thread::spawn(move || {
            Self::background_thread_loop(
                reader,
                background_internals,
                background_writer,
                background_seq,
                command_rx,
                message_tx,
                queued_events,
            );
        });

        Ok(Self {
            internals,
            rx: internals_rx,
            command_tx,
        })
    }

    /// Perform synchronous initialization by polling the reader directly
    /// Returns the reader and any events received during initialization
    fn initialise_sync(
        sequence_number: &Arc<AtomicI64>,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        mut reader: HandWrittenReader<Box<dyn BufRead + Send>>,
        args: InitialiseArguments,
    ) -> eyre::Result<(
        HandWrittenReader<Box<dyn BufRead + Send>>,
        Vec<transport::events::Event>,
    )> {
        use requests::{Initialize, PathFormat};

        tracing::debug!("performing synchronous initialization");

        let mut queued_events = Vec::new();

        // Send Initialize request
        let init_req = requests::RequestBody::Initialize(Initialize {
            adapter_id: "dap gui".to_string(),
            lines_start_at_one: false,
            path_format: PathFormat::Path,
            supports_start_debugging_request: true,
            supports_variable_type: true,
            supports_variable_paging: true,
            supports_progress_reporting: true,
            supports_memory_event: true,
        });

        let seq = Self::send_request_raw(sequence_number, writer, init_req)?;

        // Poll reader until we get the Initialize response
        let _init_response = Self::poll_for_response(&mut reader, seq, &mut queued_events)?;
        tracing::debug!("received Initialize response");

        // Send Launch or Attach request (fire-and-forget)
        match args {
            InitialiseArguments::Launch(launch_args) => {
                let req = launch_args.to_request();
                Self::send_execute_raw(sequence_number, writer, req)?;
            }
            InitialiseArguments::Attach(attach_args) => {
                let req = attach_args.to_request();
                Self::send_execute_raw(sequence_number, writer, req)?;
            }
        }

        tracing::debug!(
            "initialization complete, {} events queued",
            queued_events.len()
        );
        Ok((reader, queued_events))
    }

    /// Send a request using raw writer access, returning the sequence number
    fn send_request_raw(
        sequence_number: &Arc<AtomicI64>,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        body: requests::RequestBody,
    ) -> eyre::Result<Seq> {
        let seq = sequence_number.fetch_add(1, Ordering::SeqCst) + 1;
        let message = requests::Request {
            seq,
            r#type: "request".to_string(),
            body,
        };
        let json = serde_json::to_string(&message).wrap_err("encoding json body")?;
        tracing::debug!(seq, content = %json, "sending request");

        let mut w = writer
            .lock()
            .map_err(|e| eyre::eyre!("writer mutex poisoned: {}", e))?;

        write!(w.as_mut(), "Content-Length: {}\r\n\r\n{}", json.len(), json)
            .wrap_err("writing message")?;
        w.flush().wrap_err("flushing output")?;

        Ok(seq)
    }

    /// Send an execute request (fire-and-forget)
    fn send_execute_raw(
        sequence_number: &Arc<AtomicI64>,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        body: requests::RequestBody,
    ) -> eyre::Result<()> {
        let seq = sequence_number.fetch_add(1, Ordering::SeqCst) + 1;
        let message = requests::Request {
            seq,
            r#type: "request".to_string(),
            body,
        };
        let json = serde_json::to_string(&message).wrap_err("encoding json body")?;

        let mut w = writer
            .lock()
            .map_err(|e| eyre::eyre!("writer mutex poisoned: {}", e))?;

        write!(w.as_mut(), "Content-Length: {}\r\n\r\n{}", json.len(), json)
            .wrap_err("writing message")?;
        w.flush().wrap_err("flushing output")?;

        Ok(())
    }

    /// Poll the reader until we get a response with the given sequence number
    /// Also collects any events received during polling for later processing
    ///
    /// Uses blocking poll_message to ensure complete message reads
    fn poll_for_response(
        reader: &mut HandWrittenReader<Box<dyn BufRead + Send>>,
        expected_seq: Seq,
        queued_events: &mut Vec<transport::events::Event>,
    ) -> eyre::Result<responses::Response> {
        loop {
            // Use blocking poll_message to ensure we read complete messages
            // This is safe during initialization since we're single-threaded
            match reader.poll_message()? {
                Some(Message::Response(response)) => {
                    if response.request_seq == expected_seq {
                        return Ok(response);
                    }
                    // Not our response, continue polling
                    tracing::debug!(
                        got_seq = response.request_seq,
                        expected = expected_seq,
                        "received response for different request"
                    );
                }
                Some(Message::Event(event)) => {
                    // Queue events for later processing
                    tracing::debug!(?event, "queueing event received during initialization");
                    queued_events.push(event);
                }
                Some(Message::Request(_)) => {
                    tracing::warn!("received unexpected request from debug adapter");
                }
                None => {
                    eyre::bail!("connection closed while waiting for response");
                }
            }
        }
    }

    /// Main loop for the background thread
    ///
    /// This thread owns the reader and handles:
    /// - Polling the reader for messages (blocking with internal polling thread)
    /// - Processing events via on_event_nonblocking
    /// - Handling commands from the main thread
    /// - Processing follow-up requests
    fn background_thread_loop(
        mut reader: HandWrittenReader<Box<dyn BufRead + Send>>,
        internals: Arc<Mutex<DebuggerInternals>>,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
        sequence_number: Arc<AtomicI64>,
        command_rx: crossbeam_channel::Receiver<Command>,
        message_tx: crossbeam_channel::Sender<Message>,
        queued_events: Vec<transport::events::Event>,
    ) {
        let mut pending = PendingRequests::new();

        // Create channel for reader messages
        let (reader_tx, reader_rx) = crossbeam_channel::unbounded::<Message>();

        // Spawn a dedicated polling thread for the reader
        // This uses blocking poll_message which is reliable
        thread::spawn(move || {
            loop {
                match reader.poll_message() {
                    Ok(Some(message)) => {
                        if reader_tx.send(message).is_err() {
                            tracing::debug!("reader channel closed, terminating polling");
                            break;
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("connection closed");
                        break;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "error reading from transport");
                        break;
                    }
                }
            }
            tracing::debug!("polling helper thread terminated");
        });

        // Process events that were queued during initialization
        for event in queued_events {
            tracing::debug!(?event, "processing queued event from initialization");
            Self::process_event(&internals, &writer, &sequence_number, &mut pending, event);
        }

        // Main loop using select on reader messages and commands
        loop {
            crossbeam_channel::select! {
                recv(reader_rx) -> msg => {
                    let message = match msg {
                        Ok(m) => m,
                        Err(_) => {
                            tracing::debug!("reader channel closed");
                            break;
                        }
                    };

                    match message {
                        Message::Event(event) => {
                            Self::process_event(
                                &internals,
                                &writer,
                                &sequence_number,
                                &mut pending,
                                event,
                            );
                        }
                        Message::Response(response) => {
                            let request_seq = response.request_seq;
                            match pending.take(request_seq) {
                                Some(PendingItem::Command(tx)) => {
                                    let _ = tx.send(Ok(response));
                                }
                                Some(PendingItem::FollowUp(follow_up)) => {
                                    match internals.lock() {
                                        Ok(mut int) => {
                                            let more_follow_ups =
                                                int.on_follow_up_response(follow_up, response);
                                            drop(int);

                                            for fu in more_follow_ups {
                                                let body = fu.to_request_body();
                                                match Self::send_request_raw(
                                                    &sequence_number,
                                                    &writer,
                                                    body,
                                                ) {
                                                    Ok(seq) => {
                                                        pending.add_follow_up(seq, fu);
                                                    }
                                                    Err(e) => {
                                                        tracing::error!(
                                                            error = %e,
                                                            "failed to send follow-up"
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(error = %e, "mutex poisoned");
                                            break;
                                        }
                                    }
                                }
                                None => {
                                    // Untracked response - forward for legacy send() support
                                    tracing::trace!(
                                        request_seq,
                                        "forwarding untracked response"
                                    );
                                    let _ = message_tx.send(Message::Response(response));
                                }
                            }
                        }
                        Message::Request(_) => {
                            tracing::warn!("unexpected request from debug adapter");
                        }
                    }
                }
                recv(command_rx) -> msg => {
                    let command = match msg {
                        Ok(c) => c,
                        Err(_) => {
                            tracing::debug!("command channel closed");
                            break;
                        }
                    };

                    match command {
                        Command::SendRequest { body, response_tx } => {
                            tracing::trace!(?body, "handling send request command");
                            match Self::send_request_raw(&sequence_number, &writer, body) {
                                Ok(seq) => {
                                    pending.add_command_with_sender(seq, response_tx);
                                }
                                Err(e) => {
                                    let _ = response_tx.send(Err(e));
                                }
                            }
                        }
                        Command::SendExecute { body, response_tx } => {
                            tracing::trace!(?body, "handling send execute command");
                            match Self::send_execute_raw(&sequence_number, &writer, body) {
                                Ok(()) => {
                                    let _ = response_tx.send(Ok(()));
                                }
                                Err(e) => {
                                    let _ = response_tx.send(Err(e));
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
        }

        tracing::debug!("background thread terminated");
    }

    /// Process a single event, handling follow-up requests
    fn process_event(
        internals: &Arc<Mutex<DebuggerInternals>>,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        sequence_number: &Arc<AtomicI64>,
        pending: &mut PendingRequests,
        event: transport::events::Event,
    ) {
        let lock_id = Uuid::new_v4().to_string();
        let span = tracing::trace_span!("event", %lock_id);
        let _guard = span.enter();

        match internals.lock() {
            Ok(mut int) => {
                tracing::trace!(?event, "handling event");
                let follow_ups = int.on_event_nonblocking(event);
                drop(int);

                // Queue follow-up requests
                for follow_up in follow_ups {
                    let body = follow_up.to_request_body();
                    match Self::send_request_raw(sequence_number, writer, body) {
                        Ok(seq) => {
                            pending.add_follow_up(seq, follow_up);
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "failed to send follow-up request");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "mutex poisoned in process_event");
            }
        }
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
