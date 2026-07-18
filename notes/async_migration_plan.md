# Async Migration Plan for Transport and Debugger Crates

## Executive Summary

This document outlines a comprehensive plan to migrate the `transport` and `debugger` crates from sync-based concurrency (using threads and channels) to async-based concurrency using **tokio**. The migration will provide better support for cancellation, timeouts, and integration with modern Rust async ecosystems, while maintaining compatibility with both egui and iced UI frameworks.

## Current Architecture Analysis

### Transport Crate (`crates/transport`)

**Current Design:**
- **Background Thread Pattern**: Spawns a dedicated thread in `Client::with_transport()` (line 95) that continuously polls for messages
- **Request-Response Matching**: Uses `RequestStore` (Arc<Mutex<HashMap>>) with oneshot channels to match responses to requests by sequence number
- **Blocking Sends**: The `send()` method blocks using a polling loop with `try_recv()` and exponential backoff (lines 240-260)
- **Channel-based Events**: Uses `crossbeam_channel::unbounded()` to publish events to subscribers
- **Mutex Synchronization**: Heavy use of `Arc<Mutex<T>>` for shared state (ClientInternals, RequestStore)

**Key Pain Points:**
1. Manual polling loop in `send()` wastes CPU cycles (lines 240-260)
2. No native timeout support - implemented manually
3. No cancellation mechanism for in-flight requests
4. Background thread management is manual (oneshot for shutdown)
5. The `try_recv()` loop has to balance between latency and CPU usage

### Debugger Crate (`crates/debugger`)

**Current Design:**
- **Background Event Thread**: Spawns a thread to read transport events and update internal state (lines 180-212 in debugger.rs)
- **Mutex-Protected State**: `DebuggerInternals` wrapped in `Arc<Mutex<T>>`
- **Blocking Operations**: All operations (`continue()`, `step_over()`, etc.) acquire mutex and block
- **Channel-based Publish**: Uses `crossbeam_channel` to publish state changes to UI
- **Synchronous Client API**: Direct calls to `transport::Client::send()` which blocks

**Key Pain Points:**
1. Multiple sources of locking (debugger internals + transport internals)
2. Potential for deadlocks if not careful with lock ordering
3. No way to cancel operations in progress
4. Background thread lifecycle tied to Drop implementation
5. Difficult to implement timeouts for operations

## Proposed Async Architecture

### Design Principles

1. **Tokio-First**: Use tokio as the async runtime throughout, leveraging its mature ecosystem and excellent tooling
2. **Structured Concurrency**: Use async tasks instead of raw threads, with proper cancellation support via `tokio_util::CancellationToken`
3. **Zero-Cost Abstractions**: Maintain performance while gaining async benefits
4. **UI Framework Compatibility**: Ensure seamless integration with both egui and iced
5. **Native Async Primitives**: Use tokio's sync primitives (`tokio::sync::Mutex`, `RwLock`, `broadcast`, etc.) throughout

### Async Transport Crate Design

#### Core Changes

**1. Async Client API**

```rust
pub struct AsyncClient {
    internals: Arc<AsyncClientInternals>,
}

struct AsyncClientInternals {
    // Writer is still sync, protected by tokio async mutex
    output: tokio::sync::Mutex<Box<dyn Write + Send>>,

    // Shared state for tracking requests
    sequence_number: AtomicI64,
    store: Arc<tokio::sync::RwLock<HashMap<Seq, WaitingRequest>>>,

    // Event publisher using tokio broadcast
    event_tx: tokio::sync::broadcast::Sender<events::Event>,

    // Shutdown mechanism
    shutdown: Arc<tokio::sync::Notify>,
}

pub struct WaitingRequest {
    body: RequestBody,
    tx: tokio::sync::oneshot::Sender<Response>,
}
```

**2. Async Message Loop**

Replace the blocking thread with an async task:

```rust
impl AsyncClient {
    pub async fn with_transport<T>(
        transport: T,
        event_tx: tokio::sync::broadcast::Sender<events::Event>,
    ) -> Result<Self>
    where
        T: AsyncDapTransport,
    {
        let (mut reader, writer) = transport.split().await?;

        let internals = Arc::new(AsyncClientInternals {
            output: tokio::sync::Mutex::new(Box::new(writer)),
            sequence_number: AtomicI64::new(0),
            store: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            event_tx,
            shutdown: Arc::new(tokio::sync::Notify::new()),
        });

        // Spawn async message loop
        let loop_internals = Arc::clone(&internals);
        tokio::spawn(async move {
            Self::message_loop(reader, loop_internals).await
        });

        Ok(Self { internals })
    }

    async fn message_loop<R>(
        mut reader: R,
        internals: Arc<AsyncClientInternals>,
    )
    where
        R: AsyncBufRead + Unpin,
    {
        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = internals.shutdown.notified() => {
                    tracing::debug!("shutting down message loop");
                    break;
                }

                // Poll for next message
                msg_result = Self::read_message(&mut reader) => {
                    match msg_result {
                        Ok(Some(Message::Event(evt))) => {
                            // tokio broadcast returns Result with send count
                            let _ = internals.event_tx.send(evt);
                        }
                        Ok(Some(Message::Response(resp))) => {
                            // Match response to waiting request
                            if let Some(waiting) = internals.store
                                .write()
                                .await
                                .remove(&resp.request_seq)
                            {
                                let _ = waiting.tx.send(resp);
                            }
                        }
                        Ok(None) => break, // EOF
                        Err(e) => tracing::warn!("reader error: {e}"),
                        _ => {}
                    }
                }
            }
        }
    }
}
```

**3. Async Send with Timeout**

```rust
impl AsyncClient {
    pub async fn send(&self, body: RequestBody) -> Result<Response> {
        self.send_with_timeout(body, Duration::from_secs(30)).await
    }

    pub async fn send_with_timeout(
        &self,
        body: RequestBody,
        timeout: Duration,
    ) -> Result<Response> {
        let seq = self.internals.sequence_number.fetch_add(1, Ordering::SeqCst) + 1;
        let message = Request { seq, r#type: "request".to_string(), body };

        let (tx, rx) = tokio::sync::oneshot::channel();

        // Register request in store
        {
            let mut store = self.internals.store.write().await;
            store.insert(seq, WaitingRequest { body: message.body.clone(), tx });
        }

        // Send message
        {
            let mut output = self.internals.output.lock().await;
            let json = serde_json::to_string(&message)?;
            write!(output, "Content-Length: {}\r\n\r\n{}", json.len(), json)?;
            output.flush()?;
        }

        // Wait for response with timeout - this is where async shines!
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(eyre::eyre!("Response sender disconnected")),
            Err(_) => {
                // Timeout - clean up waiting request
                self.internals.store.write().await.remove(&seq);
                Err(eyre::eyre!("Request timeout after {:?}", timeout))
            }
        }
    }

    // Non-blocking send that doesn't wait for response
    pub async fn execute(&self, body: RequestBody) -> Result<()> {
        let seq = self.internals.sequence_number.fetch_add(1, Ordering::SeqCst) + 1;
        let message = Request { seq, r#type: "request".to_string(), body };

        let mut output = self.internals.output.lock().await;
        let json = serde_json::to_string(&message)?;
        write!(output, "Content-Length: {}\r\n\r\n{}", json.len(), json)?;
        output.flush()?;

        Ok(())
    }
}
```

**4. Async Transport Trait**

```rust
pub trait AsyncDapTransport: Send + 'static {
    type Reader: AsyncBufRead + Send + Unpin + 'static;
    type Writer: AsyncWrite + Send + Unpin + 'static;

    async fn split(self) -> eyre::Result<(Self::Reader, Self::Writer)>;
}

// Async TCP transport
pub struct AsyncTcpTransport {
    stream: TcpStream,
}

impl AsyncTcpTransport {
    pub async fn connect(addr: impl ToSocketAddrs) -> eyre::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self { stream })
    }
}

impl AsyncDapTransport for AsyncTcpTransport {
    type Reader = tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>;
    type Writer = tokio::net::tcp::OwnedWriteHalf;

    async fn split(self) -> eyre::Result<(Self::Reader, Self::Writer)> {
        let (read_half, write_half) = self.stream.into_split();
        Ok((tokio::io::BufReader::new(read_half), write_half))
    }
}
```

**5. Cancellation Support**

```rust
use tokio_util::sync::CancellationToken;

pub struct CancellableRequest {
    response_future: tokio::task::JoinHandle<Result<Response>>,
    cancel_token: CancellationToken,
}

impl CancellableRequest {
    pub async fn wait(self) -> Result<Response> {
        self.response_future.await?
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }
}

impl AsyncClient {
    pub fn send_cancellable(&self, body: RequestBody) -> CancellableRequest {
        let cancel_token = CancellationToken::new();
        let client = self.clone();
        let token = cancel_token.clone();

        let handle = tokio::spawn(async move {
            tokio::select! {
                result = client.send(body) => result,
                _ = token.cancelled() => Err(eyre::eyre!("Request cancelled")),
            }
        });

        CancellableRequest {
            response_future: handle,
            cancel_token,
        }
    }
}
```

### Async Debugger Crate Design

#### Core Changes

**1. Async Debugger API**

```rust
pub struct AsyncDebugger {
    internals: Arc<tokio::sync::RwLock<DebuggerInternals>>,
    event_rx: tokio::sync::broadcast::Receiver<Event>,
    event_tx: tokio::sync::broadcast::Sender<Event>,
}

struct DebuggerInternals {
    client: AsyncClient,
    publisher: tokio::sync::broadcast::Sender<Event>,
    current_thread_id: Option<ThreadId>,
    breakpoints: HashMap<BreakpointId, Breakpoint>,
    current_breakpoint_id: BreakpointId,
    current_source: Option<FileSource>,
    _server: Option<Box<dyn Server + Send>>,
}
```

**2. Async Event Handling**

Replace the background thread with an async task:

```rust
impl AsyncDebugger {
    pub async fn on_port(
        port: u16,
        initialise_arguments: impl Into<InitialiseArguments>,
    ) -> eyre::Result<Self> {
        let (event_tx, event_rx) = tokio::sync::broadcast::channel(100);
        let _ = event_tx.send(Event::Uninitialised);

        let args: InitialiseArguments = initialise_arguments.into();

        // Create transport and client
        let transport = AsyncTcpTransport::connect(format!("127.0.0.1:{port}")).await?;
        let (transport_tx, mut transport_rx) = tokio::sync::broadcast::channel(100);
        let client = AsyncClient::with_transport(transport, transport_tx).await?;

        // Initialize internals
        let mut internals = DebuggerInternals::new(client, event_tx.clone(), server);
        internals.initialise(args).await?;

        let internals = Arc::new(tokio::sync::RwLock::new(internals));

        // Spawn event handling task
        let event_internals = Arc::clone(&internals);
        tokio::spawn(async move {
            while let Ok(event) = transport_rx.recv().await {
                let mut internals = event_internals.write().await;
                internals.on_event(event).await;
            }
        });

        Ok(Self {
            internals,
            event_rx: event_rx.resubscribe(),
            event_tx,
        })
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }
}
```

**3. Async Operations with Timeouts**

```rust
impl AsyncDebugger {
    pub async fn continue_execution(&self) -> eyre::Result<()> {
        let thread_id = {
            let internals = self.internals.read().await;
            internals.current_thread_id
                .ok_or_else(|| eyre::eyre!("no current thread id"))?
        };

        let internals = self.internals.read().await;
        internals.client
            .execute(RequestBody::Continue(Continue {
                thread_id,
                single_thread: false,
            }))
            .await
    }

    pub async fn add_breakpoint(&self, breakpoint: &Breakpoint) -> eyre::Result<BreakpointId> {
        let mut internals = self.internals.write().await;
        internals.add_breakpoint(breakpoint).await
    }

    pub async fn evaluate(
        &self,
        input: &str,
        frame_id: StackFrameId,
    ) -> eyre::Result<Option<EvaluateResult>> {
        let req = RequestBody::Evaluate(Evaluate {
            expression: input.to_string(),
            frame_id: Some(frame_id),
            context: Some("repl".to_string()),
        });

        let response = {
            let internals = self.internals.read().await;
            internals.client.send(req).await?
        };

        // Process response...
        Ok(/* ... */)
    }
}
```

**4. Better Resource Management**

```rust
impl AsyncDebugger {
    pub async fn shutdown(self) -> eyre::Result<()> {
        // Send disconnect
        let disconnect_result = {
            let internals = self.internals.read().await;
            internals.client
                .execute(RequestBody::Disconnect(Disconnect {
                    terminate_debugee: true,
                }))
                .await
        };

        // Wait for termination event with timeout
        let mut events = self.event_rx.clone();
        match tokio::time::timeout(Duration::from_secs(5), async {
            while let Ok(event) = events.recv().await {
                if matches!(event, Event::Ended) {
                    return;
                }
            }
        }).await {
            Ok(_) => tracing::debug!("debugger terminated gracefully"),
            Err(_) => tracing::warn!("timeout waiting for debugger termination"),
        }

        disconnect_result
    }
}

// No longer need Drop - explicit shutdown
```

## UI Framework Integration

### Egui Integration

Egui is immediate-mode and not inherently async-aware. We need a bridge pattern:

**Pattern 1: Channel-Based Bridge (Recommended)**

```rust
use tokio::sync::{broadcast, mpsc};

struct DebuggerBridge {
    // Command channel: UI -> Debugger
    command_tx: mpsc::UnboundedSender<DebuggerCommand>,

    // Event channel: Debugger -> UI (using broadcast for multiple subscribers)
    event_rx: broadcast::Receiver<Event>,

    // Current state (updated each frame)
    current_state: Arc<Mutex<State>>,
}

enum DebuggerCommand {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    AddBreakpoint(Breakpoint),
    Evaluate { input: String, frame_id: StackFrameId },
}

impl DebuggerBridge {
    pub fn new(debugger: AsyncDebugger, runtime: tokio::runtime::Handle) -> Self {
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        let event_rx = debugger.subscribe();
        let current_state = Arc::new(Mutex::new(State::Initialising));

        let state = Arc::clone(&current_state);
        runtime.spawn(async move {
            // Handle commands
            while let Some(cmd) = command_rx.recv().await {
                match cmd {
                    DebuggerCommand::Continue => {
                        let _ = debugger.continue_execution().await;
                    }
                    DebuggerCommand::StepOver => {
                        let _ = debugger.step_over().await;
                    }
                    // ... handle other commands
                }
            }
        });

        let state = Arc::clone(&current_state);
        let mut events = event_rx.resubscribe();
        runtime.spawn(async move {
            // Update state from events
            while let Ok(event) = events.recv().await {
                let mut state = state.lock().unwrap();
                *state = event.into();
            }
        });

        Self {
            command_tx,
            event_rx,
            current_state,
        }
    }

    pub fn send_command(&self, cmd: DebuggerCommand) {
        // Non-blocking send (mpsc unbounded never fails unless closed)
        let _ = self.command_tx.send(cmd);
    }

    pub fn current_state(&self) -> State {
        self.current_state.lock().unwrap().clone()
    }
}

// In egui app:
impl eframe::App for DebuggerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.bridge.current_state();

        if ui.button("Continue").clicked() {
            self.bridge.send_command(DebuggerCommand::Continue);
        }

        // Render based on state...
    }
}
```

**Pattern 2: poll-promise (Alternative)**

```rust
use poll_promise::Promise;

struct DebuggerApp {
    debugger: AsyncDebugger,
    runtime: tokio::runtime::Handle,
    continue_promise: Option<Promise<eyre::Result<()>>>,
}

impl eframe::App for DebuggerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ui.button("Continue").clicked() {
            let debugger = self.debugger.clone();
            self.continue_promise = Some(Promise::spawn_async(async move {
                debugger.continue_execution().await
            }));
        }

        if let Some(promise) = &self.continue_promise {
            match promise.poll() {
                Poll::Ready(result) => {
                    // Handle result
                    self.continue_promise = None;
                }
                Poll::Pending => {
                    // Show loading indicator
                }
            }
        }
    }
}
```

### Iced Integration

Iced has first-class async support through Commands and Subscriptions:

```rust
use iced::{Application, Command, Subscription};

struct DebuggerApp {
    debugger: Option<AsyncDebugger>,
    state: State,
}

#[derive(Debug, Clone)]
enum Message {
    DebuggerReady(AsyncDebugger),
    Event(debugger::Event),
    Continue,
    StepOver,
    AddBreakpoint(Breakpoint),
    // ... other messages
}

impl Application for DebuggerApp {
    type Message = Message;

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let init_command = Command::perform(
            async {
                AsyncDebugger::new(/* ... */).await.unwrap()
            },
            Message::DebuggerReady,
        );

        (
            Self {
                debugger: None,
                state: State::Initialising,
            },
            init_command,
        )
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::DebuggerReady(debugger) => {
                self.debugger = Some(debugger);
                Command::none()
            }
            Message::Event(event) => {
                self.state = event.into();
                Command::none()
            }
            Message::Continue => {
                if let Some(debugger) = &self.debugger {
                    let debugger = debugger.clone();
                    return Command::perform(
                        async move {
                            debugger.continue_execution().await
                        },
                        |_| Message::Event(Event::Running),
                    );
                }
                Command::none()
            }
            // ... handle other messages
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        if let Some(debugger) = &self.debugger {
            // Subscribe to debugger events
            subscription::unfold(
                "debugger_events",
                debugger.subscribe(),
                |mut rx| async move {
                    let event = rx.recv().await.ok()?;
                    Some((Message::Event(event), rx))
                },
            )
        } else {
            Subscription::none()
        }
    }
}
```

## Migration Strategy

### Phase 1: Preparation (Low Risk)

1. **Add tokio dependencies**
   - Add `tokio` with features: `["full", "tracing"]`
   - Add `tokio-util` for `CancellationToken` support
   - Add `async-trait` for trait methods (until native async traits stabilize)

2. **Create async transport trait alongside existing sync trait**
   - Keep existing `DapTransport` trait
   - Add new `AsyncDapTransport` trait
   - Implement `AsyncTcpTransport`

3. **Add feature flags**
   ```toml
   [features]
   default = ["sync"]
   sync = []
   async = ["tokio", "tokio-util", "async-trait"]
   ```

### Phase 2: Async Transport (Medium Risk)

1. **Implement `AsyncClient` alongside existing `Client`**
   - Create new `async_client.rs` module
   - Implement all async methods
   - Add tests using `tokio::test`

2. **Create async-aware transports**
   - `AsyncTcpTransport`
   - `AsyncInMemoryTransport` for testing

3. **Test thoroughly**
   - Port existing tests to async versions
   - Add timeout tests
   - Add cancellation tests

### Phase 3: Async Debugger (Medium Risk)

1. **Implement `AsyncDebugger` alongside existing `Debugger`**
   - Create new `async_debugger.rs` module
   - Implement async event loop
   - Add async versions of all public methods

2. **Update internals**
   - Convert `DebuggerInternals` to use async client
   - Replace thread spawning with task spawning
   - Update event handling to be async

3. **Test integration**
   - Port existing integration tests
   - Test with real debug adapters (debugpy, delve)

### Phase 4: UI Integration (Low Risk)

1. **Create egui bridge**
   - Implement `DebuggerBridge` pattern
   - Add example showing integration
   - Update existing GUI to use bridge

2. **Create iced example**
   - Implement full iced application
   - Demonstrate Command and Subscription patterns

### Phase 5: Stabilization (Low Risk)

1. **Performance testing**
   - Compare async vs sync performance
   - Optimize hot paths
   - Profile memory usage

2. **Documentation**
   - Update CLAUDE.md
   - Add migration guide for users
   - Document async patterns

3. **Deprecation path**
   - Mark sync versions as deprecated
   - Provide migration timeline
   - Eventually remove sync code (breaking change)

## Tokio Runtime Management

All async code will use the tokio runtime. Key considerations:

### Why Tokio

1. **Ecosystem**: Most popular async runtime with excellent crate support
2. **Primitives**: Mature timeout, cancellation, and sync primitives
3. **Tooling**: `tokio-console` for debugging, `tracing` integration
4. **Features**: Native support for broadcast channels, async RwLock, etc.

### Runtime Setup

**Egui Pattern:**
```rust
// Create runtime once at app startup
let runtime = tokio::runtime::Runtime::new()?;

// Keep runtime alive for app lifetime
struct DebuggerApp {
    _runtime: tokio::runtime::Runtime,
    bridge: DebuggerBridge,
}
```

**Iced Pattern:**
```rust
// Iced handles runtime internally
// No manual runtime management needed
```

## Benefits of Async Migration

### 1. Better Concurrency Primitives

- **Before (sync):** Manual polling loop with sleep/yield
  ```rust
  loop {
      match rx.try_recv() {
          Ok(response) => break response,
          Err(TryRecvError::Empty) => {
              if attempts < 1000 {
                  std::thread::yield_now();
              } else {
                  std::thread::sleep(Duration::from_millis(1));
              }
          }
      }
  }
  ```

- **After (async):** Natural async/await with timeout
  ```rust
  tokio::time::timeout(duration, rx).await?
  ```

### 2. Cancellation Support

- **Before:** No way to cancel in-flight requests
- **After:** `CancellableRequest` with `tokio_util::CancellationToken`

### 3. Structured Concurrency

- **Before:** Raw thread spawning with manual lifecycle
- **After:** Async tasks with automatic cleanup

### 4. Better Resource Usage

- **Before:** Dedicated OS thread per background task
- **After:** Lightweight async tasks on thread pool

### 5. Composability

Async functions compose naturally:

```rust
async fn debug_session() -> Result<()> {
    let debugger = AsyncDebugger::new(args).await?;

    // These can run concurrently
    let (breakpoints, stack) = tokio::join!(
        debugger.get_breakpoints(),
        debugger.get_stack_trace(),
    );

    // Timeout for entire operation
    tokio::time::timeout(
        Duration::from_secs(30),
        run_debugging_session(&debugger),
    ).await??;

    debugger.shutdown().await
}
```

## Challenges and Mitigations

### Challenge 1: Async Trait Methods

**Problem:** Trait methods with `async fn` require `#[async_trait]` macro

**Mitigation:**
- Use `async-trait` crate for now
- Plan to migrate to native async trait when stabilized in Rust

### Challenge 2: Send + Sync Bounds

**Problem:** Async code requires careful management of Send/Sync bounds

**Mitigation:**
- Use `async-lock` which provides Send-friendly synchronization
- Avoid holding locks across `.await` points when possible
- Use RwLock to allow concurrent reads

### Challenge 3: Runtime Dependency

**Problem:** Async code needs a runtime, adding dependency

**Mitigation:**
- Make runtime choice configurable via features
- Document runtime requirements clearly
- Provide examples for both egui and iced

### Challenge 4: Testing Complexity

**Problem:** Async tests need runtime setup

**Mitigation:**
- Use `#[tokio::test]` for test functions
- Create test helpers for common patterns
- Keep existing test coverage during migration

### Challenge 5: Breaking Changes

**Problem:** Existing users need to migrate

**Mitigation:**
- Keep sync API alongside async initially
- Provide detailed migration guide
- Use semantic versioning (major bump)

## Compatibility Matrix

| Component | Egui | Iced | CLI |
|-----------|------|------|-----|
| Sync Debugger | ✅ Current | ✅ Current | ✅ Current |
| Async Debugger | ✅ Via Bridge | ✅ Native | ✅ Via Runtime |
| Runtime Needed | Yes (manual) | No (built-in) | Yes (manual) |

## Timeline Estimate

| Phase | Effort | Risk | Duration |
|-------|--------|------|----------|
| 1. Preparation | Low | Low | 1-2 days |
| 2. Async Transport | Medium | Medium | 3-5 days |
| 3. Async Debugger | High | Medium | 5-7 days |
| 4. UI Integration | Medium | Low | 2-3 days |
| 5. Stabilization | Low | Low | 2-3 days |
| **Total** | - | - | **13-20 days** |

## Conclusion

Migrating to async with **tokio** will provide significant benefits:
- Native timeout and cancellation support via `tokio::time::timeout` and `tokio_util::CancellationToken`
- Better resource utilization (async tasks vs OS threads)
- More composable APIs with `tokio::join!` and `tokio::select!`
- Excellent ecosystem integration (most async crates support tokio)
- Superior debugging tools (`tokio-console`, tracing integration)

The migration can be done incrementally with low risk by keeping sync APIs alongside async during transition. Both egui and iced can work with the async debugger - egui needs a bridge pattern while iced has native async support.

**Recommendation:** Proceed with migration using **tokio as the exclusive async runtime**, implementing async alongside sync initially, then deprecating sync after stabilization period.

## References

### Egui Async Patterns
- [egui-async crate](https://docs.rs/egui-async/latest/egui_async/)
- [How to combine egui with tokio/async code](https://users.rust-lang.org/t/how-to-combine-egui-with-tokio-async-code/82500)
- [Combining tokio and egui - The Iron Code](https://actix.vdop.org/view_post?post_num=14)
- [egui async discussions](https://github.com/emilk/egui/discussions/634)

### Iced Async Patterns
- [Iced Documentation](https://docs.rs/iced/latest/iced/)
- [Iced Subscription](https://docs.iced.rs/iced/struct.Subscription.html)
- [iced_futures](https://docs.rs/iced_futures)

### Tokio Resources
- [Tokio Documentation](https://tokio.rs/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)

## Appendix: Code Examples

### Example: Complete Async Transport Implementation

See sections above for AsyncClient, AsyncDapTransport implementations.

### Example: Complete Async Debugger Implementation

See sections above for AsyncDebugger implementation.

### Example: Egui Integration

See "Egui Integration" section above.

### Example: Iced Integration

See "Iced Integration" section above.
