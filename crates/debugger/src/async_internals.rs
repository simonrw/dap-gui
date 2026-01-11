use eyre::WrapErr;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicI64, Ordering},
};
use tokio::sync::{Mutex, mpsc, oneshot};
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

pub(crate) struct AsyncDebuggerInternals {
    writer: Mutex<DapWriter<tokio::net::tcp::OwnedWriteHalf>>,
    sequence_number: AtomicI64,
    event_tx: mpsc::UnboundedSender<Event>,
    pending_requests: Mutex<HashMap<Seq, oneshot::Sender<Response>>>,
    initialized_tx: Mutex<Option<oneshot::Sender<()>>>,

    // Debugger-specific state
    pub(crate) current_thread_id: Mutex<Option<ThreadId>>,
    pub(crate) breakpoints: Mutex<HashMap<BreakpointId, Breakpoint>>,
    current_breakpoint_id: AtomicI64,
}

impl AsyncDebuggerInternals {
    pub(crate) fn new(
        writer: DapWriter<tokio::net::tcp::OwnedWriteHalf>,
        event_tx: mpsc::UnboundedSender<Event>,
    ) -> Self {
        Self {
            writer: Mutex::new(writer),
            sequence_number: AtomicI64::new(0),
            event_tx,
            pending_requests: Mutex::new(HashMap::new()),
            initialized_tx: Mutex::new(None),
            current_thread_id: Mutex::new(None),
            breakpoints: Mutex::new(HashMap::new()),
            current_breakpoint_id: AtomicI64::new(0),
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
            command,
            arguments,
        });

        let mut writer = self.writer.lock().await;
        futures::SinkExt::send(&mut *writer, msg)
            .await
            .wrap_err("sending request")?;

        Ok(seq)
    }

    /// Send a request and wait for the response
    pub(crate) async fn send_and_wait(
        &self,
        body: requests::RequestBody,
    ) -> eyre::Result<Response> {
        let seq = self.send_request(body).await?;

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(seq, tx);
        }

        // Wait for response with timeout
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .wrap_err("timeout waiting for response")?
            .wrap_err("response channel closed")?;

        Ok(response)
    }

    /// Handle a response message by forwarding it to the waiting request
    pub(crate) async fn handle_response(&self, response: Response) {
        let mut pending = self.pending_requests.lock().await;
        if let Some(tx) = pending.remove(&response.request_seq) {
            let _ = tx.send(response);
        }
    }

    /// Handle an event from the debug adapter
    pub(crate) async fn handle_event(&self, event: &transport2::Event) -> eyre::Result<()> {
        tracing::debug!(?event, "handling event");

        match event.event.as_str() {
            "initialized" => {
                // Signal that initialization is complete
                let mut initialized_tx = self.initialized_tx.lock().await;
                if let Some(tx) = initialized_tx.take() {
                    let _ = tx.send(());
                }
                let _ = self.event_tx.send(Event::Initialised);
            }
            "stopped" => {
                let body: transport::events::StoppedEventBody =
                    serde_json::from_value(event.body.clone().unwrap_or_default())
                        .wrap_err("parsing stopped event")?;

                *self.current_thread_id.lock().await = Some(body.thread_id);

                // Fetch full program state
                let stack = self.fetch_stack_trace(body.thread_id).await?;
                let paused_frame = self.build_paused_frame(&stack).await?;
                let breakpoints = self.breakpoints.lock().await;

                let _ = self.event_tx.send(Event::Paused(ProgramState {
                    stack,
                    breakpoints: breakpoints.values().cloned().collect(),
                    paused_frame,
                }));
            }
            "continued" => {
                let _ = self.event_tx.send(Event::Running);
            }
            "terminated" => {
                let _ = self.event_tx.send(Event::Ended);
            }
            "output" => {
                tracing::debug!("output event: {:?}", event.body);
            }
            _ => {
                tracing::debug!("unhandled event: {}", event.event);
            }
        }

        Ok(())
    }

    /// Fetch stack trace for a thread
    async fn fetch_stack_trace(&self, thread_id: ThreadId) -> eyre::Result<Vec<StackFrame>> {
        let response = self
            .send_and_wait(requests::RequestBody::StackTrace(requests::StackTrace {
                thread_id,
                ..Default::default()
            }))
            .await?;

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

        // Fetch scopes for the frame
        let response = self
            .send_and_wait(requests::RequestBody::Scopes(requests::Scopes {
                frame_id: frame.id,
            }))
            .await?;

        if !response.success {
            return Ok(PausedFrame {
                frame: frame.clone(),
                variables: vec![],
            });
        }

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

        if !response.success {
            return Ok(vec![]);
        }

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
            let mut breakpoints = self.breakpoints.lock().await;
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
            let mut breakpoints = self.breakpoints.lock().await;

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
            .lock()
            .await
            .ok_or_else(|| eyre::eyre!("no current thread id"))?;

        let stack = self.fetch_stack_trace(thread_id).await?;
        let paused_frame = self.build_paused_frame_for_id(&stack, frame_id).await?;
        let breakpoints = self.breakpoints.lock().await;

        let _ = self.event_tx.send(Event::ScopeChange(ProgramState {
            stack,
            breakpoints: breakpoints.values().cloned().collect(),
            paused_frame,
        }));

        Ok(())
    }
}
