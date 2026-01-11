use eyre::WrapErr;
use futures::StreamExt;
use std::sync::Arc;
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

pub struct AsyncDebugger {
    internals: Arc<AsyncDebuggerInternals>,
    event_rx: AsyncEventReceiver,
    cancel_token: CancellationToken,

    // Task handles for cleanup
    reader_handle: Option<JoinHandle<()>>,
    processor_handle: Option<JoinHandle<()>>,
}

impl AsyncDebugger {
    /// Connect to a debug adapter and initialize the session
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

    /// Attach to a running debug session
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

    /// Create from an existing transport (useful for testing)
    pub async fn from_transport(
        reader: DapReader<tokio::net::tcp::OwnedReadHalf>,
        writer: DapWriter<tokio::net::tcp::OwnedWriteHalf>,
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
        };

        // Initialize DAP session
        debugger
            .initialize(language, launch_args, attach_args, stop_on_entry)
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

        // Send Launch or Attach request
        if let Some(launch_args) = launch_args {
            let response = self
                .internals
                .send_and_wait(launch_args.to_request())
                .await?;

            if !response.success {
                eyre::bail!(
                    "launch request failed: {}",
                    response.message.unwrap_or_default()
                );
            }
        } else if let Some(attach_args) = attach_args {
            let response = self
                .internals
                .send_and_wait(attach_args.to_request())
                .await?;

            if !response.success {
                eyre::bail!(
                    "attach request failed: {}",
                    response.message.unwrap_or_default()
                );
            }
        } else {
            eyre::bail!("either launch or attach arguments must be provided");
        }

        // Wait for initialized event
        // Note: In a real implementation, we'd wait for the initialized event here
        // For now, we'll just proceed

        // Set exception breakpoints (if needed)
        let _ = self
            .internals
            .send_and_wait(requests::RequestBody::SetExceptionBreakpoints(
                requests::SetExceptionBreakpoints { filters: vec![] },
            ))
            .await;

        Ok(())
    }

    /// Spawn the reader task that reads messages from the debug adapter
    fn spawn_reader_task(
        mut reader: DapReader<tokio::net::tcp::OwnedReadHalf>,
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
        internals: Arc<AsyncDebuggerInternals>,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::debug!("processor task cancelled");
                        break;
                    }
                    msg = message_rx.recv() => {
                        match msg {
                            Some(Message::Response(response)) => {
                                internals.handle_response(response).await;
                            }
                            Some(Message::Event(event)) => {
                                if let Err(e) = internals.handle_event(&event).await {
                                    tracing::error!(error = %e, "error handling event");
                                }
                            }
                            Some(Message::Request(_)) => {
                                tracing::warn!("received reverse request from adapter (not implemented)");
                            }
                            None => {
                                tracing::debug!("message channel closed");
                                break;
                            }
                        }
                    }
                }
            }
        })
    }

    /// Get event receiver for subscribing to debugger events
    pub fn events(&mut self) -> &mut AsyncEventReceiver {
        &mut self.event_rx
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
            .lock()
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
            .lock()
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
            .lock()
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
            .lock()
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
        let breakpoints = self.internals.breakpoints.lock().await;
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

impl Drop for AsyncDebugger {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}
