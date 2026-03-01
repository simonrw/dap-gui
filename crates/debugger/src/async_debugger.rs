use eyre::WrapErr;
use futures::StreamExt;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use transport::{
    requests::{self, PathFormat},
    types::{StackFrameId, Variable},
};
use transport2::{DapReader, DapWriter, Message};

use crate::{
    async_event::AsyncEventReceiver,
    async_internals::AsyncDebuggerInternals,
    state::{AttachArguments, Language, LaunchArguments},
    types::{Breakpoint, BreakpointId, EvaluateResult},
};

/// An async debugger client using the transport2 layer.
///
/// The type parameters `R` and `W` represent the underlying async reader
/// and writer types, allowing this to work with both TCP connections and
/// in-memory transports for testing.
///
/// For TCP connections, use the convenience methods [`connect`](Self::connect)
/// and [`attach`](Self::attach). For testing, use [`from_transport`](Self::from_transport).
pub struct AsyncDebugger<R, W> {
    internals: Arc<AsyncDebuggerInternals<W>>,
    event_rx: AsyncEventReceiver,
    cancel_token: CancellationToken,

    // Task handles for cleanup
    reader_handle: Option<JoinHandle<()>>,
    processor_handle: Option<JoinHandle<()>>,

    // Phantom data for the reader type (used in task but not stored)
    _reader: std::marker::PhantomData<R>,
}

/// Type alias for TCP-based AsyncDebugger (the most common use case).
pub type TcpAsyncDebugger =
    AsyncDebugger<tokio::net::tcp::OwnedReadHalf, tokio::net::tcp::OwnedWriteHalf>;

impl TcpAsyncDebugger {
    /// Connect to a debug adapter and initialize the session.
    ///
    /// This is a convenience method for TCP connections. The debugger is fully
    /// initialized and ready to start after this call returns.
    ///
    /// If you need to configure breakpoints before starting, use [`connect_staged`]
    /// instead, which allows you to call [`configure_breakpoints`] before [`start`].
    pub async fn connect(
        port: u16,
        language: Language,
        launch_args: &LaunchArguments,
        stop_on_entry: bool,
    ) -> eyre::Result<Self> {
        // Connect using transport2
        let (reader, writer) = transport2::connect(format!("127.0.0.1:{}", port))
            .await
            .wrap_err("connecting to debug adapter")?;

        Self::from_transport(
            reader,
            writer,
            language,
            Some(launch_args.clone()),
            None,
            stop_on_entry,
        )
        .await
    }

    /// Connect to a debug adapter with staged initialization.
    ///
    /// This method connects and initializes the DAP session (sends Initialize,
    /// Launch requests, and waits for the Initialized event), but does NOT send
    /// ConfigurationDone. This allows you to configure breakpoints before the
    /// debuggee starts executing.
    ///
    /// # Usage
    ///
    /// ```ignore
    /// let debugger = TcpAsyncDebugger::connect_staged(port, language, &launch_args, true).await?;
    ///
    /// // Configure breakpoints BEFORE execution starts
    /// debugger.configure_breakpoints(&breakpoints).await?;
    ///
    /// // Now start execution (sends ConfigurationDone)
    /// debugger.start().await?;
    /// ```
    pub async fn connect_staged(
        port: u16,
        language: Language,
        launch_args: &LaunchArguments,
        stop_on_entry: bool,
    ) -> eyre::Result<Self> {
        // Connect using transport2
        let (reader, writer) = transport2::connect(format!("127.0.0.1:{}", port))
            .await
            .wrap_err("connecting to debug adapter")?;

        Self::from_transport_staged(
            reader,
            writer,
            language,
            Some(launch_args.clone()),
            None,
            stop_on_entry,
        )
        .await
    }

    /// Attach to a running debug session.
    ///
    /// This is a convenience method for TCP connections.
    pub async fn attach(
        port: u16,
        language: Language,
        attach_args: &AttachArguments,
    ) -> eyre::Result<Self> {
        // Connect using transport2
        let (reader, writer) = transport2::connect(format!("127.0.0.1:{}", port))
            .await
            .wrap_err("connecting to debug adapter")?;

        Self::from_transport(
            reader,
            writer,
            language,
            None,
            Some(attach_args.clone()),
            false,
        )
        .await
    }

    /// Attach to a running debug session with staged initialization.
    ///
    /// Like [`attach`], but doesn't send ConfigurationDone. Use this when you
    /// need to configure breakpoints before the session starts.
    pub async fn attach_staged(
        port: u16,
        language: Language,
        attach_args: &AttachArguments,
    ) -> eyre::Result<Self> {
        // Connect using transport2
        let (reader, writer) = transport2::connect(format!("127.0.0.1:{}", port))
            .await
            .wrap_err("connecting to debug adapter")?;

        Self::from_transport_staged(
            reader,
            writer,
            language,
            None,
            Some(attach_args.clone()),
            false,
        )
        .await
    }
}

impl<R, W> AsyncDebugger<R, W>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    /// Create from an existing transport.
    ///
    /// This is the primary constructor, useful for testing with in-memory
    /// transports or for custom connection scenarios.
    pub async fn from_transport(
        reader: DapReader<R>,
        writer: DapWriter<W>,
        language: Language,
        launch_args: Option<LaunchArguments>,
        attach_args: Option<AttachArguments>,
        stop_on_entry: bool,
    ) -> eyre::Result<Self> {
        // Create channels
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Create internals
        let internals = Arc::new(AsyncDebuggerInternals::new(writer, event_tx.clone()));

        // Spawn reader task
        let reader_handle = Self::spawn_reader_task(reader, message_tx, cancel_token.clone());

        // Spawn event processor task
        let processor_handle =
            Self::spawn_processor_task(message_rx, internals.clone(), cancel_token.clone());

        let debugger = Self {
            internals,
            event_rx: AsyncEventReceiver::new(event_rx),
            cancel_token,
            reader_handle: Some(reader_handle),
            processor_handle: Some(processor_handle),
            _reader: std::marker::PhantomData,
        };

        // Initialize DAP session
        debugger
            .initialize(language, launch_args, attach_args, stop_on_entry)
            .await?;

        Ok(debugger)
    }

    /// Create from an existing transport with staged initialization.
    ///
    /// Like [`from_transport`], but doesn't complete the initialization sequence.
    /// The DAP session is initialized (Initialize, Launch/Attach, Initialized event),
    /// but ConfigurationDone is NOT sent. Call [`configure_breakpoints`] then [`start`]
    /// to complete the initialization.
    pub async fn from_transport_staged(
        reader: DapReader<R>,
        writer: DapWriter<W>,
        language: Language,
        launch_args: Option<LaunchArguments>,
        attach_args: Option<AttachArguments>,
        stop_on_entry: bool,
    ) -> eyre::Result<Self> {
        // Create channels
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Create internals
        let internals = Arc::new(AsyncDebuggerInternals::new(writer, event_tx.clone()));

        // Spawn reader task
        let reader_handle = Self::spawn_reader_task(reader, message_tx, cancel_token.clone());

        // Spawn event processor task
        let processor_handle =
            Self::spawn_processor_task(message_rx, internals.clone(), cancel_token.clone());

        let debugger = Self {
            internals,
            event_rx: AsyncEventReceiver::new(event_rx),
            cancel_token,
            reader_handle: Some(reader_handle),
            processor_handle: Some(processor_handle),
            _reader: std::marker::PhantomData,
        };

        // Initialize DAP session (but don't send ConfigurationDone)
        debugger
            .initialize_staged(language, launch_args, attach_args, stop_on_entry)
            .await?;

        Ok(debugger)
    }

    /// Initialize the debug session
    async fn initialize(
        &self,
        language: Language,
        launch_args: Option<LaunchArguments>,
        attach_args: Option<AttachArguments>,
        _stop_on_entry: bool,
    ) -> eyre::Result<()> {
        // Send Initialize request
        let adapter_id = match language {
            Language::DebugPy => "debugpy",
            Language::Delve => "delve",
        };

        let response = self
            .internals
            .send_and_wait(requests::RequestBody::Initialize(requests::Initialize {
                adapter_id: adapter_id.to_string(),
                path_format: PathFormat::Path,
                lines_start_at_one: true,
                supports_start_debugging_request: false,
                supports_variable_type: false,
                supports_variable_paging: false,
                supports_progress_reporting: false,
                supports_memory_event: false,
            }))
            .await?;

        if !response.success {
            eyre::bail!(
                "initialize request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        // Set up the channel to receive the initialized event BEFORE sending
        // Launch/Attach. This prevents a race condition where the initialized
        // event arrives before we start waiting for it.
        let initialized_rx = self.setup_initialized_event_channel().await;

        // Send Launch or Attach request (fire-and-forget, don't wait for response)
        if let Some(launch_args) = launch_args {
            self.internals
                .send_request(launch_args.to_request()?)
                .await?;
        } else if let Some(attach_args) = attach_args {
            self.internals
                .send_request(attach_args.to_request())
                .await?;
        } else {
            eyre::bail!("either launch or attach arguments must be provided");
        }

        // Wait for initialized event
        self.wait_for_initialized_event(initialized_rx).await?;

        // Set exception breakpoints (if needed)
        let _ = self
            .internals
            .send_and_wait(requests::RequestBody::SetExceptionBreakpoints(
                requests::SetExceptionBreakpoints { filters: vec![] },
            ))
            .await;

        Ok(())
    }

    /// Initialize the debug session but don't send ConfigurationDone.
    ///
    /// This is used by staged initialization to allow breakpoint configuration
    /// before the debuggee starts executing.
    async fn initialize_staged(
        &self,
        language: Language,
        launch_args: Option<LaunchArguments>,
        attach_args: Option<AttachArguments>,
        _stop_on_entry: bool,
    ) -> eyre::Result<()> {
        // Send Initialize request
        let adapter_id = match language {
            Language::DebugPy => "debugpy",
            Language::Delve => "delve",
        };

        let response = self
            .internals
            .send_and_wait(requests::RequestBody::Initialize(requests::Initialize {
                adapter_id: adapter_id.to_string(),
                path_format: PathFormat::Path,
                lines_start_at_one: true,
                supports_start_debugging_request: false,
                supports_variable_type: false,
                supports_variable_paging: false,
                supports_progress_reporting: false,
                supports_memory_event: false,
            }))
            .await?;

        if !response.success {
            eyre::bail!(
                "initialize request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        // Set up the channel to receive the initialized event BEFORE sending
        // Launch/Attach. This prevents a race condition where the initialized
        // event arrives before we start waiting for it.
        let initialized_rx = self.setup_initialized_event_channel().await;

        // Send Launch or Attach request (fire-and-forget, don't wait for response)
        // The response will arrive eventually, but we don't need to wait for it.
        // This matches the behavior of the sync debugger and debugpy's expectations.
        if let Some(launch_args) = launch_args {
            tracing::debug!("sending launch request");
            self.internals
                .send_request(launch_args.to_request()?)
                .await?;
            tracing::debug!("launch request sent");
        } else if let Some(attach_args) = attach_args {
            tracing::debug!("sending attach request");
            self.internals
                .send_request(attach_args.to_request())
                .await?;
            tracing::debug!("attach request sent");
        } else {
            eyre::bail!("either launch or attach arguments must be provided");
        }

        // Wait for initialized event
        self.wait_for_initialized_event(initialized_rx).await?;
        tracing::debug!("initialization complete, setting exception breakpoints");

        // Set exception breakpoints (if needed)
        let _ = self
            .internals
            .send_and_wait(requests::RequestBody::SetExceptionBreakpoints(
                requests::SetExceptionBreakpoints { filters: vec![] },
            ))
            .await;

        tracing::debug!("initialize_staged complete");
        // NOTE: We intentionally do NOT send ConfigurationDone here.
        // The caller should call configure_breakpoints() then start().

        Ok(())
    }

    /// Configure initial breakpoints before starting the debug session.
    ///
    /// This method should be called after [`connect_staged`] or [`from_transport_staged`]
    /// but before [`start`]. It allows you to set breakpoints that will be hit
    /// from the very beginning of program execution.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let debugger = TcpAsyncDebugger::connect_staged(port, language, &launch_args, true).await?;
    ///
    /// // Configure breakpoints before execution starts
    /// let breakpoints = vec![
    ///     Breakpoint { path: "main.py".into(), line: 10, name: None },
    ///     Breakpoint { path: "main.py".into(), line: 20, name: None },
    /// ];
    /// debugger.configure_breakpoints(&breakpoints).await?;
    ///
    /// // Now start execution
    /// debugger.start().await?;
    /// ```
    pub async fn configure_breakpoints(&self, breakpoints: &[Breakpoint]) -> eyre::Result<()> {
        for bp in breakpoints {
            self.add_breakpoint(bp).await?;
        }
        Ok(())
    }

    /// Wait for the initialized event from the debug adapter
    /// Set up a channel to receive the initialized event.
    ///
    /// Returns a receiver that will be signaled when the initialized event arrives.
    /// This must be called BEFORE sending the Launch/Attach request to avoid a race
    /// condition where the initialized event arrives before we start waiting for it.
    async fn setup_initialized_event_channel(&self) -> tokio::sync::oneshot::Receiver<()> {
        tracing::debug!("setting up initialized event channel");
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.internals.set_initialized_channel(tx).await;
        tracing::debug!("initialized event channel set up");
        rx
    }

    /// Wait for the initialized event using a pre-created receiver.
    async fn wait_for_initialized_event(
        &self,
        rx: tokio::sync::oneshot::Receiver<()>,
    ) -> eyre::Result<()> {
        tracing::debug!("waiting for initialized event");
        tokio::time::timeout(std::time::Duration::from_secs(10), rx)
            .await
            .wrap_err("timeout waiting for initialized event")?
            .wrap_err("initialized event channel closed")?;

        tracing::debug!("initialized event received");
        Ok(())
    }

    /// Spawn the reader task that reads messages from the debug adapter
    fn spawn_reader_task(
        mut reader: DapReader<R>,
        message_tx: mpsc::UnboundedSender<Message>,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::debug!("reader task cancelled");
                        break;
                    }
                    msg = reader.next() => {
                        match msg {
                            Some(Ok(message)) => {
                                tracing::debug!(?message, "received message");
                                if message_tx.send(message).is_err() {
                                    tracing::debug!("message channel closed");
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                tracing::error!(error = %e, "transport error");
                                break;
                            }
                            None => {
                                tracing::debug!("transport closed");
                                break;
                            }
                        }
                    }
                }
            }
        })
    }

    /// Spawn the processor task that handles incoming messages
    fn spawn_processor_task(
        mut message_rx: mpsc::UnboundedReceiver<Message>,
        internals: Arc<AsyncDebuggerInternals<W>>,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            tracing::debug!("processor task started");
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::debug!("processor task cancelled");
                        break;
                    }
                    msg = message_rx.recv() => {
                        tracing::debug!("processor task received message");
                        match msg {
                            Some(Message::Response(response)) => {
                                internals.handle_response(response).await;
                            }
                            Some(Message::Event(event)) => {
                                if let Err(e) = AsyncDebuggerInternals::handle_event(Arc::clone(&internals), event).await {
                                    tracing::error!(error = %e, "error handling event");
                                }
                            }
                            Some(Message::Request(_)) => {
                                tracing::warn!("received reverse request from adapter (not implemented)");
                            }
                            None => {
                                tracing::debug!("processor task: message channel closed");
                                break;
                            }
                        }
                    }
                }
            }
            tracing::debug!("processor task ended");
        })
    }

    /// Get event receiver for subscribing to debugger events
    pub fn events(&mut self) -> &mut AsyncEventReceiver {
        &mut self.event_rx
    }

    /// Take ownership of the event receiver, replacing it with an empty one.
    /// This is useful when the event receiver needs to be moved to another task.
    pub fn take_events(&mut self) -> AsyncEventReceiver {
        std::mem::replace(&mut self.event_rx, AsyncEventReceiver::empty())
    }

    /// Start the debugging session (send ConfigurationDone)
    pub async fn start(&self) -> eyre::Result<()> {
        let response = self
            .internals
            .send_and_wait(requests::RequestBody::ConfigurationDone)
            .await?;

        if !response.success {
            eyre::bail!(
                "configurationDone request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Continue execution
    pub async fn continue_(&self) -> eyre::Result<()> {
        let thread_id = self
            .internals
            .current_thread_id
            .read()
            .await
            .ok_or_else(|| eyre::eyre!("no current thread"))?;

        let response = self
            .internals
            .send_and_wait(requests::RequestBody::Continue(requests::Continue {
                thread_id,
                single_thread: false,
            }))
            .await?;

        if !response.success {
            eyre::bail!(
                "continue request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Step over (next line)
    pub async fn step_over(&self) -> eyre::Result<()> {
        let thread_id = self
            .internals
            .current_thread_id
            .read()
            .await
            .ok_or_else(|| eyre::eyre!("no current thread"))?;

        let response = self
            .internals
            .send_and_wait(requests::RequestBody::Next(requests::Next { thread_id }))
            .await?;

        if !response.success {
            eyre::bail!(
                "next request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Step into function
    pub async fn step_in(&self) -> eyre::Result<()> {
        let thread_id = self
            .internals
            .current_thread_id
            .read()
            .await
            .ok_or_else(|| eyre::eyre!("no current thread"))?;

        let response = self
            .internals
            .send_and_wait(requests::RequestBody::StepIn(requests::StepIn {
                thread_id,
            }))
            .await?;

        if !response.success {
            eyre::bail!(
                "stepIn request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Step out of function
    pub async fn step_out(&self) -> eyre::Result<()> {
        let thread_id = self
            .internals
            .current_thread_id
            .read()
            .await
            .ok_or_else(|| eyre::eyre!("no current thread"))?;

        let response = self
            .internals
            .send_and_wait(requests::RequestBody::StepOut(requests::StepOut {
                thread_id,
            }))
            .await?;

        if !response.success {
            eyre::bail!(
                "stepOut request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Evaluate expression in current frame
    pub async fn evaluate(
        &self,
        expression: &str,
        frame_id: StackFrameId,
    ) -> eyre::Result<EvaluateResult> {
        self.internals.evaluate_async(expression, frame_id).await
    }

    /// Add a breakpoint
    pub async fn add_breakpoint(&self, breakpoint: &Breakpoint) -> eyre::Result<BreakpointId> {
        self.internals.add_breakpoint_async(breakpoint).await
    }

    /// Remove a breakpoint
    pub async fn remove_breakpoint(&self, id: BreakpointId) -> eyre::Result<()> {
        self.internals.remove_breakpoint_async(id).await
    }

    /// Get current breakpoints
    pub async fn breakpoints(&self) -> Vec<Breakpoint> {
        let breakpoints = self.internals.breakpoints.read().await;
        breakpoints.values().cloned().collect()
    }

    /// Get variables for a scope
    pub async fn variables(&self, variables_reference: i64) -> eyre::Result<Vec<Variable>> {
        self.internals.variables_async(variables_reference).await
    }

    /// Change the current scope to a different stack frame
    pub async fn change_scope(&self, frame_id: StackFrameId) -> eyre::Result<()> {
        self.internals.change_scope_async(frame_id).await
    }

    /// Add a function breakpoint
    pub async fn add_function_breakpoint(&self, function_name: String) -> eyre::Result<()> {
        self.internals
            .add_function_breakpoint_async(function_name)
            .await
    }

    /// Remove a function breakpoint
    pub async fn remove_function_breakpoint(&self, function_name: &str) -> eyre::Result<()> {
        self.internals
            .remove_function_breakpoint_async(function_name)
            .await
    }

    /// Get current function breakpoints
    pub async fn function_breakpoints(&self) -> Vec<String> {
        self.internals.function_breakpoints_async().await
    }

    /// Terminate the debugee process
    pub async fn terminate(&self) -> eyre::Result<()> {
        let response = self
            .internals
            .send_and_wait(requests::RequestBody::Terminate(requests::Terminate {
                restart: Some(false),
            }))
            .await?;

        if !response.success {
            eyre::bail!(
                "terminate request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Shutdown the debugger
    pub async fn shutdown(mut self) -> eyre::Result<()> {
        // Send disconnect request
        let _ = self
            .internals
            .send_and_wait(requests::RequestBody::Disconnect(requests::Disconnect {
                terminate_debugee: true,
            }))
            .await;

        // Cancel tasks
        self.cancel_token.cancel();

        // Wait for tasks to complete
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.processor_handle.take() {
            let _ = handle.await;
        }

        Ok(())
    }
}

impl<R, W> Drop for AsyncDebugger<R, W> {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}
