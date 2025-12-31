use eyre::WrapErr;
use server::Server;
use std::{collections::HashMap, path::PathBuf};
use transport::{
    Client,
    requests::{self, Initialize, PathFormat},
    responses::{self, ResponseBody},
    types::{
        BreakpointLocation, Source, SourceBreakpoint, StackFrame, StackFrameId, ThreadId, Variable,
    },
};

use crate::{
    Event,
    debugger::InitialiseArguments,
    state::{DebuggerState, ProgramState},
    types::{Breakpoint, BreakpointId, PausedFrame},
};

/// Represents a follow-up request that needs to be made to the debug adapter
/// in response to an event. This allows event processing to be non-blocking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowUpRequest {
    /// Get stack trace for a thread
    StackTrace {
        thread_id: ThreadId,
        levels: Option<i64>,
        /// Context to identify which stage of processing this is for
        context: StackTraceContext,
    },
    /// Get scopes for a stack frame
    Scopes { frame_id: StackFrameId },
    /// Get variables for a scope
    Variables { variables_reference: i64 },
}

/// Context for why a StackTrace request was made
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackTraceContext {
    /// Initial stack trace to get current location (just top frame)
    InitialLocation,
    /// Full stack trace after getting location
    FullStack,
}

impl FollowUpRequest {
    /// Convert this follow-up request into a DAP request body
    pub fn to_request_body(&self) -> requests::RequestBody {
        match self {
            FollowUpRequest::StackTrace {
                thread_id,
                levels,
                context: _,
            } => requests::RequestBody::StackTrace(requests::StackTrace {
                thread_id: *thread_id,
                levels: levels.map(|l| l as usize),
                ..Default::default()
            }),
            FollowUpRequest::Scopes { frame_id } => {
                requests::RequestBody::Scopes(requests::Scopes {
                    frame_id: *frame_id,
                })
            }
            FollowUpRequest::Variables {
                variables_reference,
            } => requests::RequestBody::Variables(requests::Variables {
                variables_reference: *variables_reference,
            }),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FileSource {
    pub line: usize,
    pub file_path: Option<PathBuf>,
}

pub(crate) struct DebuggerInternals {
    pub(crate) client: Client,
    pub(crate) publisher: crossbeam_channel::Sender<Event>,

    // debugger specific details
    pub(crate) current_thread_id: Option<ThreadId>,
    pub(crate) breakpoints: HashMap<BreakpointId, Breakpoint>,

    current_breakpoint_id: BreakpointId,
    pub(crate) current_source: Option<FileSource>,

    pub(crate) _server: Option<Box<dyn Server + Send>>,
}

impl DebuggerInternals {
    pub(crate) fn new(
        client: Client,
        publisher: crossbeam_channel::Sender<Event>,
        server: Option<Box<dyn Server + Send>>,
    ) -> Self {
        Self::with_breakpoints(client, publisher, HashMap::new(), server)
    }

    pub(crate) fn change_scope(&mut self, stack_frame_id: StackFrameId) -> eyre::Result<()> {
        let current_thread_id = self
            .current_thread_id
            .ok_or_else(|| eyre::eyre!("no current thread id"))?;

        let responses::Response {
            body:
                Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
                    stack_frames,
                })),
            success: true,
            ..
        } = self
            .client
            .send(requests::RequestBody::StackTrace(requests::StackTrace {
                thread_id: current_thread_id,
                ..Default::default()
            }))
            .unwrap()
        else {
            unreachable!()
        };

        let chosen_stack_frame = stack_frames
            .iter()
            .find(|f| f.id == stack_frame_id)
            .ok_or_else(|| eyre::eyre!("missing stack frame {}", stack_frame_id))?;

        let paused_frame = self
            .compute_paused_frame(chosen_stack_frame)
            .context("computing paused frame")?;
        self.emit(Event::ScopeChange(ProgramState {
            stack: stack_frames,
            breakpoints: self.breakpoints.values().cloned().collect(),
            paused_frame,
        }));

        Ok(())
    }

    fn compute_paused_frame(&mut self, stack_frame: &StackFrame) -> eyre::Result<PausedFrame> {
        let responses::Response {
            body: Some(responses::ResponseBody::Scopes(responses::ScopesResponse { scopes })),
            success: true,
            ..
        } = self
            .client
            .send(requests::RequestBody::Scopes(requests::Scopes {
                frame_id: stack_frame.id,
            }))
            .expect("requesting scopes")
        else {
            unreachable!()
        };

        let mut variables = Vec::new();
        for scope in scopes {
            variables.extend(
                self.variables(scope.variables_reference)
                    .with_context(|| format!("fetching variables for scope {:?}", scope))?,
            );
        }
        let paused_frame = PausedFrame {
            frame: stack_frame.clone(),
            variables,
        };

        Ok(paused_frame)
    }

    pub(crate) fn variables(&mut self, variables_reference: i64) -> eyre::Result<Vec<Variable>> {
        let req = requests::RequestBody::Variables(requests::Variables {
            variables_reference,
        });
        match self.client.send(req).context("sending variables request") {
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
            other => eyre::bail!("bad response from variables request: {other:?}"),
        }
    }

    pub(crate) fn emit(&mut self, event: Event) {
        let _ = self.publisher.send(event);
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn initialise(&mut self, arguments: InitialiseArguments) -> eyre::Result<()> {
        tracing::debug!("initialising debugger internals");
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

        // TODO: deal with capabilities from the response
        tracing::debug!(request = ?req, "sending initialize event");
        let _ = self.client.send(req).context("sending initialize event")?;

        match arguments {
            InitialiseArguments::Launch(launch_arguments) => {
                // send launch event
                let req = launch_arguments.to_request();
                self.client.execute(req).context("sending launch request")?;
            }
            InitialiseArguments::Attach(attach_arguments) => {
                let req = attach_arguments.to_request();
                self.client.execute(req).context("sending attach request")?;
            }
        }

        tracing::debug!("initialised");

        Ok(())
    }

    pub(crate) fn with_breakpoints(
        client: Client,
        publisher: crossbeam_channel::Sender<Event>,
        existing_breakpoints: impl Into<HashMap<BreakpointId, Breakpoint>>,
        server: Option<Box<dyn Server + Send>>,
    ) -> Self {
        let breakpoints = existing_breakpoints.into();
        let current_breakpoint_id = *breakpoints.keys().max().unwrap_or(&0);

        Self {
            client,
            publisher,
            current_thread_id: None,
            breakpoints,
            current_breakpoint_id,
            current_source: None,
            _server: server,
        }
    }

    #[allow(dead_code)]
    fn get_stack_frames(&self) -> eyre::Result<Vec<StackFrame>> {
        todo!()
    }

    #[tracing::instrument(skip(self), level = "trace")]
    pub(crate) fn on_event(&mut self, event: transport::events::Event) {
        tracing::debug!("handling event");

        match event {
            transport::events::Event::Initialized => {
                // broadcast our internal state change
                self.set_state(DebuggerState::Initialised);
            }
            // transport::events::Event::Output(_) => todo!(),
            // transport::events::Event::Process(_) => todo!(),
            transport::events::Event::Stopped(transport::events::StoppedEventBody {
                thread_id,
                ..
            }) => {
                self.current_thread_id = Some(thread_id);
                // determine where we are in the source code
                let response = match self.client.send(requests::RequestBody::StackTrace(
                    requests::StackTrace {
                        thread_id,
                        levels: Some(1),
                        ..Default::default()
                    },
                )) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to get initial stack trace");
                        return;
                    }
                };

                let stack_frames = match response {
                    responses::Response {
                        body:
                            Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
                                stack_frames,
                            })),
                        success: true,
                        ..
                    } => stack_frames,
                    resp => {
                        tracing::error!(?resp, "unexpected response to initial StackTrace request");
                        return;
                    }
                };

                if stack_frames.is_empty() {
                    tracing::error!("no stack frames received in stopped event");
                    return;
                }

                if stack_frames.len() != 1 {
                    tracing::warn!(
                        count = stack_frames.len(),
                        "unexpected number of stack frames, using first frame"
                    );
                }

                let Some(source) = stack_frames[0].source.as_ref() else {
                    tracing::error!("stack frame has no source information");
                    return;
                };
                let line = stack_frames[0].line;

                let current_source = FileSource {
                    line,
                    file_path: source.path.clone(),
                };
                self.current_source = Some(current_source.clone());

                let response = match self.client.send(requests::RequestBody::StackTrace(
                    requests::StackTrace {
                        thread_id,
                        ..Default::default()
                    },
                )) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to get full stack trace");
                        return;
                    }
                };

                let stack_frames = match response {
                    responses::Response {
                        body:
                            Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
                                stack_frames,
                            })),
                        success: true,
                        ..
                    } => stack_frames,
                    resp => {
                        tracing::error!(?resp, "unexpected response to full StackTrace request");
                        return;
                    }
                };

                let Some(top_frame) = stack_frames.first() else {
                    tracing::error!("no frames found in full stack trace");
                    return;
                };

                let paused_frame = match self.compute_paused_frame(top_frame) {
                    Ok(frame) => frame,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to compute paused frame");
                        return;
                    }
                };

                self.set_state(DebuggerState::Paused {
                    stack: stack_frames,
                    paused_frame: Box::new(paused_frame),
                    breakpoints: self.breakpoints.values().cloned().collect(),
                });
            }
            transport::events::Event::Continued(_) => {
                self.current_thread_id = None;
                self.current_source = None;
                self.set_state(DebuggerState::Running);
            }
            // transport::events::Event::Thread(_) => todo!(),
            transport::events::Event::Exited(_) | transport::events::Event::Terminated => {
                self.set_state(DebuggerState::Ended);
            }
            // transport::events::Event::DebugpyWaitingForServer { host, port } => todo!(),
            // transport::events::Event::Module(_) => todo!(),
            _ => {
                tracing::debug!(?event, "unknown event");
            }
        }
        tracing::debug!(?event, "event handled");
    }

    #[tracing::instrument(skip(self), level = "trace", ret)]
    pub(crate) fn add_breakpoint(&mut self, breakpoint: &Breakpoint) -> eyre::Result<BreakpointId> {
        tracing::debug!("adding breakpoint");
        let id = self.next_id();
        self.breakpoints.insert(id, breakpoint.clone());
        self.broadcast_breakpoints()
            .context("updating breakpoints with debugee")?;
        Ok(id)
    }

    #[allow(dead_code)]
    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) fn remove_breakpoint(&mut self, id: BreakpointId) {
        tracing::debug!("removing breakpoint");
        self.breakpoints.remove(&id);
        self.broadcast_breakpoints()
            .expect("updating breakpoints with debugee");
    }

    fn broadcast_breakpoints(&mut self) -> eyre::Result<()> {
        tracing::debug!("broadcasting breakpoints");
        // TODO: don't assume the breakpoints are for the same file
        if self.breakpoints.is_empty() {
            return Ok(());
        }

        // group breakpoints by source file and send in multiple batches
        let breakpoints_by_source = self.breakpoints_by_source();

        for (source, breakpoints) in &breakpoints_by_source {
            let req = requests::RequestBody::SetBreakpoints(requests::SetBreakpoints {
                source: Source {
                    name: Some(source.display().to_string()),
                    path: Some(source.clone()),
                    ..Default::default()
                },
                lines: Some(breakpoints.iter().map(|b| b.line).collect()),
                breakpoints: Some(
                    breakpoints
                        .iter()
                        .map(|b| SourceBreakpoint {
                            line: b.line,
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            });

            tracing::debug!("sending broadcast breakpoints message");
            let _ = self
                .client
                .send(req)
                .context("broadcasting breakpoints to debugee")?;
            tracing::debug!("broadcast breakpoints message sent");
        }
        Ok(())
    }

    fn breakpoints_by_source(&self) -> HashMap<PathBuf, Vec<Breakpoint>> {
        let mut out = HashMap::new();
        for breakpoint in self.breakpoints.values() {
            let file_breakpoints = out.entry(breakpoint.path.clone()).or_insert(Vec::new());
            file_breakpoints.push(breakpoint.clone());
        }
        out
    }

    pub(crate) fn get_breakpoint_locations(
        &self,
        file: impl Into<PathBuf>,
    ) -> eyre::Result<Vec<BreakpointLocation>> {
        let req = requests::RequestBody::BreakpointLocations(requests::BreakpointLocations {
            source: Source {
                path: Some(file.into()),
                ..Default::default()
            },
            ..Default::default()
        });

        let res = self
            .client
            .send(req)
            .context("sending BreakpointLocations request")?;

        let Some(ResponseBody::BreakpointLocations(locations)) = res.body else {
            eyre::bail!("invalid response type: {:?}", res);
        };

        Ok(locations.breakpoints)
    }

    fn next_id(&mut self) -> BreakpointId {
        self.current_breakpoint_id += 1;
        self.current_breakpoint_id
    }

    #[tracing::instrument(skip(self), level = "trace")]
    pub(crate) fn set_state(&mut self, new_state: DebuggerState) {
        tracing::debug!("setting debugger state");
        let event = Event::from(&new_state);
        self.emit(event);
    }

    /// Non-blocking event processing that returns follow-up requests to make.
    ///
    /// This is the async-ready version of `on_event()` that doesn't make blocking
    /// calls to the transport layer. Instead, it returns a list of follow-up requests
    /// that should be made, which will later be processed by `on_follow_up_response()`.
    #[tracing::instrument(skip(self), level = "trace")]
    pub(crate) fn on_event_nonblocking(
        &mut self,
        event: transport::events::Event,
    ) -> Vec<FollowUpRequest> {
        tracing::debug!("handling event non-blocking");

        match event {
            transport::events::Event::Initialized => {
                self.set_state(DebuggerState::Initialised);
                Vec::new()
            }
            transport::events::Event::Stopped(transport::events::StoppedEventBody {
                thread_id,
                ..
            }) => {
                self.current_thread_id = Some(thread_id);
                // Request initial stack trace to get current location
                vec![FollowUpRequest::StackTrace {
                    thread_id,
                    levels: Some(1),
                    context: StackTraceContext::InitialLocation,
                }]
            }
            transport::events::Event::Continued(_) => {
                self.current_thread_id = None;
                self.current_source = None;
                self.set_state(DebuggerState::Running);
                Vec::new()
            }
            transport::events::Event::Exited(_) | transport::events::Event::Terminated => {
                self.set_state(DebuggerState::Ended);
                Vec::new()
            }
            _ => {
                tracing::debug!(?event, "unknown event");
                Vec::new()
            }
        }
    }

    /// Process the response to a follow-up request, potentially generating more follow-up requests.
    ///
    /// This is called when a response arrives for a request that was returned from
    /// `on_event_nonblocking()` or a previous call to this method.
    #[tracing::instrument(skip(self), level = "trace")]
    pub(crate) fn on_follow_up_response(
        &mut self,
        request: FollowUpRequest,
        response: responses::Response,
    ) -> Vec<FollowUpRequest> {
        tracing::debug!("handling follow-up response");

        match request {
            FollowUpRequest::StackTrace {
                thread_id, context, ..
            } => self.handle_stack_trace_response(thread_id, context, response),
            FollowUpRequest::Scopes { frame_id } => self.handle_scopes_response(frame_id, response),
            FollowUpRequest::Variables {
                variables_reference,
            } => self.handle_variables_response(variables_reference, response),
        }
    }

    fn handle_stack_trace_response(
        &mut self,
        thread_id: ThreadId,
        context: StackTraceContext,
        response: responses::Response,
    ) -> Vec<FollowUpRequest> {
        let stack_frames = match response {
            responses::Response {
                body:
                    Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
                        stack_frames,
                    })),
                success: true,
                ..
            } => stack_frames,
            resp => {
                tracing::error!(?resp, ?context, "unexpected response to StackTrace request");
                return Vec::new();
            }
        };

        match context {
            StackTraceContext::InitialLocation => {
                // Process initial stack trace (just top frame to get location)
                if stack_frames.is_empty() {
                    tracing::error!("no stack frames received in stopped event");
                    return Vec::new();
                }

                if stack_frames.len() != 1 {
                    tracing::warn!(
                        count = stack_frames.len(),
                        "unexpected number of stack frames, using first frame"
                    );
                }

                let Some(source) = stack_frames[0].source.as_ref() else {
                    tracing::error!("stack frame has no source information");
                    return Vec::new();
                };

                let line = stack_frames[0].line;
                let current_source = FileSource {
                    line,
                    file_path: source.path.clone(),
                };
                self.current_source = Some(current_source);

                // Now request full stack trace
                vec![FollowUpRequest::StackTrace {
                    thread_id,
                    levels: None,
                    context: StackTraceContext::FullStack,
                }]
            }
            StackTraceContext::FullStack => {
                // Process full stack trace and request scopes for top frame
                let Some(top_frame) = stack_frames.first() else {
                    tracing::error!("no frames found in full stack trace");
                    return Vec::new();
                };

                // Store the stack frames temporarily
                // We'll emit the full Paused event once we have the variables
                let frame_id = top_frame.id;

                // Request scopes for the top frame
                vec![FollowUpRequest::Scopes { frame_id }]
            }
        }
    }

    fn handle_scopes_response(
        &mut self,
        _frame_id: StackFrameId,
        response: responses::Response,
    ) -> Vec<FollowUpRequest> {
        let scopes = match response {
            responses::Response {
                body: Some(responses::ResponseBody::Scopes(responses::ScopesResponse { scopes })),
                success: true,
                ..
            } => scopes,
            resp => {
                tracing::error!(?resp, "unexpected response to Scopes request");
                return Vec::new();
            }
        };

        // Request variables for each scope
        scopes
            .into_iter()
            .map(|scope| FollowUpRequest::Variables {
                variables_reference: scope.variables_reference,
            })
            .collect()
    }

    fn handle_variables_response(
        &mut self,
        _variables_reference: i64,
        response: responses::Response,
    ) -> Vec<FollowUpRequest> {
        match response {
            responses::Response {
                body:
                    Some(responses::ResponseBody::Variables(responses::VariablesResponse {
                        variables: _scope_variables,
                    })),
                success: true,
                ..
            } => {
                // Variables received - we would accumulate these and emit Paused event
                // once all variables are collected. For now, this is a simplified version.
                // The full implementation will need to track state across multiple variable requests.
                tracing::debug!("variables received");
                Vec::new()
            }
            other => {
                tracing::error!(?other, "unexpected response to Variables request");
                Vec::new()
            }
        }
    }
}
