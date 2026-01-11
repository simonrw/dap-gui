# Debugger + transport2 Integration Plan

This document outlines the plan for integrating the new async `transport2` crate with the `debugger` crate.

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
| `TransportConnection` | `transport/client.rs` | TCP connection, splits into reader/writer |
| Polling thread | `debugger.rs:168` | Owns reader, calls `poll_message()` |
| Background thread | `debugger.rs:282` | Processes events and commands |

### Current Synchronization

- `Arc<Mutex<DebuggerInternals>>` - Shared state
- `Arc<Mutex<Box<dyn Write>>>` - Writer protection
- `Arc<AtomicI64>` - Lock-free sequence numbers
- `crossbeam_channel` - Event/message/command distribution
- `oneshot` - Request-response pattern

## Integration Options

### Option A: Fully Async Debugger

Convert the debugger crate to use async/await throughout.

```
                          tokio channels
    +------------------+                      +--------------------+
    |   Reader Task    |  -- events -->       |  Event Handler     |
    |                  |  -- messages -->     |  Task              |
    |  reader.next()   |                      |                    |
    |  (async Stream)  |                      |  async event proc  |
    +------------------+                      +--------------------+
                                                       |
                                              commands | events
                                                       v
                                              +------------------+
                                              |   Caller Task    |
                                              |   (async API)    |
                                              +------------------+
```

#### Changes Required

1. **New dependencies** in `debugger/Cargo.toml`:
   ```toml
   transport2 = { path = "../transport2" }
   tokio = { version = "1.48", features = ["sync", "rt"] }
   ```

2. **Replace TransportConnection** with transport2's split pattern:
   ```rust
   // Before
   let connection = TransportConnection::connect(addr)?;
   let (reader, writer, seq) = connection.split_connection();

   // After
   let (reader, writer) = transport2::connect(addr).await?;
   ```

3. **Replace polling thread** with async task:
   ```rust
   // Before: std::thread::spawn with blocking poll
   thread::spawn(move || {
       loop {
           match reader.poll_message() { ... }
       }
   });

   // After: tokio task with async stream
   tokio::spawn(async move {
       while let Some(msg) = reader.next().await {
           match msg { ... }
       }
   });
   ```

4. **Replace crossbeam channels** with tokio channels:
   ```rust
   // Before
   let (tx, rx) = crossbeam_channel::unbounded();

   // After
   let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
   ```

5. **Make public API async**:
   ```rust
   // Before
   pub fn continue_(&self) -> eyre::Result<()>

   // After
   pub async fn continue_(&self) -> eyre::Result<()>
   ```

6. **Replace Mutex with async Mutex** where held across await points:
   ```rust
   // Before
   let guard = self.internals.lock().unwrap();

   // After
   let guard = self.internals.lock().await;
   ```

#### Pros
- Clean async design throughout
- No thread spawning overhead
- Better resource utilization
- Natural fit with transport2's Stream/Sink API
- Easier to compose with other async code

#### Cons
- Breaking API change for all consumers (gui, gui2, tui, repl)
- All UI crates must adopt async runtime
- Larger scope of changes
- Need to handle async in test utilities

---

### Option B: Sync Debugger with Internal Tokio Runtime

Keep the debugger's public API synchronous but use tokio internally.

```
    +----------------------------------------------------------+
    |                    Debugger (sync API)                   |
    |  +----------------------------------------------------+  |
    |  |              Internal Tokio Runtime                |  |
    |  |                                                    |  |
    |  |   +-------------+          +------------------+    |  |
    |  |   | Reader Task |  ------> | Event Handler    |    |  |
    |  |   | (async)     |          | Task (async)     |    |  |
    |  |   +-------------+          +------------------+    |  |
    |  |                                                    |  |
    |  +----------------------------------------------------+  |
    |                            |                             |
    |                   crossbeam channels                     |
    |                            v                             |
    |                    Public sync API                       |
    +----------------------------------------------------------+
```

#### Changes Required

1. **Add runtime to DebuggerInternals**:
   ```rust
   pub(crate) struct DebuggerInternals {
       runtime: tokio::runtime::Runtime,
       reader_handle: tokio::task::JoinHandle<()>,
       writer: transport2::DapWriter<...>,
       // ... rest unchanged
   }
   ```

2. **Create runtime at initialization**:
   ```rust
   impl Debugger {
       pub fn on_port(port: u16, ...) -> eyre::Result<Self> {
           let runtime = tokio::runtime::Builder::new_multi_thread()
               .worker_threads(2)
               .enable_all()
               .build()?;

           let (reader, writer) = runtime.block_on(async {
               transport2::connect(format!("127.0.0.1:{port}")).await
           })?;

           // Spawn reader task on runtime
           let reader_handle = runtime.spawn(async move {
               while let Some(msg) = reader.next().await {
                   // Forward to crossbeam channel
               }
           });

           // ...
       }
   }
   ```

3. **Bridge async writer to sync API**:
   ```rust
   impl DebuggerInternals {
       pub(crate) fn send_request(&mut self, body: RequestBody) -> eyre::Result<Seq> {
           let msg = OutgoingMessage::Request(...);

           // Block on async send
           self.runtime.block_on(async {
               self.writer.send(msg).await
           })?;

           Ok(seq)
       }
   }
   ```

4. **Keep public API unchanged**:
   ```rust
   // No change needed
   pub fn continue_(&self) -> eyre::Result<()>
   ```

#### Pros
- No breaking changes to public API
- UI crates don't need modification
- Incremental migration path
- Can migrate to full async later
- Tests remain synchronous

#### Cons
- Runtime overhead (tokio runtime per Debugger instance)
- `block_on` calls add latency
- Mixed sync/async can be confusing
- Potential for deadlocks if not careful with `block_on`
- More complex internal architecture

---

### Option C: Hybrid Approach

Provide both sync and async APIs.

```rust
// Async API (new)
pub struct AsyncDebugger { ... }

impl AsyncDebugger {
    pub async fn connect(addr: &str) -> Result<Self>;
    pub async fn continue_(&self) -> Result<()>;
    pub fn events(&self) -> impl Stream<Item = Event>;
}

// Sync API (wrapper)
pub struct Debugger {
    inner: AsyncDebugger,
    runtime: tokio::runtime::Runtime,
}

impl Debugger {
    pub fn connect(addr: &str) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()?;
        let inner = runtime.block_on(AsyncDebugger::connect(addr))?;
        Ok(Self { inner, runtime })
    }

    pub fn continue_(&self) -> Result<()> {
        self.runtime.block_on(self.inner.continue_())
    }
}
```

#### Pros
- Best of both worlds
- Gradual migration for consumers
- Full async available when needed

#### Cons
- Code duplication / maintenance burden
- Two APIs to document and test

---

## Recommendation

**Option A (Fully Async)** is recommended for the following reasons:

1. **Alignment with transport2 design**: The transport2 crate was designed with async Stream/Sink patterns. Using it synchronously negates many benefits.

2. **Simpler long-term maintenance**: One consistent async model is easier to reason about than a sync/async bridge.

3. **UI framework compatibility**: Both egui and ratatui (tui) work well with async runtimes. The gui crates can run the debugger in a tokio task.

4. **Performance**: Async I/O is more efficient than blocking threads, especially for multiple concurrent debug sessions.

5. **Future extensibility**: Features like parallel debugging, remote debugging, and DAP server mode are easier with async.

---

## Implementation Plan

### Phase 1: Prepare Infrastructure

1. Add tokio dependencies to debugger crate
2. Create async channel wrappers compatible with current Event type
3. Add `#[cfg(feature = "async")]` gates for gradual migration

### Phase 2: Core Async Conversion

1. **Create AsyncDebuggerInternals**:
   - Replace `Arc<Mutex<Box<dyn Write>>>` with `DapWriter`
   - Replace `crossbeam_channel::Receiver<Message>` with message stream
   - Add methods: `send_request_async()`, `on_event_async()`

2. **Create AsyncDebugger**:
   - `async fn connect()` using transport2
   - Spawn reader task that forwards to channel
   - `async fn continue_()`, `async fn step_over()`, etc.
   - `fn events() -> impl Stream<Item = Event>`

3. **Migrate event processing**:
   - Convert `on_event()` to `async fn on_event()`
   - Handle follow-up requests with async/await instead of queue

### Phase 3: Update Consumers

1. **gui crate**:
   ```rust
   // In eframe app
   let runtime = tokio::runtime::Runtime::new()?;
   let debugger = runtime.block_on(AsyncDebugger::connect(...))?;

   // Or use eframe's async support
   ```

2. **gui2 crate**: Similar to gui

3. **tui crate**:
   ```rust
   #[tokio::main]
   async fn main() {
       let debugger = AsyncDebugger::connect(...).await?;
       // Event loop with select! on terminal events and debugger events
   }
   ```

4. **repl crate**: Wrap in tokio runtime

### Phase 4: Cleanup

1. Remove old transport crate dependency
2. Remove synchronous code paths
3. Update documentation
4. Remove feature flags

---

## API Changes

### Before (Sync)

```rust
use debugger::Debugger;

let debugger = Debugger::on_port(port, language, &config, stop_on_entry)?;
debugger.start()?;

// Event handling via channel
let events = debugger.events();
loop {
    match events.recv() {
        Ok(Event::Paused { .. }) => {
            debugger.continue_()?;
        }
        // ...
    }
}
```

### After (Async)

```rust
use debugger::AsyncDebugger;
use futures::StreamExt;

let debugger = AsyncDebugger::connect(port, language, &config, stop_on_entry).await?;
debugger.start().await?;

// Event handling via async stream
let mut events = debugger.events();
while let Some(event) = events.next().await {
    match event {
        Event::Paused { .. } => {
            debugger.continue_().await?;
        }
        // ...
    }
}
```

---

## Risk Mitigation

### Deadlock Prevention

When using `block_on` (Option B) or mixing sync/async:
- Never call `block_on` while holding a lock
- Use `tokio::sync::Mutex` instead of `std::sync::Mutex` for state held across await
- Document which methods are safe to call from sync context

### Testing Strategy

1. **Unit tests**: Use `#[tokio::test]` for async tests
2. **Integration tests**: Use `tokio::runtime::Runtime` to drive async code
3. **Mock transport**: Use `MemoryTransport` from transport2 for deterministic tests

### Backwards Compatibility

If Option B or C is chosen:
- Maintain sync API surface
- Deprecate sync API with clear migration path
- Provide migration guide in documentation

---

## Timeline Considerations

This plan focuses on implementation steps without time estimates. The work can be broken into independently testable chunks:

1. Infrastructure changes (can be merged independently)
2. AsyncDebugger implementation (parallel to existing Debugger)
3. Consumer updates (can be done per-crate)
4. Cleanup and deprecation (after all consumers migrated)

---

## Open Questions

1. **Runtime ownership**: Should each Debugger own its runtime, or should the caller provide one?
   - Recommendation: Caller provides runtime handle for flexibility

2. **Event buffering**: How many events to buffer before applying backpressure?
   - Recommendation: Unbounded initially, add backpressure if needed

3. **Cancellation**: How to handle task cancellation on debugger drop?
   - Recommendation: Use `tokio_util::sync::CancellationToken`

4. **Error handling**: Should transport errors terminate the session or be recoverable?
   - Recommendation: Emit error event, let caller decide

---

## References

- [transport2 DESIGN.md](../transport2/DESIGN.md) - Transport layer architecture
- [notes/startup_sequence.md](../../notes/startup_sequence.md) - DAP initialization sequence
- [tokio documentation](https://tokio.rs/tokio/tutorial) - Async runtime patterns
