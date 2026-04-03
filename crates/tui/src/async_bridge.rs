use crossbeam_channel::Sender;
use dap_types::Variable;
use debugger::{AsyncEventReceiver, Breakpoint, EvaluateResult, Event, TcpAsyncDebugger};
use tokio::sync::{mpsc, oneshot};

type StackFrameId = i64;

/// Commands the TUI can send to the async debugger runtime.
#[allow(dead_code)] // Variants used as phases are implemented
pub(crate) enum UiCommand {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Evaluate {
        expression: String,
        frame_id: StackFrameId,
        reply: oneshot::Sender<eyre::Result<EvaluateResult>>,
    },
    AddBreakpoint {
        breakpoint: Breakpoint,
        reply: oneshot::Sender<eyre::Result<u64>>,
    },
    RemoveBreakpoint {
        id: u64,
        reply: oneshot::Sender<eyre::Result<()>>,
    },
    FetchVariables {
        reference: i64,
        reply: oneshot::Sender<eyre::Result<Vec<Variable>>>,
    },
    ChangeScope {
        frame_id: StackFrameId,
        reply: oneshot::Sender<eyre::Result<()>>,
    },
    Terminate,
    Shutdown,
}

/// Result of an async debugger operation (for fire-and-forget commands).
pub(crate) struct CommandError {
    pub operation: &'static str,
    pub error: eyre::Report,
}

/// The async bridge connects the synchronous TUI render loop to the async debugger.
///
/// Unlike the GUI bridge which uses `egui::Context::request_repaint()` for wakeup,
/// this version sends events through a crossbeam channel that the TUI event loop
/// can select on alongside terminal input and tick events.
pub(crate) struct AsyncBridge {
    command_tx: mpsc::UnboundedSender<UiCommand>,
    error_rx: crossbeam_channel::Receiver<CommandError>,
    #[allow(dead_code)] // Used in Phase 4 for send_sync
    runtime_handle: tokio::runtime::Handle,
}

impl AsyncBridge {
    /// Spawn the async bridge, connecting to the debugger.
    ///
    /// Debugger events are forwarded to `event_tx` which feeds into the TUI's
    /// unified event loop. The `wakeup_tx` is sent a unit value after each
    /// event to ensure the TUI redraws.
    pub fn spawn(
        debugger: TcpAsyncDebugger,
        mut event_receiver: AsyncEventReceiver,
        event_tx: crossbeam_channel::Sender<Event>,
        wakeup_tx: Sender<()>,
        runtime: tokio::runtime::Runtime,
    ) -> eyre::Result<Self> {
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<UiCommand>();
        let (error_tx, error_rx) = crossbeam_channel::unbounded::<CommandError>();

        let handle = runtime.handle().clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Spawn event forwarding task
                let wakeup = wakeup_tx.clone();
                let event_forward_handle = tokio::spawn(async move {
                    while let Some(event) = event_receiver.recv().await {
                        if event_tx.send(event).is_err() {
                            tracing::debug!("event channel closed");
                            break;
                        }
                        let _ = wakeup.send(());
                    }
                    tracing::debug!("event forwarding task ended");
                });

                // Process UI commands
                while let Some(cmd) = command_rx.recv().await {
                    match cmd {
                        UiCommand::Continue => {
                            if let Err(e) = debugger.continue_().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Continue",
                                    error: e,
                                });
                                let _ = wakeup_tx.send(());
                            }
                        }
                        UiCommand::StepOver => {
                            if let Err(e) = debugger.step_over().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step over",
                                    error: e,
                                });
                                let _ = wakeup_tx.send(());
                            }
                        }
                        UiCommand::StepIn => {
                            if let Err(e) = debugger.step_in().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step into",
                                    error: e,
                                });
                                let _ = wakeup_tx.send(());
                            }
                        }
                        UiCommand::StepOut => {
                            if let Err(e) = debugger.step_out().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step out",
                                    error: e,
                                });
                                let _ = wakeup_tx.send(());
                            }
                        }
                        UiCommand::Evaluate {
                            expression,
                            frame_id,
                            reply,
                        } => {
                            let result = debugger.evaluate(&expression, frame_id).await;
                            let _ = reply.send(result);
                            let _ = wakeup_tx.send(());
                        }
                        UiCommand::AddBreakpoint { breakpoint, reply } => {
                            let result = debugger.add_breakpoint(&breakpoint).await;
                            let _ = reply.send(result);
                            let _ = wakeup_tx.send(());
                        }
                        UiCommand::RemoveBreakpoint { id, reply } => {
                            let result = debugger.remove_breakpoint(id).await;
                            let _ = reply.send(result);
                            let _ = wakeup_tx.send(());
                        }
                        UiCommand::FetchVariables { reference, reply } => {
                            let result = debugger.variables(reference).await;
                            let _ = reply.send(result);
                            let _ = wakeup_tx.send(());
                        }
                        UiCommand::ChangeScope { frame_id, reply } => {
                            let result = debugger.change_scope(frame_id).await;
                            let _ = reply.send(result);
                            let _ = wakeup_tx.send(());
                        }
                        UiCommand::Terminate => {
                            if let Err(e) = debugger.terminate().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Terminate",
                                    error: e,
                                });
                                let _ = wakeup_tx.send(());
                            }
                        }
                        UiCommand::Shutdown => {
                            // Shutdown consumes the debugger, so we break out of the loop
                            if let Err(e) = debugger.shutdown().await {
                                tracing::warn!(error = %e, "error during shutdown");
                            }
                            break;
                        }
                    }
                }

                tracing::debug!("command loop ended, aborting event forwarder");
                event_forward_handle.abort();
            });
            tracing::debug!("async bridge runtime ended");
        });

        Ok(Self {
            command_tx,
            error_rx,
            runtime_handle: handle,
        })
    }

    /// Send a fire-and-forget command (continue, step, terminate, etc.).
    pub fn send(&self, cmd: UiCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Send a command and get a reply synchronously (blocks briefly).
    /// Use for operations where the TUI needs the result immediately.
    #[allow(dead_code)] // Used in Phase 4 for evaluate/breakpoint operations
    pub fn send_sync<T>(
        &self,
        make_cmd: impl FnOnce(oneshot::Sender<eyre::Result<T>>) -> UiCommand,
    ) -> eyre::Result<T> {
        let (tx, rx) = oneshot::channel();
        let cmd = make_cmd(tx);
        self.command_tx
            .send(cmd)
            .map_err(|_| eyre::eyre!("command channel closed"))?;
        self.runtime_handle
            .block_on(async { rx.await.map_err(|_| eyre::eyre!("reply channel closed"))? })
    }

    /// Drain any pending errors from fire-and-forget commands.
    pub fn drain_errors(&self) -> Vec<CommandError> {
        self.error_rx.try_iter().collect()
    }
}
