use anyhow::Context;
use std::collections::HashMap;
use transport::{
    requests, responses,
    types::{Source, SourceBreakpoint, ThreadId},
    Client,
};

use crate::{
    state::DebuggerState,
    types::{Breakpoint, BreakpointId},
    Event,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FileSource {
    pub line: isize,
    pub contents: String,
}

pub(crate) struct DebuggerInternals {
    pub(crate) _state: DebuggerState,
    pub(crate) client: Client,
    pub(crate) publisher: spmc::Sender<Event>,

    // debugger specific details
    pub(crate) current_thread_id: Option<ThreadId>,
    pub(crate) breakpoints: HashMap<BreakpointId, Breakpoint>,

    current_breakpoint_id: BreakpointId,
    pub(crate) current_source: Option<FileSource>,
}

impl DebuggerInternals {
    pub(crate) fn new(client: Client, publisher: spmc::Sender<Event>) -> Self {
        Self::with_breakpoints(client, publisher, HashMap::new())
    }

    pub(crate) fn with_breakpoints(
        client: Client,
        publisher: spmc::Sender<Event>,
        existing_breakpoints: impl Into<HashMap<BreakpointId, Breakpoint>>,
    ) -> Self {
        let breakpoints = existing_breakpoints.into();
        let current_breakpoint_id = *breakpoints.keys().max().unwrap_or(&0);

        Self {
            _state: DebuggerState::Running,
            client,
            publisher,
            current_thread_id: None,
            breakpoints,
            current_breakpoint_id,
            current_source: None,
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
                let contents = std::fs::read_to_string(source.path.as_ref().unwrap()).unwrap();
                let line = stack_frames[0].line;

                let current_source = FileSource { contents, line };
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
    pub(crate) fn add_breakpoint(&mut self, breakpoint: Breakpoint) -> BreakpointId {
        tracing::debug!("adding breakpoint");
        let id = self.next_id();
        self.breakpoints.insert(id, breakpoint.clone());
        self.broadcast_breakpoints()
            .expect("updating breakpoints with debugee");
        id
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn remove_breakpoint(&mut self, id: BreakpointId) {
        tracing::debug!("removing breakpoint");
        self.breakpoints.remove(&id);
        self.broadcast_breakpoints()
            .expect("updating breakpoints with debugee");
    }

    fn broadcast_breakpoints(&mut self) -> anyhow::Result<()> {
        // TODO: don't assume the breakpoints are for the same file
        if self.breakpoints.is_empty() {
            return Ok(());
        }

        let first_breakpoint = self.breakpoints.values().next().unwrap();

        let req = requests::RequestBody::SetBreakpoints(requests::SetBreakpoints {
            source: Source {
                name: first_breakpoint.name.clone(),
                path: Some(first_breakpoint.path.clone()),
                ..Default::default()
            },
            lines: Some(self.breakpoints.values().map(|b| b.line).collect()),
            breakpoints: Some(
                self.breakpoints
                    .values()
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
        Ok(())
    }

    fn next_id(&mut self) -> BreakpointId {
        self.current_breakpoint_id += 1;
        self.current_breakpoint_id
    }

    pub(crate) fn set_state(&mut self, new_state: DebuggerState) {
        let event = Event::from(&new_state);
        let _ = self.publisher.send(event);
    }
}
