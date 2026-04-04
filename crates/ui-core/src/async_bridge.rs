use crossbeam_channel::Sender;
use dap_types::Variable;
use debugger::{AsyncEventReceiver, Breakpoint, EvaluateResult, Event, TcpAsyncDebugger};
use tokio::sync::{mpsc, oneshot};

type StackFrameId = i64;

/// Commands a UI frontend can send to the async debugger runtime.
pub enum UiCommand {
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
pub struct CommandError {
    pub operation: &'static str,
    pub error: eyre::Report,
}

/// The async bridge connects a synchronous UI render loop to the async debugger.
///
/// It runs a tokio runtime in a background thread and communicates via channels.
/// The `wakeup` callback is invoked after each event or error to prompt the UI
/// to redraw. For the TUI this sends to a crossbeam channel; for the GUI this
/// calls `egui::Context::request_repaint()`.
pub struct AsyncBridge {
    command_tx: mpsc::UnboundedSender<UiCommand>,
    error_rx: crossbeam_channel::Receiver<CommandError>,
    runtime_handle: tokio::runtime::Handle,
}

impl AsyncBridge {
    /// Spawn the async bridge, connecting to the debugger.
    ///
    /// Debugger events are forwarded to `event_tx`. The `wakeup` callback is
    /// called after each event or error to prompt the UI to redraw.
    pub fn spawn(
        debugger: TcpAsyncDebugger,
        mut event_receiver: AsyncEventReceiver,
        event_tx: Sender<Event>,
        wakeup: Box<dyn Fn() + Send + Sync + 'static>,
        runtime: tokio::runtime::Runtime,
    ) -> eyre::Result<Self> {
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<UiCommand>();
        let (error_tx, error_rx) = crossbeam_channel::unbounded::<CommandError>();

        let handle = runtime.handle().clone();

        let wakeup2 = std::sync::Arc::new(wakeup);
        let wakeup_for_events = wakeup2.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Spawn event forwarding task
                let event_forward_handle = tokio::spawn(async move {
                    while let Some(event) = event_receiver.recv().await {
                        if event_tx.send(event).is_err() {
                            tracing::debug!("event channel closed");
                            break;
                        }
                        (wakeup_for_events)();
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
                                (wakeup2)();
                            }
                        }
                        UiCommand::StepOver => {
                            if let Err(e) = debugger.step_over().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step over",
                                    error: e,
                                });
                                (wakeup2)();
                            }
                        }
                        UiCommand::StepIn => {
                            if let Err(e) = debugger.step_in().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step into",
                                    error: e,
                                });
                                (wakeup2)();
                            }
                        }
                        UiCommand::StepOut => {
                            if let Err(e) = debugger.step_out().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step out",
                                    error: e,
                                });
                                (wakeup2)();
                            }
                        }
                        UiCommand::Evaluate {
                            expression,
                            frame_id,
                            reply,
                        } => {
                            let result = debugger.evaluate(&expression, frame_id).await;
                            let _ = reply.send(result);
                            (wakeup2)();
                        }
                        UiCommand::AddBreakpoint { breakpoint, reply } => {
                            let result = debugger.add_breakpoint(&breakpoint).await;
                            let _ = reply.send(result);
                            (wakeup2)();
                        }
                        UiCommand::RemoveBreakpoint { id, reply } => {
                            let result = debugger.remove_breakpoint(id).await;
                            let _ = reply.send(result);
                            (wakeup2)();
                        }
                        UiCommand::FetchVariables { reference, reply } => {
                            let result = debugger.variables(reference).await;
                            let _ = reply.send(result);
                            (wakeup2)();
                        }
                        UiCommand::ChangeScope { frame_id, reply } => {
                            let result = debugger.change_scope(frame_id).await;
                            let _ = reply.send(result);
                            (wakeup2)();
                        }
                        UiCommand::Terminate => {
                            if let Err(e) = debugger.terminate().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Terminate",
                                    error: e,
                                });
                                (wakeup2)();
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
    /// Use for operations where the UI needs the result immediately.
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
