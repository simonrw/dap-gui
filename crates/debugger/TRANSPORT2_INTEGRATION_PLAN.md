# Debugger + transport2 Integration Plan

This document outlines the plan for integrating the new async `transport2` crate with the `debugger` crate, and subsequently integrating the async debugger with the `gui-poc` prototype UI.

## Table of Contents

1. [Current Architecture](#current-architecture)
2. [Target Architecture](#target-architecture)
3. [Migration Plan](#migration-plan)
4. [GUI-POC Integration](#gui-poc-integration)
5. [Task Checklist](#task-checklist)

---

## Current Architecture

The debugger crate currently uses a **3-thread synchronous design**:

```
                          crossbeam channels
    +------------------+                      +--------------------+
    |  Polling Thread  |  -- events -->       |  Background Thread |
    |                  |  -- messages -->     |                    |
    |  reader.poll()   |                      |  Event processing  |
    |  (blocking I/O)  |                      |  Command handling  |
    +------------------+                      +--------------------+
                                                       |
                                              commands | events
                                                       v
                                              +------------------+
                                              |   Main Thread    |
                                              |   (UI/caller)    |
                                              +------------------+
```

### Key Components

| Component | Location | Description |
|-----------|----------|-------------|
| `Debugger` | `debugger.rs:113` | Public API, holds channels |
| `DebuggerInternals` | `internals.rs:87` | State, writer, event processing |
| `TransportConnection` | `transport/client.rs` | TCP connection, reader/writer split |
| Polling thread | `debugger.rs:168` | Owns reader, calls `poll_message()` |
| Background thread | `debugger.rs:282` | Processes events and commands |

### Current Synchronization Primitives

- `Arc<Mutex<DebuggerInternals>>` - Shared state
- `Arc<Mutex<Box<dyn Write>>>` - Writer protection
- `Arc<AtomicI64>` - Lock-free sequence numbers
- `crossbeam_channel` - Event/message/command distribution
- `oneshot` - Request-response pattern

---

## Target Architecture

The new architecture uses **async/await with tokio tasks**:

```
                           tokio mpsc channels
    +------------------+                      +--------------------+
    |   Reader Task    |  -- events -->       |  Event Processor   |
    |                  |  -- messages -->     |  Task              |
    |  reader.next()   |                      |                    |
    |  (async Stream)  |                      |  async processing  |
    +------------------+                      +--------------------+
            ^                                          |
            |                                 commands | events
      transport2::split()                              v
            |                                 +------------------+
    +------------------+                      |   Caller Task    |
    |   Writer Task    | <-- requests ------- |   (async API)    |
    |  writer.send()   |                      +------------------+
    +------------------+
```

### Benefits

- **No thread spawning** - Uses tokio tasks instead of OS threads
- **Better resource utilization** - Cooperative scheduling
- **Natural fit with transport2** - Stream/Sink patterns
- **Composable** - Easy to integrate with async UI frameworks
- **Cancellation support** - Via `CancellationToken`

---

## Migration Plan

### Phase 1: Infrastructure Setup

Add async dependencies and create compatibility layer.

#### 1.1 Update Cargo.toml

```toml
[dependencies]
transport2 = { path = "../transport2" }
tokio = { version = "1.48", features = ["sync", "rt", "macros"] }
tokio-util = { version = "0.7", features = ["sync"] }  # For CancellationToken
futures = "0.3.31"

# Keep for transition period
crossbeam-channel = "0.5"
```

#### 1.2 Create Async Event Types

File: `crates/debugger/src/async_event.rs`

```rust
use tokio::sync::mpsc;
use crate::state::Event;

/// Async event receiver that wraps tokio mpsc
pub struct AsyncEventReceiver {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl AsyncEventReceiver {
    /// Receive next event asynchronously
    pub async fn recv(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    /// Convert to a Stream for use with StreamExt
    pub fn into_stream(self) -> impl futures::Stream<Item = Event> {
        tokio_stream::wrappers::UnboundedReceiverStream::new(self.rx)
    }
}
```

#### 1.3 Create Async Internals Structure

File: `crates/debugger/src/async_internals.rs`

```rust
use transport2::{DapWriter, OutgoingMessage, Message};
use tokio::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};

pub struct AsyncDebuggerInternals {
    writer: Mutex<DapWriter<tokio::net::tcp::OwnedWriteHalf>>,
    sequence_number: AtomicI64,
    event_tx: tokio::sync::mpsc::UnboundedSender<Event>,

    // State fields (same as current)
    current_thread_id: Option<ThreadId>,
    breakpoints: HashMap<BreakpointId, Breakpoint>,
    current_source: Option<FileSource>,
}

impl AsyncDebuggerInternals {
    pub async fn send_request(&self, body: RequestBody) -> eyre::Result<Seq> {
        let seq = self.sequence_number.fetch_add(1, Ordering::SeqCst);
        let msg = OutgoingMessage::Request(Request { seq, body });

        let mut writer = self.writer.lock().await;
        writer.send(msg).await?;

        Ok(seq)
    }
}
```

### Phase 2: Create AsyncDebugger

#### 2.1 Core Structure

File: `crates/debugger/src/async_debugger.rs`

```rust
use transport2::{split, connect, Message, DapReader, DapWriter};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use futures::StreamExt;

pub struct AsyncDebugger {
    internals: Arc<tokio::sync::Mutex<AsyncDebuggerInternals>>,
    event_rx: AsyncEventReceiver,
    cancel_token: CancellationToken,

    // Task handles for cleanup
    reader_handle: tokio::task::JoinHandle<()>,
    processor_handle: tokio::task::JoinHandle<()>,
}
```

#### 2.2 Connection and Initialization

```rust
impl AsyncDebugger {
    pub async fn connect(
        port: u16,
        language: Language,
        config: &LaunchConfiguration,
        stop_on_entry: bool,
    ) -> eyre::Result<Self> {
        // Connect using transport2
        let (reader, writer) = transport2::connect(format!("127.0.0.1:{port}")).await?;

        // Create channels
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Create internals
        let internals = Arc::new(tokio::sync::Mutex::new(AsyncDebuggerInternals::new(
            writer,
            event_tx.clone(),
        )));

        // Spawn reader task
        let reader_handle = Self::spawn_reader_task(
            reader,
            event_tx,
            message_tx,
            cancel_token.clone(),
        );

        // Spawn event processor task
        let processor_handle = Self::spawn_processor_task(
            message_rx,
            internals.clone(),
            cancel_token.clone(),
        );

        let debugger = Self {
            internals,
            event_rx: AsyncEventReceiver { rx: event_rx },
            cancel_token,
            reader_handle,
            processor_handle,
        };

        // Initialize DAP session
        debugger.initialize(language, config, stop_on_entry).await?;

        Ok(debugger)
    }
}
```

#### 2.3 Reader Task

```rust
impl AsyncDebugger {
    fn spawn_reader_task(
        mut reader: DapReader<OwnedReadHalf>,
        event_tx: mpsc::UnboundedSender<Event>,
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
                                // Forward events
                                if let Message::Event(ref evt) = message {
                                    if let Some(event) = convert_transport_event(evt) {
                                        let _ = event_tx.send(event);
                                    }
                                }
                                // Forward all messages
                                if message_tx.send(message).is_err() {
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
}
```

#### 2.4 Public API Methods

```rust
impl AsyncDebugger {
    /// Get event receiver for subscribing to debugger events
    pub fn events(&mut self) -> &mut AsyncEventReceiver {
        &mut self.event_rx
    }

    /// Start the debugging session
    pub async fn start(&self) -> eyre::Result<()> {
        let mut internals = self.internals.lock().await;
        internals.send_request(RequestBody::ConfigurationDone).await?;
        Ok(())
    }

    /// Continue execution
    pub async fn continue_(&self) -> eyre::Result<()> {
        let mut internals = self.internals.lock().await;
        let thread_id = internals.current_thread_id
            .ok_or_else(|| eyre::eyre!("no current thread"))?;
        internals.send_request(RequestBody::Continue { thread_id }).await?;
        Ok(())
    }

    /// Step over (next line)
    pub async fn step_over(&self) -> eyre::Result<()> {
        let mut internals = self.internals.lock().await;
        let thread_id = internals.current_thread_id
            .ok_or_else(|| eyre::eyre!("no current thread"))?;
        internals.send_request(RequestBody::Next { thread_id }).await?;
        Ok(())
    }

    /// Step into function
    pub async fn step_in(&self) -> eyre::Result<()> {
        let mut internals = self.internals.lock().await;
        let thread_id = internals.current_thread_id
            .ok_or_else(|| eyre::eyre!("no current thread"))?;
        internals.send_request(RequestBody::StepIn { thread_id }).await?;
        Ok(())
    }

    /// Step out of function
    pub async fn step_out(&self) -> eyre::Result<()> {
        let mut internals = self.internals.lock().await;
        let thread_id = internals.current_thread_id
            .ok_or_else(|| eyre::eyre!("no current thread"))?;
        internals.send_request(RequestBody::StepOut { thread_id }).await?;
        Ok(())
    }

    /// Evaluate expression in current frame
    pub async fn evaluate(
        &self,
        expression: &str,
        frame_id: StackFrameId,
    ) -> eyre::Result<EvaluateResult> {
        let internals = self.internals.lock().await;
        internals.evaluate_async(expression, frame_id).await
    }

    /// Add a breakpoint
    pub async fn add_breakpoint(&self, breakpoint: &Breakpoint) -> eyre::Result<BreakpointId> {
        let mut internals = self.internals.lock().await;
        internals.add_breakpoint_async(breakpoint).await
    }

    /// Get current breakpoints
    pub async fn breakpoints(&self) -> Vec<Breakpoint> {
        let internals = self.internals.lock().await;
        internals.breakpoints.values().cloned().collect()
    }

    /// Get variables for a scope
    pub async fn variables(&self, variables_reference: i64) -> eyre::Result<Vec<Variable>> {
        let internals = self.internals.lock().await;
        internals.variables_async(variables_reference).await
    }

    /// Shutdown the debugger
    pub async fn shutdown(self) -> eyre::Result<()> {
        self.cancel_token.cancel();
        let _ = self.reader_handle.await;
        let _ = self.processor_handle.await;
        Ok(())
    }
}

impl Drop for AsyncDebugger {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}
```

### Phase 3: Event Processing Migration

#### 3.1 Async Event Handler

```rust
impl AsyncDebuggerInternals {
    pub async fn on_event(&mut self, event: &transport::events::Event) -> eyre::Result<()> {
        match event {
            transport::events::Event::Stopped(body) => {
                self.current_thread_id = Some(body.thread_id);

                // Fetch stack trace
                let stack = self.fetch_stack_trace(body.thread_id).await?;

                // Emit paused event
                self.event_tx.send(Event::Paused(ProgramState {
                    stack,
                    breakpoints: self.breakpoints.values().cloned().collect(),
                    paused_frame: self.build_paused_frame().await?,
                }))?;
            }
            transport::events::Event::Continued(_) => {
                self.event_tx.send(Event::Running)?;
            }
            transport::events::Event::Terminated => {
                self.event_tx.send(Event::Ended)?;
            }
            // ... other events
        }
        Ok(())
    }

    async fn fetch_stack_trace(&self, thread_id: ThreadId) -> eyre::Result<Vec<StackFrame>> {
        // Send stack trace request and await response
        // ...
    }
}
```

#### 3.2 Request-Response Correlation

```rust
struct PendingRequest {
    tx: oneshot::Sender<Response>,
}

impl AsyncDebuggerInternals {
    async fn send_and_wait(&mut self, body: RequestBody) -> eyre::Result<Response> {
        let seq = self.send_request(body).await?;

        let (tx, rx) = oneshot::channel();
        self.pending_requests.insert(seq, PendingRequest { tx });

        // Await response with timeout
        tokio::time::timeout(
            Duration::from_secs(30),
            rx,
        ).await??
    }

    fn handle_response(&mut self, response: Response) {
        if let Some(pending) = self.pending_requests.remove(&response.request_seq) {
            let _ = pending.tx.send(response);
        }
    }
}
```

### Phase 4: Testing Infrastructure

#### 4.1 Async Test Utilities

File: `crates/debugger/src/testing.rs`

```rust
use transport2::testing::MemoryTransport;

pub struct TestDebugger {
    pub debugger: AsyncDebugger,
    pub mock_adapter: MockAdapter,
}

impl TestDebugger {
    pub async fn new() -> Self {
        let (client_transport, adapter_transport) = MemoryTransport::pair();

        let (reader, writer) = transport2::split(client_transport);
        let mock_adapter = MockAdapter::new(adapter_transport);

        // Create debugger with test transport
        let debugger = AsyncDebugger::from_transport(reader, writer).await.unwrap();

        Self { debugger, mock_adapter }
    }
}

pub struct MockAdapter {
    reader: DapReader<DuplexStream>,
    writer: DapWriter<DuplexStream>,
}

impl MockAdapter {
    pub async fn expect_request(&mut self, command: &str) -> Request {
        // Wait for request from debugger
    }

    pub async fn send_response(&mut self, response: Response) {
        // Send response to debugger
    }

    pub async fn send_event(&mut self, event: Event) {
        // Send event to debugger
    }
}
```

#### 4.2 Example Test

```rust
#[tokio::test]
async fn test_step_over() {
    let test = TestDebugger::new().await;

    // Mock adapter expects step request
    let handle = tokio::spawn(async move {
        let req = test.mock_adapter.expect_request("next").await;
        test.mock_adapter.send_response(Response::success(req.seq)).await;
        test.mock_adapter.send_event(Event::Stopped { ... }).await;
    });

    // Debugger sends step over
    test.debugger.step_over().await.unwrap();

    // Wait for stopped event
    let event = test.debugger.events().recv().await.unwrap();
    assert!(matches!(event, Event::Paused(_)));

    handle.await.unwrap();
}
```

---

## GUI-POC Integration

The gui-poc crate is currently a standalone prototype with mock state. Integration requires:

1. Adding it to the workspace
2. Replacing MockState with real AsyncDebugger
3. Running async operations from the UI thread

### Current GUI-POC Structure

```rust
// Current mock-based structure
struct App {
    state: MockState,  // Contains mock debugger state
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard_input(ctx);
        // Render panels...
    }
}
```

### Target Integration Structure

```rust
use debugger::AsyncDebugger;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// Commands sent from UI to async runtime
enum UiCommand {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    AddBreakpoint(Breakpoint),
    RemoveBreakpoint(BreakpointId),
    Evaluate(String, StackFrameId),
}

/// State updates received from async runtime
enum StateUpdate {
    DebuggerEvent(debugger::Event),
    EvaluateResult(String),
    Error(String),
}

struct App {
    // UI state
    ui_state: UiState,

    // Communication with async runtime
    command_tx: mpsc::UnboundedSender<UiCommand>,
    update_rx: mpsc::UnboundedReceiver<StateUpdate>,

    // Runtime handle (owned by app)
    _runtime: Runtime,
}

struct UiState {
    // Debugger state (updated from events)
    is_running: bool,
    current_file: Option<String>,
    current_line: Option<usize>,
    stack_frames: Vec<StackFrame>,
    selected_frame: usize,
    variables: Vec<Variable>,
    breakpoints: Vec<Breakpoint>,
    console_output: Vec<String>,

    // UI-only state
    selected_tab: BottomPanelTab,
    editor_mode: EditorMode,
    selected_node: Option<SelectedNode>,

    // AST state
    parsed_tree: Option<Tree>,
    source_code: String,
}
```

### Integration Implementation

#### Step 1: Add to Workspace

Update root `Cargo.toml`:
```toml
[workspace]
members = ["crates/*"]
# Remove: exclude = ["crates/gui-poc"]
```

Update `crates/gui-poc/Cargo.toml`:
```toml
[dependencies]
debugger = { path = "../debugger" }
tokio = { version = "1.48", features = ["sync", "rt-multi-thread"] }
# ... existing deps
```

#### Step 2: Create Async Bridge

File: `crates/gui-poc/src/async_bridge.rs`

```rust
use debugger::{AsyncDebugger, Event};
use tokio::sync::mpsc;
use tokio::runtime::Runtime;

pub struct AsyncBridge {
    runtime: Runtime,
    command_tx: mpsc::UnboundedSender<UiCommand>,
    update_rx: mpsc::UnboundedReceiver<StateUpdate>,
}

impl AsyncBridge {
    pub fn new(port: u16, config: LaunchConfiguration) -> eyre::Result<Self> {
        let runtime = Runtime::new()?;
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Spawn the debugger management task
        runtime.spawn(Self::run_debugger(port, config, command_rx, update_tx));

        Ok(Self {
            runtime,
            command_tx,
            update_rx,
        })
    }

    async fn run_debugger(
        port: u16,
        config: LaunchConfiguration,
        mut command_rx: mpsc::UnboundedReceiver<UiCommand>,
        update_tx: mpsc::UnboundedSender<StateUpdate>,
    ) {
        // Connect to debugger
        let debugger = match AsyncDebugger::connect(port, Language::Python, &config, true).await {
            Ok(d) => d,
            Err(e) => {
                let _ = update_tx.send(StateUpdate::Error(e.to_string()));
                return;
            }
        };

        let mut events = debugger.events();

        loop {
            tokio::select! {
                // Handle commands from UI
                Some(cmd) = command_rx.recv() => {
                    let result = match cmd {
                        UiCommand::Continue => debugger.continue_().await,
                        UiCommand::StepOver => debugger.step_over().await,
                        UiCommand::StepIn => debugger.step_in().await,
                        UiCommand::StepOut => debugger.step_out().await,
                        // ... other commands
                    };
                    if let Err(e) = result {
                        let _ = update_tx.send(StateUpdate::Error(e.to_string()));
                    }
                }

                // Handle events from debugger
                Some(event) = events.recv() => {
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
```

#### Step 3: Update App Structure

File: `crates/gui-poc/src/main.rs`

```rust
struct App {
    ui_state: UiState,
    bridge: Option<AsyncBridge>,

    // Launch dialog state
    show_launch_dialog: bool,
    launch_config: LaunchDialogState,
}

impl App {
    fn new(_cc: &eframe::CreationContext) -> Self {
        Self {
            ui_state: UiState::default(),
            bridge: None,
            show_launch_dialog: true,
            launch_config: LaunchDialogState::default(),
        }
    }

    fn connect_debugger(&mut self, config: LaunchConfiguration) {
        match AsyncBridge::new(config.port, config) {
            Ok(bridge) => {
                self.bridge = Some(bridge);
                self.show_launch_dialog = false;
            }
            Err(e) => {
                self.ui_state.console_output.push(format!("Error: {}", e));
            }
        }
    }

    fn process_updates(&mut self) {
        if let Some(bridge) = &mut self.bridge {
            for update in bridge.poll_updates() {
                match update {
                    StateUpdate::DebuggerEvent(event) => {
                        self.handle_debugger_event(event);
                    }
                    StateUpdate::EvaluateResult(result) => {
                        self.ui_state.last_evaluation = Some(result);
                    }
                    StateUpdate::Error(msg) => {
                        self.ui_state.console_output.push(format!("Error: {}", msg));
                    }
                }
            }
        }
    }

    fn handle_debugger_event(&mut self, event: debugger::Event) {
        match event {
            debugger::Event::Paused(state) => {
                self.ui_state.is_running = false;
                self.ui_state.stack_frames = state.stack;
                self.ui_state.breakpoints = state.breakpoints;
                if let Some(frame) = state.paused_frame.frame {
                    self.ui_state.current_file = frame.source.map(|s| s.path);
                    self.ui_state.current_line = Some(frame.line as usize);
                }
            }
            debugger::Event::Running => {
                self.ui_state.is_running = true;
            }
            debugger::Event::Ended => {
                self.bridge = None;
                self.ui_state = UiState::default();
            }
            _ => {}
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process async updates each frame
        self.process_updates();

        // Request repaint to check for updates
        if self.bridge.is_some() {
            ctx.request_repaint();
        }

        // Handle keyboard input
        self.handle_keyboard_input(ctx);

        // Render UI...
    }
}
```

#### Step 4: Wire Up Control Buttons

```rust
fn render_control_buttons(&mut self, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let is_connected = self.bridge.is_some();
        let is_paused = !self.ui_state.is_running;

        // Continue/Pause button
        if is_paused {
            if ui.add_enabled(is_connected, egui::Button::new("Continue (F5)")).clicked() {
                if let Some(bridge) = &self.bridge {
                    bridge.send_command(UiCommand::Continue);
                }
            }
        } else {
            if ui.add_enabled(is_connected, egui::Button::new("Pause (F5)")).clicked() {
                // Send pause command
            }
        }

        // Step buttons (only enabled when paused)
        if ui.add_enabled(is_connected && is_paused, egui::Button::new("Step Over (F10)")).clicked() {
            if let Some(bridge) = &self.bridge {
                bridge.send_command(UiCommand::StepOver);
            }
        }

        if ui.add_enabled(is_connected && is_paused, egui::Button::new("Step In (F11)")).clicked() {
            if let Some(bridge) = &self.bridge {
                bridge.send_command(UiCommand::StepIn);
            }
        }

        if ui.add_enabled(is_connected && is_paused, egui::Button::new("Step Out (Shift+F11)")).clicked() {
            if let Some(bridge) = &self.bridge {
                bridge.send_command(UiCommand::StepOut);
            }
        }
    });
}
```

#### Step 5: Integrate AST Evaluation

```rust
fn evaluate_selected_node(&mut self) {
    if let (Some(bridge), Some(node), Some(frame_id)) = (
        &self.bridge,
        &self.ui_state.selected_node,
        self.ui_state.current_frame_id,
    ) {
        bridge.send_command(UiCommand::Evaluate(node.text.clone(), frame_id));
    }
}
```

---

## Task Checklist

### Phase 1: Infrastructure Setup
- [ ] Add transport2 and tokio dependencies to debugger/Cargo.toml
- [ ] Create `async_event.rs` with AsyncEventReceiver
- [ ] Create `async_internals.rs` with AsyncDebuggerInternals
- [ ] Add tokio-stream dependency for Stream conversions
- [ ] Verify compilation with `cargo check -p debugger`

### Phase 2: Core AsyncDebugger Implementation
- [ ] Create `async_debugger.rs` with AsyncDebugger struct
- [ ] Implement `AsyncDebugger::connect()` using transport2
- [ ] Implement reader task spawning with cancellation support
- [ ] Implement event processor task
- [ ] Add request-response correlation with pending requests map
- [ ] Implement `start()` method
- [ ] Implement `continue_()` method
- [ ] Implement `step_over()` method
- [ ] Implement `step_in()` method
- [ ] Implement `step_out()` method
- [ ] Implement `evaluate()` method
- [ ] Implement `add_breakpoint()` method
- [ ] Implement `remove_breakpoint()` method
- [ ] Implement `breakpoints()` method
- [ ] Implement `variables()` method
- [ ] Implement `change_scope()` method
- [ ] Implement `shutdown()` method
- [ ] Implement `Drop` for cleanup

### Phase 3: Event Processing
- [ ] Create async event handler in AsyncDebuggerInternals
- [ ] Migrate `on_event()` to async version
- [ ] Implement async `fetch_stack_trace()`
- [ ] Implement async `fetch_scopes()`
- [ ] Implement async `fetch_variables()`
- [ ] Handle Stopped event with full state fetch
- [ ] Handle Continued event
- [ ] Handle Terminated event
- [ ] Handle Output event (console output)
- [ ] Handle Thread event

### Phase 4: DAP Protocol Implementation
- [ ] Implement async Initialize request/response
- [ ] Implement async Launch request/response
- [ ] Implement async Attach request/response
- [ ] Implement async SetBreakpoints request/response
- [ ] Implement async SetFunctionBreakpoints request/response
- [ ] Implement async SetExceptionBreakpoints request/response
- [ ] Implement async ConfigurationDone request
- [ ] Implement async Disconnect request
- [ ] Implement async Terminate request
- [ ] Add timeout handling for requests
- [ ] Add error handling for failed requests

### Phase 5: Testing Infrastructure
- [ ] Create `testing.rs` module
- [ ] Implement TestDebugger with MemoryTransport
- [ ] Implement MockAdapter for simulating debug adapter
- [ ] Write test for connect/initialize flow
- [ ] Write test for step_over
- [ ] Write test for step_in
- [ ] Write test for step_out
- [ ] Write test for continue
- [ ] Write test for breakpoint management
- [ ] Write test for evaluate
- [ ] Write test for variables fetch
- [ ] Write test for error handling
- [ ] Write test for cancellation/shutdown
- [ ] Run integration tests with real debugpy

### Phase 6: GUI-POC Integration
- [ ] Remove gui-poc from workspace exclude list
- [ ] Add debugger dependency to gui-poc/Cargo.toml
- [ ] Add tokio dependency to gui-poc/Cargo.toml
- [ ] Create `async_bridge.rs` module
- [ ] Implement UiCommand enum
- [ ] Implement StateUpdate enum
- [ ] Implement AsyncBridge struct
- [ ] Implement command sending (non-blocking)
- [ ] Implement update polling (non-blocking)
- [ ] Update App struct to use AsyncBridge
- [ ] Add launch/connect dialog
- [ ] Implement `process_updates()` in update loop
- [ ] Wire up Continue button to UiCommand::Continue
- [ ] Wire up Step Over button to UiCommand::StepOver
- [ ] Wire up Step In button to UiCommand::StepIn
- [ ] Wire up Step Out button to UiCommand::StepOut
- [ ] Wire up keyboard shortcuts to commands
- [ ] Implement breakpoint toggle on line click
- [ ] Wire up AST evaluation to UiCommand::Evaluate
- [ ] Display evaluation results in overlay
- [ ] Update stack frame display from events
- [ ] Update variables display from events
- [ ] Update current line highlight from events
- [ ] Load real source files from paused frame
- [ ] Update console output from Output events
- [ ] Handle debugger disconnection gracefully
- [ ] Test full debug session with debugpy

### Phase 7: Cleanup and Documentation
- [ ] Remove old transport crate dependency from debugger
- [ ] Remove synchronous Debugger struct (or deprecate)
- [ ] Update debugger crate README
- [ ] Update gui-poc README with integration notes
- [ ] Add rustdoc comments to all public async APIs
- [ ] Run `cargo fmt` on all modified files
- [ ] Run `cargo clippy` and fix warnings
- [ ] Run full test suite
- [ ] Update CLAUDE.md if needed

### Phase 8: Other UI Crate Updates (Future)
- [ ] Update gui crate to use AsyncDebugger
- [ ] Update gui2 crate to use AsyncDebugger
- [ ] Update tui crate to use AsyncDebugger
- [ ] Update repl crate to use AsyncDebugger

---

## API Reference

### AsyncDebugger Public API

```rust
impl AsyncDebugger {
    // Construction
    pub async fn connect(port: u16, language: Language, config: &LaunchConfiguration, stop_on_entry: bool) -> Result<Self>;

    // Event subscription
    pub fn events(&mut self) -> &mut AsyncEventReceiver;

    // Session lifecycle
    pub async fn start(&self) -> Result<()>;
    pub async fn shutdown(self) -> Result<()>;

    // Execution control
    pub async fn continue_(&self) -> Result<()>;
    pub async fn step_over(&self) -> Result<()>;
    pub async fn step_in(&self) -> Result<()>;
    pub async fn step_out(&self) -> Result<()>;

    // Breakpoints
    pub async fn add_breakpoint(&self, bp: &Breakpoint) -> Result<BreakpointId>;
    pub async fn remove_breakpoint(&self, id: BreakpointId) -> Result<()>;
    pub async fn breakpoints(&self) -> Vec<Breakpoint>;

    // Inspection
    pub async fn evaluate(&self, expr: &str, frame_id: StackFrameId) -> Result<EvaluateResult>;
    pub async fn variables(&self, ref: i64) -> Result<Vec<Variable>>;
    pub async fn change_scope(&self, frame_id: StackFrameId) -> Result<()>;
}
```

### AsyncEventReceiver API

```rust
impl AsyncEventReceiver {
    pub async fn recv(&mut self) -> Option<Event>;
    pub fn into_stream(self) -> impl Stream<Item = Event>;
}
```

---

## References

- [transport2 DESIGN.md](../transport2/DESIGN.md) - Transport layer architecture
- [notes/startup_sequence.md](../../notes/startup_sequence.md) - DAP initialization sequence
- [tokio documentation](https://tokio.rs/tokio/tutorial) - Async runtime patterns
- [egui integration guide](https://docs.rs/eframe/latest/eframe/) - UI framework patterns
