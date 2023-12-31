use eyre::WrapErr;
use server::Server;
use std::{collections::HashMap, path::PathBuf};
use transport::{
    requests::{self, Initialize, PathFormat},
    responses,
    types::{Source, SourceBreakpoint, ThreadId},
    Client,
};

use crate::{
    debugger::InitialiseArguments,
    state::DebuggerState,
    types::{Breakpoint, BreakpointId},
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

    pub(crate) fn emit(&mut self, event: Event) {
        let _ = self.publisher.send(event);
    }

    pub(crate) fn initialise(&mut self, arguments: InitialiseArguments) -> eyre::Result<()> {
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

    #[tracing::instrument(skip(self))]
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
                let Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
                    stack_frames,
                })) = self
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

                let Some(responses::ResponseBody::StackTrace(responses::StackTraceResponse {
                    stack_frames,
                })) = self
                    .client
                    .send(requests::RequestBody::StackTrace(requests::StackTrace {
                        thread_id,
                        ..Default::default()
                    }))
                    .unwrap()
                else {
                    unreachable!()
                };

                self.set_state(DebuggerState::Paused {
                    stack: stack_frames,
                    source: current_source,
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
                tracing::debug!("unknown event");
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn add_breakpoint(&mut self, breakpoint: Breakpoint) -> eyre::Result<BreakpointId> {
        tracing::debug!("adding breakpoint");
        let id = self.next_id();
        self.breakpoints.insert(id, breakpoint.clone());
        self.broadcast_breakpoints()
            .context("updating breakpoints with debugee")?;
        Ok(id)
    }

    #[tracing::instrument(skip(self))]
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

    fn next_id(&mut self) -> BreakpointId {
        self.current_breakpoint_id += 1;
        self.current_breakpoint_id
    }

    pub(crate) fn set_state(&mut self, new_state: DebuggerState) {
        let event = Event::from(&new_state);
        self.emit(event);
    }
}
