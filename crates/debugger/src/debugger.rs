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
    DEFAULT_DAP_PORT, Message, Reader, TransportConnection,
    reader::{PollResult, hand_written_reader::HandWrittenReader},
    requests::{self, Disconnect, Initialize, PathFormat},
    responses,
    types::{BreakpointLocation, Seq, StackFrameId, Variable},
};
use uuid::Uuid;

use crate::{
    AttachArguments, Event, Language, LaunchArguments,
    commands::Command,
    internals::{DebuggerInternals, FollowUpRequest},
    pending_requests::PendingRequests,
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

        // Set up common connection components
        let server: Option<Box<dyn server::Server + Send>> = match &args {
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

                Some(
                    server::for_implementation_on_port(implementation, port)
                        .context("creating background server process")?,
                )
            }
            InitialiseArguments::Attach(_) => None,
        };

        let connection = TransportConnection::connect(format!("127.0.0.1:{port}"))
            .context("connecting to server")?;

        // Split the connection into reader and writer
        let (reader, writer, sequence_number) = connection.split_connection();

        // Wrap writer in Arc<Mutex<>> for shared access
        let writer_arc = Arc::new(Mutex::new(writer));

        // Perform synchronous initialization before starting background thread
        let (reader, queued_events) =
            Self::initialise_sync(&sequence_number, &writer_arc, reader, args)
                .context("synchronous initialization")?;

        // Create internals (without message_rx channel since background thread owns reader)
        let internals = DebuggerInternals::from_split_connection_no_channel(
            Arc::clone(&writer_arc),
            Arc::clone(&sequence_number),
            tx.clone(),
            server,
        );

        let internals = Arc::new(Mutex::new(internals));

        // Create command channel for main thread -> background thread communication
        let (command_tx, command_rx) = crossbeam_channel::unbounded();

        // Single background thread that owns the reader and processes everything
        let background_internals = Arc::clone(&internals);
        let background_writer = Arc::clone(&writer_arc);
        let background_seq = Arc::clone(&sequence_number);
        thread::spawn(move || {
            Self::background_thread_loop(
                reader,
                background_writer,
                background_seq,
                background_internals,
                command_rx,
                queued_events,
            );
        });

        Ok(Self {
            internals,
            rx: internals_rx,
            command_tx,
        })
    }

    /// Perform synchronous initialization before starting the background thread.
    ///
    /// This sends Initialize, Launch/Attach requests and waits for the Initialized event.
    /// Returns the reader and any events that were received during initialization
    /// (to be processed by the background thread).
    fn initialise_sync(
        sequence_number: &Arc<AtomicI64>,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        mut reader: HandWrittenReader<Box<dyn BufRead + Send>>,
        args: InitialiseArguments,
    ) -> eyre::Result<(
        HandWrittenReader<Box<dyn BufRead + Send>>,
        Vec<transport::events::Event>,
    )> {
        tracing::debug!("performing synchronous initialization");

        let mut queued_events = Vec::new();

        // Send Initialize request
        let init_seq = Self::send_request_raw(
            sequence_number,
            writer,
            requests::RequestBody::Initialize(Initialize {
                adapter_id: "dap gui".to_string(),
                lines_start_at_one: false,
                path_format: PathFormat::Path,
                supports_start_debugging_request: true,
                supports_variable_type: true,
                supports_variable_paging: true,
                supports_progress_reporting: true,
                supports_memory_event: true,
            }),
        )
        .context("sending initialize request")?;

        // Wait for Initialize response
        Self::poll_for_response(&mut reader, init_seq, &mut queued_events)
            .context("waiting for initialize response")?;

        // Send Launch or Attach request (fire-and-forget)
        let launch_body = match args {
            InitialiseArguments::Launch(launch_args) => launch_args.to_request(),
            InitialiseArguments::Attach(attach_args) => attach_args.to_request(),
        };
        Self::send_execute_raw(sequence_number, writer, launch_body)
            .context("sending launch/attach request")?;

        tracing::debug!("synchronous initialization complete");
        Ok((reader, queued_events))
    }

    /// Send a request and return its sequence number (helper for sync init)
    fn send_request_raw(
        sequence_number: &Arc<AtomicI64>,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        body: requests::RequestBody,
    ) -> eyre::Result<Seq> {
        use std::io::Write as _;

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
        w.flush().wrap_err("flushing")?;

        Ok(seq)
    }

    /// Send a fire-and-forget request (helper for sync init)
    fn send_execute_raw(
        sequence_number: &Arc<AtomicI64>,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        body: requests::RequestBody,
    ) -> eyre::Result<()> {
        Self::send_request_raw(sequence_number, writer, body)?;
        Ok(())
    }

    /// Poll for a specific response, queuing any events received
    fn poll_for_response(
        reader: &mut HandWrittenReader<Box<dyn BufRead + Send>>,
        expected_seq: Seq,
        queued_events: &mut Vec<transport::events::Event>,
    ) -> eyre::Result<responses::Response> {
        loop {
            match reader.poll_message()? {
                Some(Message::Response(resp)) if resp.request_seq == expected_seq => {
                    return Ok(resp);
                }
                Some(Message::Response(_)) => {
                    // Different response, keep waiting
                    continue;
                }
                Some(Message::Event(event)) => {
                    // Queue event for later processing
                    queued_events.push(event);
                }
                Some(Message::Request(_)) => {
                    tracing::warn!("unexpected request from debug adapter during init");
                }
                None => {
                    eyre::bail!("connection closed during initialization");
                }
            }
        }
    }

    /// The main background thread loop that processes messages and commands
    fn background_thread_loop(
        mut reader: HandWrittenReader<Box<dyn BufRead + Send>>,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
        sequence_number: Arc<AtomicI64>,
        internals: Arc<Mutex<DebuggerInternals>>,
        command_rx: crossbeam_channel::Receiver<Command>,
        queued_events: Vec<transport::events::Event>,
    ) {
        const POLL_TIMEOUT: Duration = Duration::from_millis(10);

        let mut pending_requests = PendingRequests::new();
        let mut follow_up_queue: Vec<FollowUpRequest> = Vec::new();
        let mut pending_follow_ups: std::collections::HashMap<Seq, FollowUpRequest> =
            std::collections::HashMap::new();

        // Process any events that were queued during initialization
        for event in queued_events {
            if let Ok(mut guard) = internals.lock() {
                let follow_ups = guard.on_event_nonblocking(event);
                follow_up_queue.extend(follow_ups);
            }
        }

        loop {
            // 1. Poll for messages from the transport (non-blocking with timeout)
            match reader.try_poll_message(POLL_TIMEOUT) {
                Ok(PollResult::Message(message)) => {
                    tracing::debug!(?message, "received message");
                    Self::process_message(
                        message,
                        &internals,
                        &mut pending_requests,
                        &mut pending_follow_ups,
                        &mut follow_up_queue,
                    );
                }
                Ok(PollResult::Closed) => {
                    tracing::debug!("connection closed, terminating background thread");
                    break;
                }
                Ok(PollResult::Timeout) => {
                    // No message available, continue to check commands
                }
                Err(e) => {
                    tracing::error!(error = %e, "error receiving message, terminating");
                    break;
                }
            }

            // 2. Check for commands from the main thread (non-blocking)
            match command_rx.try_recv() {
                Ok(command) => {
                    let should_exit = Self::process_command(
                        command,
                        &writer,
                        &sequence_number,
                        &mut pending_requests,
                    );
                    if should_exit {
                        break;
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // No command available
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    tracing::debug!("command channel closed, terminating background thread");
                    break;
                }
            }

            // 3. Process follow-up requests
            Self::process_follow_ups(
                &mut follow_up_queue,
                &mut pending_follow_ups,
                &writer,
                &sequence_number,
            );
        }

        tracing::debug!("background thread terminated");
    }

    /// Process an incoming message
    fn process_message(
        message: Message,
        internals: &Arc<Mutex<DebuggerInternals>>,
        pending_requests: &mut PendingRequests,
        pending_follow_ups: &mut std::collections::HashMap<Seq, FollowUpRequest>,
        follow_up_queue: &mut Vec<FollowUpRequest>,
    ) {
        match message {
            Message::Event(event) => {
                // Process event and get follow-up requests
                if let Ok(mut guard) = internals.lock() {
                    let follow_ups = guard.on_event_nonblocking(event);
                    follow_up_queue.extend(follow_ups);
                }
            }
            Message::Response(response) => {
                let req_seq = response.request_seq;

                // Check if this is a follow-up response
                if let Some(follow_up) = pending_follow_ups.remove(&req_seq) {
                    if let Ok(mut guard) = internals.lock() {
                        let more_follow_ups = guard.on_follow_up_response(follow_up, response);
                        follow_up_queue.extend(more_follow_ups);
                    }
                } else {
                    // Regular command response
                    pending_requests.handle_response(response);
                }
            }
            Message::Request(_) => {
                tracing::warn!("unexpected request from debug adapter");
            }
        }
    }

    /// Process a command from the main thread. Returns true if should exit.
    fn process_command(
        command: Command,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        sequence_number: &Arc<AtomicI64>,
        pending_requests: &mut PendingRequests,
    ) -> bool {
        match command {
            Command::SendRequest { body, response_tx } => {
                match Self::send_request_raw(sequence_number, writer, body) {
                    Ok(seq) => {
                        // Track the pending request
                        let rx = pending_requests.add(seq);
                        // Spawn a helper to forward the response
                        let tx = response_tx;
                        std::thread::spawn(move || match rx.recv() {
                            Ok(response) => {
                                let _ = tx.send(Ok(response));
                            }
                            Err(_) => {
                                let _ = tx.send(Err(eyre::eyre!("response channel closed")));
                            }
                        });
                    }
                    Err(e) => {
                        let _ = response_tx.send(Err(e));
                    }
                }
                false
            }
            Command::SendExecute { body, response_tx } => {
                match Self::send_execute_raw(sequence_number, writer, body) {
                    Ok(()) => {
                        let _ = response_tx.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = response_tx.send(Err(e));
                    }
                }
                false
            }
            Command::Shutdown => {
                tracing::debug!("received shutdown command");
                true
            }
        }
    }

    /// Process pending follow-up requests
    fn process_follow_ups(
        follow_up_queue: &mut Vec<FollowUpRequest>,
        pending_follow_ups: &mut std::collections::HashMap<Seq, FollowUpRequest>,
        writer: &Arc<Mutex<Box<dyn Write + Send>>>,
        sequence_number: &Arc<AtomicI64>,
    ) {
        while let Some(follow_up) = follow_up_queue.pop() {
            let body = follow_up.to_request_body();
            match Self::send_request_raw(sequence_number, writer, body) {
                Ok(seq) => {
                    // Track this follow-up by its sequence number
                    pending_follow_ups.insert(seq, follow_up);
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to send follow-up request");
                }
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
        // First, update local state and get the breakpoint ID
        let (id, requests) = self.with_internals(|internals| {
            let id = internals.add_breakpoint_local(breakpoint);
            let requests = internals.get_breakpoint_requests();
            Ok((id, requests))
        })?;

        // Then send breakpoint requests through the command channel
        for req in requests {
            self.send_request(req).context("broadcasting breakpoints")?;
        }

        Ok(id)
    }

    pub fn get_breakpoint_locations(
        &self,
        path: impl Into<PathBuf>,
    ) -> eyre::Result<Vec<BreakpointLocation>> {
        let req = requests::RequestBody::BreakpointLocations(requests::BreakpointLocations {
            source: transport::types::Source {
                path: Some(path.into()),
                ..Default::default()
            },
            ..Default::default()
        });

        let res = self
            .send_request(req)
            .context("sending BreakpointLocations request")?;

        match res.body {
            Some(responses::ResponseBody::BreakpointLocations(locations)) => {
                Ok(locations.breakpoints)
            }
            _ => eyre::bail!("invalid response type: {:?}", res),
        }
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
        let req = requests::RequestBody::Variables(requests::Variables {
            variables_reference,
        });
        match self.send_request(req).context("sending variables request") {
            Ok(responses::Response {
                body,
                success: true,
                ..
            }) => {
                if let Some(responses::ResponseBody::Variables(responses::VariablesResponse {
                    variables: scope_variables,
                })) = body
                {
                    Ok(scope_variables)
                } else {
                    tracing::debug!(vref = %variables_reference, "no variables found for reference");
                    Ok(Vec::new())
                }
            }
            Ok(other) => eyre::bail!("bad response from variables request: {other:?}"),
            Err(e) => Err(e),
        }
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
        // Get current thread ID
        let current_thread_id = self.with_internals(|internals| {
            internals
                .current_thread_id
                .ok_or_else(|| eyre::eyre!("no current thread id"))
        })?;

        // Send StackTrace request
        let stack_trace_response = self
            .send_request(requests::RequestBody::StackTrace(requests::StackTrace {
                thread_id: current_thread_id,
                ..Default::default()
            }))
            .context("getting stack trace")?;

        let stack_frames = match stack_trace_response {
            responses::Response {
                body:
                    Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
                        stack_frames,
                    })),
                success: true,
                ..
            } => stack_frames,
            resp => {
                eyre::bail!("unexpected response to StackTrace request: {:?}", resp);
            }
        };

        let chosen_stack_frame = stack_frames
            .iter()
            .find(|f| f.id == stack_frame_id)
            .ok_or_else(|| eyre::eyre!("missing stack frame {}", stack_frame_id))?;

        // Compute the paused frame (get scopes and variables)
        let paused_frame = self
            .compute_paused_frame(chosen_stack_frame)
            .context("computing paused frame")?;

        // Emit the scope change event
        self.with_internals(|internals| {
            internals.emit(Event::ScopeChange(state::ProgramState {
                stack: stack_frames.clone(),
                breakpoints: internals.breakpoints.values().cloned().collect(),
                paused_frame,
            }));
            Ok(())
        })
    }

    /// Compute paused frame by getting scopes and variables
    fn compute_paused_frame(
        &self,
        stack_frame: &transport::types::StackFrame,
    ) -> eyre::Result<types::PausedFrame> {
        // Get scopes for the frame
        let scopes_response = self
            .send_request(requests::RequestBody::Scopes(requests::Scopes {
                frame_id: stack_frame.id,
            }))
            .context("requesting scopes")?;

        let scopes = match scopes_response {
            responses::Response {
                body: Some(responses::ResponseBody::Scopes(responses::ScopesResponse { scopes })),
                success: true,
                ..
            } => scopes,
            resp => {
                eyre::bail!("unexpected response to Scopes request: {:?}", resp);
            }
        };

        // Get variables for each scope
        let mut variables = Vec::new();
        for scope in scopes {
            let scope_vars = self
                .variables(scope.variables_reference)
                .with_context(|| format!("fetching variables for scope {:?}", scope))?;
            variables.extend(scope_vars);
        }

        Ok(types::PausedFrame {
            frame: stack_frame.clone(),
            variables,
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
