//! Async bridge for communicating between the synchronous TUI event loop
//! and the async debugger runtime.
//!
//! This module provides a channel-based bridge that allows the ratatui UI
//! to send commands to and receive events from the async debugger without
//! blocking the UI thread.

use debugger::{Breakpoint, EvaluateResult, Event, InitialiseArguments, TcpAsyncDebugger};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use transport::types::{StackFrameId, Variable};

/// Commands sent from the TUI to the async runtime.
#[derive(Debug)]
pub enum UiCommand {
    /// Continue execution
    Continue,
    /// Step over the current line
    StepOver,
    /// Step into the current function call
    StepIn,
    /// Step out of the current function
    StepOut,
    /// Terminate the debug session
    Terminate,
    /// Add a breakpoint
    AddBreakpoint(Breakpoint),
    /// Remove a breakpoint by ID
    RemoveBreakpoint(u64),
    /// Evaluate an expression in the given stack frame
    Evaluate(String, StackFrameId),
    /// Change the current scope to a different stack frame
    ChangeScope(StackFrameId),
    /// Fetch variables for a given variable reference
    FetchVariables(i64),
}

/// State updates received from the async runtime.
pub enum StateUpdate {
    /// A debugger event occurred
    DebuggerEvent(Event),
    /// Result of an expression evaluation
    EvaluateResult(EvaluateResult),
    /// Variables fetched from the debugger
    VariablesResult(Vec<Variable>),
    /// An error occurred
    Error(String),
}

/// Bridge between the async debugger runtime and the synchronous TUI.
///
/// The bridge maintains a tokio runtime and provides non-blocking methods
/// to send commands and poll for updates, making it suitable for use in
/// a synchronous event loop.
///
/// # Example
///
/// ```ignore
/// let bridge = AsyncBridge::new(port, Language::Python, launch_args)?;
///
/// // In the event loop:
/// // Send commands (non-blocking)
/// bridge.send_command(UiCommand::Continue);
///
/// // Poll for updates (non-blocking)
/// for update in bridge.poll_updates() {
///     match update {
///         StateUpdate::DebuggerEvent(event) => { /* handle event */ }
///         StateUpdate::Error(msg) => { /* handle error */ }
///         // ...
///     }
/// }
/// ```
pub struct AsyncBridge {
    #[allow(dead_code)]
    runtime: Runtime,
    command_tx: mpsc::UnboundedSender<UiCommand>,
    update_rx: mpsc::UnboundedReceiver<StateUpdate>,
}

impl AsyncBridge {
    /// Create a new async bridge and connect to the debugger.
    ///
    /// This spawns a tokio runtime and a background task that manages
    /// the debugger connection. No initial breakpoints are configured.
    ///
    /// # Arguments
    ///
    /// * `port` - The port to connect to the debug adapter
    /// * `init_args` - Arguments for initializing the debug session (Launch or Attach)
    pub fn new(port: u16, init_args: InitialiseArguments) -> eyre::Result<Self> {
        Self::with_breakpoints(port, init_args, vec![])
    }

    /// Create a new async bridge with initial breakpoints.
    ///
    /// This spawns a tokio runtime and a background task that manages
    /// the debugger connection. The provided breakpoints are configured
    /// before the debuggee starts executing.
    ///
    /// # Arguments
    ///
    /// * `port` - The port to connect to the debug adapter
    /// * `init_args` - Arguments for initializing the debug session (Launch or Attach)
    /// * `initial_breakpoints` - Breakpoints to configure before starting execution
    pub fn with_breakpoints(
        port: u16,
        init_args: InitialiseArguments,
        initial_breakpoints: Vec<Breakpoint>,
    ) -> eyre::Result<Self> {
        let runtime = Runtime::new()?;
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Spawn the debugger management task
        runtime.spawn(Self::run_debugger(
            port,
            init_args,
            initial_breakpoints,
            command_rx,
            update_tx,
        ));

        Ok(Self {
            runtime,
            command_tx,
            update_rx,
        })
    }

    /// Run the debugger in an async context.
    ///
    /// This is the main async task that:
    /// 1. Connects to the debug adapter (staged)
    /// 2. Configures initial breakpoints
    /// 3. Starts the debug session (sends ConfigurationDone)
    /// 4. Processes commands from the UI
    /// 5. Forwards events to the UI
    async fn run_debugger(
        port: u16,
        init_args: InitialiseArguments,
        initial_breakpoints: Vec<Breakpoint>,
        mut command_rx: mpsc::UnboundedReceiver<UiCommand>,
        update_tx: mpsc::UnboundedSender<StateUpdate>,
    ) {
        // Connect to debugger using staged initialization
        let mut debugger: TcpAsyncDebugger = match &init_args {
            InitialiseArguments::Launch(launch_args) => {
                match TcpAsyncDebugger::connect_staged(
                    port,
                    launch_args.language,
                    launch_args,
                    true,
                )
                .await
                {
                    Ok(d) => d,
                    Err(e) => {
                        let _ =
                            update_tx.send(StateUpdate::Error(format!("Connect failed: {}", e)));
                        return;
                    }
                }
            }
            InitialiseArguments::Attach(attach_args) => {
                match TcpAsyncDebugger::attach_staged(port, attach_args.language, attach_args).await
                {
                    Ok(d) => d,
                    Err(e) => {
                        let _ = update_tx.send(StateUpdate::Error(format!("Attach failed: {}", e)));
                        return;
                    }
                }
            }
        };

        // Configure initial breakpoints BEFORE starting execution
        tracing::debug!(
            count = initial_breakpoints.len(),
            "configuring initial breakpoints"
        );
        if !initial_breakpoints.is_empty() {
            if let Err(e) = debugger.configure_breakpoints(&initial_breakpoints).await {
                tracing::error!(error = %e, "failed to configure breakpoints");
                let _ = update_tx.send(StateUpdate::Error(format!(
                    "Failed to configure breakpoints: {}",
                    e
                )));
                // Continue anyway - breakpoints failing shouldn't prevent debugging
            }
        }
        tracing::debug!("breakpoints configured, starting debug session");

        // Start the debug session (sends ConfigurationDone)
        if let Err(e) = debugger.start().await {
            tracing::error!(error = %e, "failed to start debug session");
            let _ = update_tx.send(StateUpdate::Error(format!("Start failed: {}", e)));
            return;
        }
        tracing::debug!("debug session started");

        loop {
            tokio::select! {
                // Handle commands from UI
                Some(cmd) = command_rx.recv() => {
                    let result: eyre::Result<()> = match cmd {
                        UiCommand::Continue => debugger.continue_().await,
                        UiCommand::StepOver => debugger.step_over().await,
                        UiCommand::StepIn => debugger.step_in().await,
                        UiCommand::StepOut => debugger.step_out().await,
                        UiCommand::Terminate => debugger.terminate().await,
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

    /// Send a command to the async runtime (non-blocking).
    ///
    /// Commands are queued and processed by the background task.
    /// This method never blocks.
    pub fn send_command(&self, cmd: UiCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Poll for state updates (non-blocking).
    ///
    /// Returns all available updates without blocking.
    /// Call this in your event loop to receive debugger events.
    pub fn poll_updates(&mut self) -> Vec<StateUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    /// Check if the command channel is still connected.
    ///
    /// Returns `false` if the async task has terminated.
    pub fn is_connected(&self) -> bool {
        !self.command_tx.is_closed()
    }
}
