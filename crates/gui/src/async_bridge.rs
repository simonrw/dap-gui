use std::sync::{Arc, Mutex};

use debugger::{AsyncEventReceiver, Breakpoint, EvaluateResult, Event, TcpAsyncDebugger};
use eframe::egui;
use tokio::sync::{mpsc, oneshot};
use transport::types::{StackFrameId, Variable};

/// Commands the GUI can send to the async debugger runtime
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
}

/// Result of an async debugger operation (for fire-and-forget commands)
pub(crate) struct CommandError {
    pub operation: &'static str,
    pub error: eyre::Report,
}

/// The async bridge connects the synchronous egui render loop to the async debugger.
///
/// It runs a tokio runtime in a background thread and communicates via channels.
pub(crate) struct AsyncBridge {
    command_tx: mpsc::UnboundedSender<UiCommand>,
    error_rx: Arc<Mutex<crossbeam_channel::Receiver<CommandError>>>,
    runtime_handle: tokio::runtime::Handle,
}

impl AsyncBridge {
    /// Spawn the async bridge, connecting to the debugger.
    ///
    /// Returns the bridge and a crossbeam receiver for debugger events (for the GUI thread).
    pub fn spawn(
        debugger: TcpAsyncDebugger,
        mut event_receiver: AsyncEventReceiver,
        event_tx: crossbeam_channel::Sender<Event>,
        egui_ctx: egui::Context,
        runtime: tokio::runtime::Runtime,
    ) -> eyre::Result<Self> {
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<UiCommand>();
        let (error_tx, error_rx) = crossbeam_channel::unbounded::<CommandError>();

        let handle = runtime.handle().clone();

        // Spawn the main async loop in the runtime
        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Spawn event forwarding task
                let event_egui_ctx = egui_ctx.clone();
                let event_forward_handle = tokio::spawn(async move {
                    while let Some(event) = event_receiver.recv().await {
                        if event_tx.send(event).is_err() {
                            tracing::debug!("event channel closed");
                            break;
                        }
                        event_egui_ctx.request_repaint();
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
                                egui_ctx.request_repaint();
                            }
                        }
                        UiCommand::StepOver => {
                            if let Err(e) = debugger.step_over().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step over",
                                    error: e,
                                });
                                egui_ctx.request_repaint();
                            }
                        }
                        UiCommand::StepIn => {
                            if let Err(e) = debugger.step_in().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step into",
                                    error: e,
                                });
                                egui_ctx.request_repaint();
                            }
                        }
                        UiCommand::StepOut => {
                            if let Err(e) = debugger.step_out().await {
                                let _ = error_tx.send(CommandError {
                                    operation: "Step out",
                                    error: e,
                                });
                                egui_ctx.request_repaint();
                            }
                        }
                        UiCommand::Evaluate {
                            expression,
                            frame_id,
                            reply,
                        } => {
                            let result = debugger.evaluate(&expression, frame_id).await;
                            let _ = reply.send(result);
                            egui_ctx.request_repaint();
                        }
                        UiCommand::AddBreakpoint { breakpoint, reply } => {
                            let result = debugger.add_breakpoint(&breakpoint).await;
                            let _ = reply.send(result);
                            egui_ctx.request_repaint();
                        }
                        UiCommand::RemoveBreakpoint { id, reply } => {
                            let result = debugger.remove_breakpoint(id).await;
                            let _ = reply.send(result);
                            egui_ctx.request_repaint();
                        }
                        UiCommand::FetchVariables { reference, reply } => {
                            let result = debugger.variables(reference).await;
                            let _ = reply.send(result);
                            egui_ctx.request_repaint();
                        }
                        UiCommand::ChangeScope { frame_id, reply } => {
                            let result = debugger.change_scope(frame_id).await;
                            let _ = reply.send(result);
                            egui_ctx.request_repaint();
                        }
                    }
                }

                tracing::debug!("command loop ended, waiting for event forwarder");
                event_forward_handle.abort();
            });
            tracing::debug!("async bridge runtime ended");
        });

        Ok(Self {
            command_tx,
            error_rx: Arc::new(Mutex::new(error_rx)),
            runtime_handle: handle,
        })
    }

    /// Send a fire-and-forget command (continue, step, etc.)
    pub fn send(&self, cmd: UiCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Send a command and get a reply synchronously (blocks briefly).
    /// Use for operations where the GUI needs the result immediately.
    pub fn send_sync<T>(
        &self,
        make_cmd: impl FnOnce(oneshot::Sender<eyre::Result<T>>) -> UiCommand,
    ) -> eyre::Result<T> {
        let (tx, rx) = oneshot::channel();
        let cmd = make_cmd(tx);
        self.command_tx
            .send(cmd)
            .map_err(|_| eyre::eyre!("command channel closed"))?;
        // Block on the oneshot - this is fine for short operations
        self.runtime_handle
            .block_on(async { rx.await.map_err(|_| eyre::eyre!("reply channel closed"))? })
    }

    /// Drain any pending errors from fire-and-forget commands
    pub fn drain_errors(&self) -> Vec<CommandError> {
        let rx = self.error_rx.lock().unwrap();
        rx.try_iter().collect()
    }
}
