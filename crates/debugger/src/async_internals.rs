use eyre::WrapErr;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};
use tokio::io::AsyncWrite;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use transport::{
    requests::{self},
    responses::{self},
    types::{Source, SourceBreakpoint, StackFrame, StackFrameId, ThreadId, Variable},
};
use transport2::{DapWriter, OutgoingMessage, Response};

type Seq = i64;

use crate::{
    state::{Event, ProgramState},
    types::{Breakpoint, BreakpointId, EvaluateResult, PausedFrame},
};

/// Internal state for the async debugger.
///
/// The type parameter `W` represents the underlying async writer type,
/// allowing this to work with both TCP connections and in-memory transports
/// for testing.
pub struct AsyncDebuggerInternals<W> {
    writer: Mutex<DapWriter<W>>,
    sequence_number: AtomicI64,
    event_tx: mpsc::UnboundedSender<Event>,
    pending_requests: Mutex<HashMap<Seq, oneshot::Sender<Response>>>,
    initialized_tx: Mutex<Option<oneshot::Sender<()>>>,

    // Debugger-specific state
    pub(crate) current_thread_id: RwLock<Option<ThreadId>>,
    pub(crate) breakpoints: RwLock<HashMap<BreakpointId, Breakpoint>>,
    current_breakpoint_id: AtomicI64,
    pub(crate) function_breakpoints: Mutex<Vec<String>>,
}

impl<W> AsyncDebuggerInternals<W>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub(crate) fn new(writer: DapWriter<W>, event_tx: mpsc::UnboundedSender<Event>) -> Self {
        Self {
            writer: Mutex::new(writer),
            sequence_number: AtomicI64::new(0),
            event_tx,
            pending_requests: Mutex::new(HashMap::new()),
            initialized_tx: Mutex::new(None),
            current_thread_id: RwLock::new(None),
            breakpoints: RwLock::new(HashMap::new()),
            current_breakpoint_id: AtomicI64::new(0),
            function_breakpoints: Mutex::new(Vec::new()),
        }
    }

    /// Set the channel for receiving the initialized event
    pub(crate) async fn set_initialized_channel(&self, tx: oneshot::Sender<()>) {
        let mut initialized_tx = self.initialized_tx.lock().await;
        *initialized_tx = Some(tx);
    }

    /// Send a request and return the sequence number
    pub(crate) async fn send_request(&self, body: requests::RequestBody) -> eyre::Result<Seq> {
        let seq = self.sequence_number.fetch_add(1, Ordering::SeqCst);

        // Serialize the request body to JSON
        let arguments = serde_json::to_value(&body).wrap_err("encoding request body")?;

        // Extract command name from the request body
        let command = match &body {
            requests::RequestBody::StackTrace(_) => "stackTrace",
            requests::RequestBody::Threads => "threads",
            requests::RequestBody::ConfigurationDone => "configurationDone",
            requests::RequestBody::Initialize(_) => "initialize",
            requests::RequestBody::Continue(_) => "continue",
            requests::RequestBody::SetFunctionBreakpoints(_) => "setFunctionBreakpoints",
            requests::RequestBody::SetBreakpoints(_) => "setBreakpoints",
            requests::RequestBody::SetExceptionBreakpoints(_) => "setExceptionBreakpoints",
            requests::RequestBody::Attach(_) => "attach",
            requests::RequestBody::Launch(_) => "launch",
            requests::RequestBody::Scopes(_) => "scopes",
            requests::RequestBody::Variables(_) => "variables",
            requests::RequestBody::BreakpointLocations(_) => "breakpointLocations",
            requests::RequestBody::LoadedSources => "loadedSources",
            requests::RequestBody::Terminate(_) => "terminate",
            requests::RequestBody::Disconnect(_) => "disconnect",
            requests::RequestBody::Next(_) => "next",
            requests::RequestBody::StepIn(_) => "stepIn",
            requests::RequestBody::StepOut(_) => "stepOut",
            requests::RequestBody::Evaluate(_) => "evaluate",
        }
        .to_string();

        // Extract arguments - for commands without arguments, use null
        let arguments = match &body {
            requests::RequestBody::Threads
            | requests::RequestBody::ConfigurationDone
            | requests::RequestBody::LoadedSources => None,
            _ => Some(arguments.get("arguments").cloned().unwrap_or(arguments)),
        };

        let msg = OutgoingMessage::Request(transport2::Request {
            seq,
            command: command.clone(),
            arguments: arguments.clone(),
        });

        tracing::debug!(seq, command, ?arguments, "sending request");

        let mut writer = self.writer.lock().await;
        futures::SinkExt::send(&mut *writer, msg)
            .await
            .wrap_err("sending request")?;

        tracing::debug!(seq, "request sent");

        Ok(seq)
    }

    /// Send a request and wait for the response
    #[tracing::instrument(skip(self))]
    pub(crate) async fn send_and_wait(
        &self,
        body: requests::RequestBody,
    ) -> eyre::Result<Response> {
        tracing::debug!("sending request");
        let seq = self.send_request(body).await?;

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(seq, tx);
        }

        tracing::debug!(seq, "waiting for response");

        // Wait for response with timeout
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .wrap_err("timeout waiting for response")?
            .wrap_err("response channel closed")?;

        tracing::debug!(seq, success = response.success, "received response");

        Ok(response)
    }

    /// Handle a response message by forwarding it to the waiting request
    pub(crate) async fn handle_response(&self, response: Response) {
        tracing::debug!(
            request_seq = response.request_seq,
            success = response.success,
            command = %response.command,
            message = ?response.message,
            "handling response"
        );
        let mut pending = self.pending_requests.lock().await;
        if let Some(tx) = pending.remove(&response.request_seq) {
            tracing::debug!(
                request_seq = response.request_seq,
                "forwarding response to waiter"
            );
            let _ = tx.send(response);
        } else {
            tracing::warn!(
                request_seq = response.request_seq,
                "no waiter found for response"
            );
        }
    }

    /// Handle an event from the debug adapter.
    ///
    /// This method takes an `Arc<Self>` to allow spawning background tasks
    /// for heavy event handlers (like `stopped`) that need to make further
    /// DAP requests without blocking the processor task.
    pub(crate) async fn handle_event(
        self_arc: Arc<Self>,
        event: transport2::Event,
    ) -> eyre::Result<()> {
        tracing::debug!(?event, "handling event");

        match event.event.as_str() {
            "initialized" => {
                // Signal that initialization is complete
                let mut initialized_tx = self_arc.initialized_tx.lock().await;
                if let Some(tx) = initialized_tx.take() {
                    tracing::debug!("signaling initialized event");
                    let _ = tx.send(());
                } else {
                    tracing::warn!("initialized event received but no channel was set up");
                }
                let _ = self_arc.event_tx.send(Event::Initialised);
            }
            "stopped" => {
                let body: transport::events::StoppedEventBody =
                    serde_json::from_value(event.body.clone().unwrap_or_default())
                        .wrap_err("parsing stopped event")?;

                // Spawn the heavy work (fetching stack trace, scopes, variables)
                // in a separate task so the processor task can continue processing
                // incoming messages (including the responses we need).
                let internals = Arc::clone(&self_arc);
                tokio::spawn(async move {
                    if let Err(e) = internals.handle_stopped_event(body).await {
                        tracing::error!(error = %e, "error handling stopped event");
                    }
                });
            }
            "continued" => {
                let _ = self_arc.event_tx.send(Event::Running);
            }
            "terminated" => {
                let _ = self_arc.event_tx.send(Event::Ended);
            }
            "output" => {
                tracing::debug!("output event: {:?}", event.body);
            }
            "thread" => {
                tracing::debug!("thread event: {:?}", event.body);
            }
            _ => {
                tracing::debug!("unhandled event: {}", event.event);
            }
        }

        Ok(())
    }

    /// Handle a stopped event by fetching the full program state.
    ///
    /// This is extracted into a separate method so it can be spawned as a
    /// background task, avoiding deadlock in the processor task.
    async fn handle_stopped_event(
        &self,
        body: transport::events::StoppedEventBody,
    ) -> eyre::Result<()> {
        tracing::debug!("locking current thread id");
        {
            // scope to enforce lock drop
            *self.current_thread_id.write().await = Some(body.thread_id);
        }

        // Fetch full program state
        tracing::debug!(?body.thread_id, "fetching stack trace");
        let stack = self.fetch_stack_trace(body.thread_id).await?;
        tracing::debug!("building paused stack frame");
        let paused_frame = self.build_paused_frame(&stack).await?;
        tracing::debug!(?paused_frame, "locking breakpoints");
        let breakpoints = self.breakpoints.read().await;

        tracing::debug!("sending paused event");
        let _ = self.event_tx.send(Event::Paused(ProgramState {
            stack,
            breakpoints: breakpoints.values().cloned().collect(),
            paused_frame,
        }));

        Ok(())
    }

    /// Fetch stack trace for a thread
    #[tracing::instrument(skip(self))]
    async fn fetch_stack_trace(&self, thread_id: ThreadId) -> eyre::Result<Vec<StackFrame>> {
        tracing::debug!("sending request");
        let response = self
            .send_and_wait(requests::RequestBody::StackTrace(requests::StackTrace {
                thread_id,
                ..Default::default()
            }))
            .await?;

        tracing::debug!(?response, "got response");

        if !response.success {
            eyre::bail!(
                "stackTrace request failed: {}",
                response.message.unwrap_or_default()
            );
        }

        let body: responses::StackTraceResponse =
            serde_json::from_value(response.body.unwrap_or_default())
                .wrap_err("parsing stackTrace response")?;

        Ok(body.stack_frames)
    }

    /// Build the paused frame information
    async fn build_paused_frame(&self, stack: &[StackFrame]) -> eyre::Result<PausedFrame> {
        let frame = stack.first().ok_or_else(|| eyre::eyre!("no frames"))?;
        self.build_paused_frame_for_id(stack, frame.id).await
    }

    /// Build the paused frame information for a specific frame ID
    async fn build_paused_frame_for_id(
        &self,
        stack: &[StackFrame],
        frame_id: StackFrameId,
    ) -> eyre::Result<PausedFrame> {
        // Find the frame with the given ID
        let frame = stack
            .iter()
            .find(|f| f.id == frame_id)
            .ok_or_else(|| eyre::eyre!("frame not found"))?;

        tracing::debug!(?frame.id, "fetching scopes");

        // Fetch scopes for the frame
        let response = self
            .send_and_wait(requests::RequestBody::Scopes(requests::Scopes {
                frame_id: frame.id,
            }))
            .await?;

        eyre::ensure!(response.success, "bad response received: {response:?}");

        let scopes_body: responses::ScopesResponse =
            serde_json::from_value(response.body.unwrap_or_default())
                .wrap_err("parsing scopes response")?;

        // Fetch variables for the first scope (usually locals)
        let variables = if let Some(scope) = scopes_body.scopes.first() {
            self.fetch_variables(scope.variables_reference).await?
        } else {
            vec![]
        };

        Ok(PausedFrame {
            frame: frame.clone(),
            variables,
        })
    }

    /// Fetch variables for a scope
    async fn fetch_variables(&self, variables_reference: i64) -> eyre::Result<Vec<Variable>> {
        let response = self
            .send_and_wait(requests::RequestBody::Variables(requests::Variables {
                variables_reference,
            }))
            .await?;

        eyre::ensure!(response.success, "variables request failed");

        let body: responses::VariablesResponse =
            serde_json::from_value(response.body.unwrap_or_default())
                .wrap_err("parsing variables response")?;

        Ok(body.variables)
    }

    /// Evaluate an expression in the context of a stack frame
    pub(crate) async fn evaluate_async(
        &self,
        expression: &str,
        frame_id: StackFrameId,
    ) -> eyre::Result<EvaluateResult> {
        let response = self
            .send_and_wait(requests::RequestBody::Evaluate(requests::Evaluate {
                expression: expression.to_string(),
                frame_id: Some(frame_id),
                context: Some("repl".to_string()),
            }))
            .await?;

        if !response.success {
            return Ok(EvaluateResult {
                output: response.message.unwrap_or_else(|| "Error".to_string()),
                error: true,
            });
        }

        let body: responses::EvaluateResponse =
            serde_json::from_value(response.body.unwrap_or_default())
                .wrap_err("parsing evaluate response")?;

        Ok(EvaluateResult {
            output: body.result,
            error: false,
        })
    }

    /// Add a breakpoint
    pub(crate) async fn add_breakpoint_async(
        &self,
        breakpoint: &Breakpoint,
    ) -> eyre::Result<BreakpointId> {
        let id = self.current_breakpoint_id.fetch_add(1, Ordering::SeqCst) as u64;

        // Add to internal map and collect all breakpoints for this source file
        let source_breakpoints: Vec<SourceBreakpoint>;
        {
            let mut breakpoints = self.breakpoints.write().await;
            breakpoints.insert(id, breakpoint.clone());

            // Collect all breakpoints for this source file
            source_breakpoints = breakpoints
                .values()
                .filter(|bp| bp.path == breakpoint.path)
                .map(|bp| SourceBreakpoint {
                    line: bp.line,
                    ..Default::default()
                })
                .collect();
        }

        // Send all breakpoints for this file to debug adapter
        let response = self
            .send_and_wait(requests::RequestBody::SetBreakpoints(
                requests::SetBreakpoints {
                    source: Source {
                        name: breakpoint
                            .path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string()),
                        path: Some(breakpoint.path.clone()),
                        ..Default::default()
                    },
                    breakpoints: Some(source_breakpoints),
                    ..Default::default()
                },
            ))
            .await?;

        if !response.success {
            eyre::bail!(
                "setBreakpoints failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(id)
    }

    /// Remove a breakpoint
    pub(crate) async fn remove_breakpoint_async(&self, id: BreakpointId) -> eyre::Result<()> {
        // Remove from internal map and get the file path
        let (file_path, source_breakpoints): (std::path::PathBuf, Vec<SourceBreakpoint>);
        {
            let mut breakpoints = self.breakpoints.write().await;

            // Get the file path before removing
            let removed_bp = breakpoints
                .remove(&id)
                .ok_or_else(|| eyre::eyre!("breakpoint not found"))?;
            file_path = removed_bp.path.clone();

            // Collect remaining breakpoints for this source file
            source_breakpoints = breakpoints
                .values()
                .filter(|bp| bp.path == file_path)
                .map(|bp| SourceBreakpoint {
                    line: bp.line,
                    ..Default::default()
                })
                .collect();
        }

        // Send updated breakpoints list to debug adapter
        let response = self
            .send_and_wait(requests::RequestBody::SetBreakpoints(
                requests::SetBreakpoints {
                    source: Source {
                        name: file_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string()),
                        path: Some(file_path),
                        ..Default::default()
                    },
                    breakpoints: Some(source_breakpoints),
                    ..Default::default()
                },
            ))
            .await?;

        if !response.success {
            eyre::bail!(
                "setBreakpoints failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Get variables for a scope
    pub(crate) async fn variables_async(
        &self,
        variables_reference: i64,
    ) -> eyre::Result<Vec<Variable>> {
        self.fetch_variables(variables_reference).await
    }

    /// Change the current scope to a different stack frame
    pub(crate) async fn change_scope_async(&self, frame_id: StackFrameId) -> eyre::Result<()> {
        let thread_id = self
            .current_thread_id
            .read()
            .await
            .ok_or_else(|| eyre::eyre!("no current thread id"))?;

        let stack = self.fetch_stack_trace(thread_id).await?;
        let paused_frame = self.build_paused_frame_for_id(&stack, frame_id).await?;
        let breakpoints = self.breakpoints.read().await;

        let _ = self.event_tx.send(Event::ScopeChange(ProgramState {
            stack,
            breakpoints: breakpoints.values().cloned().collect(),
            paused_frame,
        }));

        Ok(())
    }

    /// Add a function breakpoint
    pub(crate) async fn add_function_breakpoint_async(
        &self,
        function_name: String,
    ) -> eyre::Result<()> {
        let mut function_breakpoints = self.function_breakpoints.lock().await;
        function_breakpoints.push(function_name.clone());

        // Send all function breakpoints to debug adapter
        let breakpoints: Vec<requests::Breakpoint> = function_breakpoints
            .iter()
            .map(|name| requests::Breakpoint { name: name.clone() })
            .collect();

        let response = self
            .send_and_wait(requests::RequestBody::SetFunctionBreakpoints(
                requests::SetFunctionBreakpoints { breakpoints },
            ))
            .await?;

        if !response.success {
            eyre::bail!(
                "setFunctionBreakpoints failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Remove a function breakpoint
    pub(crate) async fn remove_function_breakpoint_async(
        &self,
        function_name: &str,
    ) -> eyre::Result<()> {
        let mut function_breakpoints = self.function_breakpoints.lock().await;
        function_breakpoints.retain(|name| name != function_name);

        // Send updated function breakpoints to debug adapter
        let breakpoints: Vec<requests::Breakpoint> = function_breakpoints
            .iter()
            .map(|name| requests::Breakpoint { name: name.clone() })
            .collect();

        let response = self
            .send_and_wait(requests::RequestBody::SetFunctionBreakpoints(
                requests::SetFunctionBreakpoints { breakpoints },
            ))
            .await?;

        if !response.success {
            eyre::bail!(
                "setFunctionBreakpoints failed: {}",
                response.message.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Get all function breakpoints
    pub(crate) async fn function_breakpoints_async(&self) -> Vec<String> {
        let function_breakpoints = self.function_breakpoints.lock().await;
        function_breakpoints.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Create an AsyncDebuggerInternals connected to a MockAdapter for testing.
    /// Returns (internals, mock_adapter, event_rx).
    ///
    /// This also spawns a background task that reads from the client reader and
    /// forwards responses/events to the internals, mimicking the processor task
    /// in `AsyncDebugger`.
    fn setup() -> (
        Arc<AsyncDebuggerInternals<tokio::io::DuplexStream>>,
        MockAdapter,
        mpsc::UnboundedReceiver<Event>,
    ) {
        let (client_reader, client_writer, adapter_reader, adapter_writer) =
            create_test_transports();
        let mock = MockAdapter::new(adapter_reader, adapter_writer);

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let internals = Arc::new(AsyncDebuggerInternals::new(client_writer, event_tx));

        // Spawn a processor task that reads responses from the client reader
        // and delivers them to the internals, just like AsyncDebugger does.
        let internals_for_reader = Arc::clone(&internals);
        tokio::spawn(async move {
            use futures::StreamExt;
            let mut reader = client_reader;
            while let Some(Ok(msg)) = reader.next().await {
                match msg {
                    transport2::Message::Response(response) => {
                        internals_for_reader.handle_response(response).await;
                    }
                    transport2::Message::Event(event) => {
                        let _ = AsyncDebuggerInternals::handle_event(
                            Arc::clone(&internals_for_reader),
                            event,
                        )
                        .await;
                    }
                    _ => {}
                }
            }
        });

        (internals, mock, event_rx)
    }

    #[tokio::test]
    async fn handle_response_forwards_to_waiter() {
        let (internals, _, _event_rx) = setup();

        // Insert a pending request
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = internals.pending_requests.lock().await;
            pending.insert(42, tx);
        }

        let response = transport2::Response {
            seq: 1,
            request_seq: 42,
            success: true,
            command: "stackTrace".to_string(),
            message: None,
            body: None,
        };

        internals.handle_response(response).await;

        let received = rx.await.unwrap();
        assert_eq!(received.request_seq, 42);
        assert!(received.success);
    }

    #[tokio::test]
    async fn handle_response_orphaned_response() {
        let (internals, _, _event_rx) = setup();

        // No pending requests — should not panic
        let response = transport2::Response {
            seq: 1,
            request_seq: 999,
            success: true,
            command: "threads".to_string(),
            message: None,
            body: None,
        };

        internals.handle_response(response).await;

        // Verify pending map is still empty
        let pending = internals.pending_requests.lock().await;
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn handle_event_initialized() {
        let (internals, _, mut event_rx) = setup();

        let (tx, _rx) = tokio::sync::oneshot::channel();
        internals.set_initialized_channel(tx).await;

        let event = transport2::Event {
            seq: 1,
            event: "initialized".to_string(),
            body: None,
        };

        AsyncDebuggerInternals::handle_event(internals, event)
            .await
            .unwrap();

        let evt = event_rx.recv().await.unwrap();
        assert!(matches!(evt, Event::Initialised));
    }

    #[tokio::test]
    async fn handle_event_continued() {
        let (internals, _, mut event_rx) = setup();

        let event = transport2::Event {
            seq: 1,
            event: "continued".to_string(),
            body: Some(json!({"threadId": 1})),
        };

        AsyncDebuggerInternals::handle_event(internals, event)
            .await
            .unwrap();

        let evt = event_rx.recv().await.unwrap();
        assert!(matches!(evt, Event::Running));
    }

    #[tokio::test]
    async fn handle_event_terminated() {
        let (internals, _, mut event_rx) = setup();

        let event = transport2::Event {
            seq: 1,
            event: "terminated".to_string(),
            body: None,
        };

        AsyncDebuggerInternals::handle_event(internals, event)
            .await
            .unwrap();

        let evt = event_rx.recv().await.unwrap();
        assert!(matches!(evt, Event::Ended));
    }

    #[tokio::test]
    async fn handle_event_unknown_does_not_error() {
        let (internals, _, _event_rx) = setup();

        let event = transport2::Event {
            seq: 1,
            event: "customUnknownEvent".to_string(),
            body: None,
        };

        // Should not error on unknown events
        let result = AsyncDebuggerInternals::handle_event(internals, event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn evaluate_async_success() {
        let (internals, mock, _event_rx) = setup();

        let internals_clone = Arc::clone(&internals);
        let eval_handle =
            tokio::spawn(async move { internals_clone.evaluate_async("1 + 1", 1).await });

        // Expect the evaluate request
        let req = mock.expect_request("evaluate").await;
        mock.send_evaluate_response(req.seq, "2").await;

        let result = eval_handle.await.unwrap().unwrap();
        assert_eq!(result.output, "2");
        assert!(!result.error);
    }

    #[tokio::test]
    async fn evaluate_async_error_response() {
        let (internals, mock, _event_rx) = setup();

        let internals_clone = Arc::clone(&internals);
        let eval_handle =
            tokio::spawn(async move { internals_clone.evaluate_async("bad_var", 1).await });

        let req = mock.expect_request("evaluate").await;
        mock.send_error_response(req.seq, "NameError: name 'bad_var' is not defined")
            .await;

        let result = eval_handle.await.unwrap().unwrap();
        assert!(result.error);
        assert!(result.output.contains("NameError"));
    }

    #[tokio::test]
    async fn add_breakpoint_sends_set_breakpoints() {
        let (internals, mock, _event_rx) = setup();

        let bp = Breakpoint {
            name: None,
            path: PathBuf::from("/tmp/test.py"),
            line: 10,
        };

        let internals_clone = Arc::clone(&internals);
        let bp_clone = bp.clone();
        let add_handle =
            tokio::spawn(async move { internals_clone.add_breakpoint_async(&bp_clone).await });

        let req = mock.expect_request("setBreakpoints").await;
        mock.send_success_response(req.seq, Some(json!({"breakpoints": [{"verified": true}]})))
            .await;

        let id = add_handle.await.unwrap().unwrap();

        // Verify breakpoint was stored
        let breakpoints = internals.breakpoints.read().await;
        assert_eq!(breakpoints.len(), 1);
        assert!(breakpoints.contains_key(&id));
        assert_eq!(breakpoints[&id].line, 10);
    }

    #[tokio::test]
    async fn add_and_remove_breakpoint() {
        let (internals, mock, _event_rx) = setup();

        let bp = Breakpoint {
            name: None,
            path: PathBuf::from("/tmp/test.py"),
            line: 10,
        };

        // Add breakpoint
        let internals_clone = Arc::clone(&internals);
        let bp_clone = bp.clone();
        let add_handle =
            tokio::spawn(async move { internals_clone.add_breakpoint_async(&bp_clone).await });

        let req = mock.expect_request("setBreakpoints").await;
        mock.send_success_response(req.seq, Some(json!({"breakpoints": [{"verified": true}]})))
            .await;

        let id = add_handle.await.unwrap().unwrap();

        // Remove breakpoint
        let internals_clone = Arc::clone(&internals);
        let remove_handle =
            tokio::spawn(async move { internals_clone.remove_breakpoint_async(id).await });

        let req = mock.expect_request("setBreakpoints").await;
        mock.send_success_response(req.seq, Some(json!({"breakpoints": []})))
            .await;

        remove_handle.await.unwrap().unwrap();

        let breakpoints = internals.breakpoints.read().await;
        assert_eq!(breakpoints.len(), 0);
    }

    #[tokio::test]
    async fn remove_nonexistent_breakpoint_errors() {
        let (internals, _mock, _event_rx) = setup();

        let result = internals.remove_breakpoint_async(999).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("breakpoint not found")
        );
    }

    #[tokio::test]
    async fn add_function_breakpoint() {
        let (internals, mock, _event_rx) = setup();

        let internals_clone = Arc::clone(&internals);
        let handle = tokio::spawn(async move {
            internals_clone
                .add_function_breakpoint_async("main".to_string())
                .await
        });

        let req = mock.expect_request("setFunctionBreakpoints").await;
        mock.send_success_response(req.seq, Some(json!({"breakpoints": [{"verified": true}]})))
            .await;

        handle.await.unwrap().unwrap();

        let fbs = internals.function_breakpoints_async().await;
        assert_eq!(fbs, vec!["main"]);
    }

    #[tokio::test]
    async fn remove_function_breakpoint() {
        let (internals, mock, _event_rx) = setup();

        // Add first
        let internals_clone = Arc::clone(&internals);
        let handle = tokio::spawn(async move {
            internals_clone
                .add_function_breakpoint_async("main".to_string())
                .await
        });
        let req = mock.expect_request("setFunctionBreakpoints").await;
        mock.send_success_response(req.seq, Some(json!({"breakpoints": []})))
            .await;
        handle.await.unwrap().unwrap();

        // Remove
        let internals_clone = Arc::clone(&internals);
        let handle = tokio::spawn(async move {
            internals_clone
                .remove_function_breakpoint_async("main")
                .await
        });
        let req = mock.expect_request("setFunctionBreakpoints").await;
        mock.send_success_response(req.seq, Some(json!({"breakpoints": []})))
            .await;
        handle.await.unwrap().unwrap();

        let fbs = internals.function_breakpoints_async().await;
        assert!(fbs.is_empty());
    }

    #[tokio::test]
    async fn send_and_wait_receives_response() {
        let (internals, mock, _event_rx) = setup();

        let internals_clone = Arc::clone(&internals);
        let handle = tokio::spawn(async move {
            internals_clone
                .send_and_wait(transport::requests::RequestBody::Threads)
                .await
        });

        let req = mock.expect_request("threads").await;
        mock.send_success_response(
            req.seq,
            Some(json!({"threads": [{"id": 1, "name": "main"}]})),
        )
        .await;

        let response = handle.await.unwrap().unwrap();
        assert!(response.success);
    }

    #[tokio::test]
    async fn handle_stopped_event_fetches_state() {
        let (internals, mock, mut event_rx) = setup();

        let event = transport2::Event {
            seq: 1,
            event: "stopped".to_string(),
            body: Some(json!({"reason": "breakpoint", "threadId": 1})),
        };

        AsyncDebuggerInternals::handle_event(Arc::clone(&internals), event)
            .await
            .unwrap();

        // The stopped handler spawns a task that will request stackTrace, scopes, variables
        let req = mock.expect_request("stackTrace").await;
        mock.send_stack_trace_response(req.seq, vec![StackFrameData::default()])
            .await;

        let req = mock.expect_request("scopes").await;
        mock.send_scopes_response(req.seq, vec![ScopeData::default()])
            .await;

        let req = mock.expect_request("variables").await;
        mock.send_variables_response(req.seq, vec![VariableData::default()])
            .await;

        // Should receive a Paused event
        let evt = tokio::time::timeout(std::time::Duration::from_secs(5), event_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(evt, Event::Paused(_)));
    }

    #[tokio::test]
    async fn multiple_breakpoints_same_file() {
        let (internals, mock, _event_rx) = setup();

        let bp1 = Breakpoint {
            name: None,
            path: PathBuf::from("/tmp/test.py"),
            line: 10,
        };
        let bp2 = Breakpoint {
            name: None,
            path: PathBuf::from("/tmp/test.py"),
            line: 20,
        };

        // Add first breakpoint
        let internals_clone = Arc::clone(&internals);
        let bp1_clone = bp1.clone();
        let handle =
            tokio::spawn(async move { internals_clone.add_breakpoint_async(&bp1_clone).await });
        let req = mock.expect_request("setBreakpoints").await;
        mock.send_success_response(req.seq, Some(json!({"breakpoints": [{"verified": true}]})))
            .await;
        handle.await.unwrap().unwrap();

        // Add second breakpoint (same file — should send both)
        let internals_clone = Arc::clone(&internals);
        let bp2_clone = bp2.clone();
        let handle =
            tokio::spawn(async move { internals_clone.add_breakpoint_async(&bp2_clone).await });
        let req = mock.expect_request("setBreakpoints").await;

        // Verify the request includes both breakpoints
        if let Some(args) = &req.arguments {
            let breakpoints = args.get("breakpoints").unwrap().as_array().unwrap();
            assert_eq!(breakpoints.len(), 2);
        }

        mock.send_success_response(
            req.seq,
            Some(json!({"breakpoints": [{"verified": true}, {"verified": true}]})),
        )
        .await;
        handle.await.unwrap().unwrap();

        let breakpoints = internals.breakpoints.read().await;
        assert_eq!(breakpoints.len(), 2);
    }
}
