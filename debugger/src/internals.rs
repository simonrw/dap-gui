use eyre::WrapErr;
use server::Server;
use std::{collections::HashMap, path::PathBuf};
use transport::{
    requests::{self, Initialize, PathFormat},
    responses::{self, ResponseBody},
    types::{BreakpointLocation, Source, SourceBreakpoint, StackFrame, StackFrameId, ThreadId},
    Client,
};

use crate::{
    debugger::InitialiseArguments,
    state::DebuggerState,
    types::{Breakpoint, BreakpointId, PausedFrame},
    Event,
};

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
        self.emit(Event::ScopeChange {
            stack: stack_frames,
            breakpoints: self.breakpoints.values().cloned().collect(),
            paused_frame,
        });

        Ok(())
    }

    fn compute_paused_frame(&self, stack_frame: &StackFrame) -> eyre::Result<PausedFrame> {
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
            let req = requests::RequestBody::Variables(requests::Variables {
                variables_reference: scope.variables_reference,
            });
            match self.client.send(req).expect("fetching variables") {
                responses::Response {
                    body:
                        Some(responses::ResponseBody::Variables(responses::VariablesResponse {
                            variables: scope_variables,
                        })),
                    success: true,
                    ..
                } => variables.extend(scope_variables.into_iter()),
                r => {
                    tracing::warn!(?r, "unhandled response from send variables request")
                }
            };
        }
        let paused_frame = PausedFrame {
            frame: stack_frame.clone(),
            variables,
        };

        Ok(paused_frame)
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
                        thread_id,
                        levels: Some(1),
                        ..Default::default()
                    }))
                    .unwrap()
                else {
                    unreachable!()
                };

                if stack_frames.len() != 1 {
                    panic!("unexpected number of stack frames: {}", stack_frames.len());
                }

                let source = stack_frames[0].source.as_ref().unwrap();
                let line = stack_frames[0].line;

                let current_source = FileSource {
                    line,
                    file_path: source.path.clone(),
                };
                self.current_source = Some(current_source.clone());

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
                        thread_id,
                        ..Default::default()
                    }))
                    .unwrap()
                else {
                    unreachable!()
                };

                let top_frame = stack_frames.first().expect("no frames found");
                let paused_frame = self
                    .compute_paused_frame(top_frame)
                    .expect("building paused frame construct");

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
    }

    #[tracing::instrument(skip(self), level = "trace")]
    pub(crate) fn add_breakpoint(&mut self, breakpoint: &Breakpoint) -> eyre::Result<BreakpointId> {
        tracing::debug!("adding breakpoint");
        let id = self.next_id();
        self.breakpoints.insert(id, breakpoint.clone());
        self.broadcast_breakpoints()
            .context("updating breakpoints with debugee")?;
        Ok(id)
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) fn remove_breakpoint(&mut self, id: BreakpointId) {
        tracing::debug!("removing breakpoint");
        self.breakpoints.remove(&id);
        self.broadcast_breakpoints()
            .expect("updating breakpoints with debugee");
    }

    fn broadcast_breakpoints(&mut self) -> eyre::Result<()> {
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

            let _ = self
                .client
                .send(req)
                .context("broadcasting breakpoints to debugee")?;
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
}
