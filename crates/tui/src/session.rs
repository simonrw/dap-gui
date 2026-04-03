use std::path::PathBuf;

use crossbeam_channel::Sender;
use debugger::{Breakpoint, Event, ProgramState};
use eyre::Context;
use launch_configuration::{Debugpy, LaunchConfiguration};

use crate::async_bridge::AsyncBridge;

type StackFrameId = i64;

/// The state of the debugger as seen by the TUI.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Fields used as phases are implemented
pub(crate) enum DebuggerState {
    /// Session exists but hasn't reached the first pause yet.
    Running,
    /// Execution is paused at a breakpoint or step.
    Paused {
        stack: Vec<dap_types::StackFrame>,
        paused_frame: Box<debugger::PausedFrame>,
        breakpoints: Vec<Breakpoint>,
    },
    /// The debugee has terminated.
    Terminated,
}

impl DebuggerState {
    /// Apply a debugger event, returning the new state.
    pub fn apply(self, event: &Event) -> Self {
        match event {
            Event::Initialised | Event::Running => DebuggerState::Running,
            Event::Paused(ProgramState {
                stack,
                paused_frame,
                breakpoints,
            })
            | Event::ScopeChange(ProgramState {
                stack,
                paused_frame,
                breakpoints,
            }) => DebuggerState::Paused {
                stack: stack.clone(),
                paused_frame: Box::new(paused_frame.clone()),
                breakpoints: breakpoints.clone(),
            },
            Event::Ended => DebuggerState::Terminated,
            // Uninitialised, Output, Thread, Error don't change core state
            _ => self,
        }
    }
}

/// An active debug session, bundling the async bridge and server process.
///
/// Dropping this struct tears down the session: the bridge command channel
/// closes, which causes the async runtime to shut down. The server process
/// (if any) is killed on drop.
pub(crate) struct Session {
    pub bridge: AsyncBridge,
    pub state: DebuggerState,
    pub current_frame_id: Option<StackFrameId>,
    /// Channel to receive debugger events. The bridge's event forwarding task
    /// sends events here; the app drains this each tick.
    pub debugger_event_rx: crossbeam_channel::Receiver<Event>,
    _server: Option<Box<dyn server::Server + Send>>,
}

impl Session {
    /// Start a new debug session from a launch configuration.
    ///
    /// This spawns the DAP server (for launch mode), connects to the debugger,
    /// configures breakpoints, and starts execution.
    ///
    /// The `wakeup_tx` channel is used to nudge the TUI event loop when a
    /// debugger event arrives, ensuring the UI redraws promptly.
    pub fn start(
        config: &LaunchConfiguration,
        breakpoints: &[Breakpoint],
        debug_root_dir: &mut PathBuf,
        wakeup_tx: Sender<()>,
    ) -> eyre::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .map_err(|e| eyre::eyre!("failed to create tokio runtime: {e}"))?;

        let mut server_handle: Option<Box<dyn server::Server + Send>> = None;

        let mut debugger = rt.block_on(async {
            match config {
                LaunchConfiguration::Python(debugpy) | LaunchConfiguration::Debugpy(debugpy) => {
                    let Debugpy {
                        request,
                        cwd,
                        connect,
                        path_mappings,
                        program,
                        ..
                    } = debugpy.clone();

                    if let Some(dir) = cwd {
                        *debug_root_dir =
                            std::fs::canonicalize(debugger::utils::normalise_path(&dir).as_ref())
                                .unwrap_or_else(|_| {
                                    debugger::utils::normalise_path(&dir).into_owned()
                                });
                    }

                    match request.as_str() {
                        "attach" => {
                            let attach_args = debugger::AttachArguments {
                                working_directory: debug_root_dir.to_owned(),
                                port: connect.as_ref().map(|c| c.port),
                                host: connect.map(|c| c.host),
                                language: debugger::Language::DebugPy,
                                path_mappings,
                                just_my_code: None,
                            };

                            let port = attach_args.port.unwrap_or(server::DEFAULT_DAP_PORT);
                            let debugger = debugger::TcpAsyncDebugger::connect(
                                port,
                                debugger::Language::DebugPy,
                                debugger::SessionArgs::Attach(attach_args),
                                debugger::StartMode::Staged,
                            )
                            .await
                            .context("creating async debugger (attach)")?;

                            Ok::<_, eyre::Report>(debugger)
                        }
                        "launch" => {
                            let Some(program) = program else {
                                eyre::bail!("'program' is a required setting");
                            };

                            let program = std::fs::canonicalize(&program).unwrap_or(program);

                            let port = server::DEFAULT_DAP_PORT;
                            server_handle = Some(
                                server::for_implementation_on_port(
                                    server::Implementation::Debugpy,
                                    port,
                                )
                                .context("creating background server process")?,
                            );

                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                            let launch_args = debugger::LaunchArguments {
                                program: Some(program),
                                module: None,
                                args: None,
                                env: None,
                                working_directory: Some(debug_root_dir.to_owned()),
                                language: debugger::Language::DebugPy,
                                just_my_code: debugpy.just_my_code,
                                stop_on_entry: debugpy.stop_on_entry,
                            };

                            let debugger = debugger::TcpAsyncDebugger::connect(
                                port,
                                debugger::Language::DebugPy,
                                debugger::SessionArgs::Launch(launch_args),
                                debugger::StartMode::Staged,
                            )
                            .await
                            .context("creating async debugger (launch)")?;

                            Ok(debugger)
                        }
                        other => eyre::bail!("unsupported request type: {other}"),
                    }
                }
                other => eyre::bail!("unsupported configuration: {other:?}"),
            }
        })?;

        rt.block_on(async {
            debugger
                .configure_breakpoints(breakpoints)
                .await
                .context("configuring breakpoints")?;

            tracing::debug!("launching debugee");
            debugger.start().await.context("launching debugee")
        })?;

        // Channel for debugger events: bridge -> app
        let (debugger_event_tx, debugger_event_rx) = crossbeam_channel::unbounded();
        let event_receiver = debugger.take_events();

        let bridge = AsyncBridge::spawn(debugger, event_receiver, debugger_event_tx, wakeup_tx, rt)
            .context("creating async bridge")?;

        Ok(Self {
            bridge,
            state: DebuggerState::Running,
            current_frame_id: None,
            debugger_event_rx,
            _server: server_handle,
        })
    }

    /// Process a debugger event, updating session state.
    pub fn handle_event(&mut self, event: &Event) {
        let old_state = self.state.clone();
        self.state = old_state.apply(event);

        if let DebuggerState::Paused { paused_frame, .. } = &self.state {
            self.current_frame_id = Some(paused_frame.frame.id);
        } else if matches!(self.state, DebuggerState::Running) {
            self.current_frame_id = None;
        }
    }
}
