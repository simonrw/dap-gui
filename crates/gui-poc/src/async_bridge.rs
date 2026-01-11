use debugger::{Breakpoint, EvaluateResult, Event, Language, LaunchArguments, TcpAsyncDebugger};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use transport::types::{StackFrameId, Variable};

/// Commands sent from UI to async runtime
#[derive(Debug)]
pub enum UiCommand {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    AddBreakpoint(Breakpoint),
    RemoveBreakpoint(u64),
    Evaluate(String, StackFrameId),
    ChangeScope(StackFrameId),
    FetchVariables(i64),
}

/// State updates received from async runtime
pub enum StateUpdate {
    DebuggerEvent(Event),
    EvaluateResult(EvaluateResult),
    VariablesResult(Vec<Variable>),
    Error(String),
}

/// Bridge between the async debugger runtime and the synchronous UI
pub struct AsyncBridge {
    runtime: Runtime,
    command_tx: mpsc::UnboundedSender<UiCommand>,
    update_rx: mpsc::UnboundedReceiver<StateUpdate>,
}

impl AsyncBridge {
    /// Create a new async bridge and connect to the debugger
    pub fn new(port: u16, language: Language, launch_args: LaunchArguments) -> eyre::Result<Self> {
        let runtime = Runtime::new()?;
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Spawn the debugger management task
        runtime.spawn(Self::run_debugger(
            port,
            language,
            launch_args,
            command_rx,
            update_tx,
        ));

        Ok(Self {
            runtime,
            command_tx,
            update_rx,
        })
    }

    /// Run the debugger in an async context
    async fn run_debugger(
        port: u16,
        language: Language,
        launch_args: LaunchArguments,
        mut command_rx: mpsc::UnboundedReceiver<UiCommand>,
        update_tx: mpsc::UnboundedSender<StateUpdate>,
    ) {
        // Connect to debugger
        let mut debugger: TcpAsyncDebugger =
            match TcpAsyncDebugger::connect(port, language, &launch_args, true).await {
                Ok(d) => d,
                Err(e) => {
                    let _ = update_tx.send(StateUpdate::Error(format!("{}", e)));
                    return;
                }
            };

        // Start the debug session
        if let Err(e) = debugger.start().await {
            let _ = update_tx.send(StateUpdate::Error(format!("{}", e)));
            return;
        }

        loop {
            tokio::select! {
                // Handle commands from UI
                Some(cmd) = command_rx.recv() => {
                    let result: eyre::Result<()> = match cmd {
                        UiCommand::Continue => debugger.continue_().await,
                        UiCommand::StepOver => debugger.step_over().await,
                        UiCommand::StepIn => debugger.step_in().await,
                        UiCommand::StepOut => debugger.step_out().await,
                        UiCommand::AddBreakpoint(bp) => {
                            debugger.add_breakpoint(&bp).await.map(|_| ())
                        }
                        UiCommand::RemoveBreakpoint(id) => {
                            debugger.remove_breakpoint(id).await
                        }
                        UiCommand::Evaluate(expr, frame_id) => {
                            match debugger.evaluate(&expr, frame_id).await {
                                Ok(result) => {
                                    let _ = update_tx.send(StateUpdate::EvaluateResult(result));
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        UiCommand::ChangeScope(frame_id) => {
                            debugger.change_scope(frame_id).await
                        }
                        UiCommand::FetchVariables(var_ref) => {
                            match debugger.variables(var_ref).await {
                                Ok(vars) => {
                                    let _ = update_tx.send(StateUpdate::VariablesResult(vars));
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                    };

                    if let Err(e) = result {
                        let _ = update_tx.send(StateUpdate::Error(format!("{}", e)));
                    }
                }

                // Handle events from debugger
                Some(event) = debugger.events().recv() => {
                    let _ = update_tx.send(StateUpdate::DebuggerEvent(event));
                }

                else => break,
            }
        }
    }

    /// Send command to async runtime (non-blocking)
    pub fn send_command(&self, cmd: UiCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Poll for state updates (non-blocking)
    pub fn poll_updates(&mut self) -> Vec<StateUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}
