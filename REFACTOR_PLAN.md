# Refactoring Plan: Simplify Message Receiving Architecture

## Current Architecture

### Background Threads

The current implementation has multiple background threads handling message receiving:

1. **Transport Layer (`Client` type)**:
   - Location: `crates/transport/src/client.rs:97-140`
   - Spawns a background polling thread
   - Polls messages from the reader
   - Routes events to event channel
   - Routes responses to waiting requests via `RequestStore`
   - Uses `Arc<Mutex<ClientInternals>>` for shared state

2. **Debugger Layer - Polling Thread**:
   - Location: `crates/debugger/src/debugger.rs:166-199` (Launch) and `224-257` (Attach)
   - Spawned when using `TransportConnection`
   - Polls `reader.poll_message()` in a loop
   - Forwards events to event channel
   - Forwards ALL messages to message channel

3. **Debugger Layer - Background Thread**:
   - Location: `crates/debugger/src/debugger.rs:280-404`
   - Listens on two channels:
     - Events from polling thread
     - Commands from main thread
   - Processes events using `on_event_nonblocking()`
   - Handles follow-up requests (e.g., StackTrace -> Scopes -> Variables)
   - Processes commands (SendRequest, SendExecute)

### Problems with Current Architecture

1. **Redundant Threading**: Both transport and debugger spawn background threads for message receiving
2. **Unclear Responsibilities**: Transport layer should only handle serialization/deserialization, not threading
3. **Inefficiency**: Multiple threads doing similar work (polling messages)
4. **Complexity**: Hard to reason about message flow through multiple threads and channels
5. **Migration Debt**: Code has both old `Client` and new `TransportConnection`, showing incomplete migration

## Proposed Architecture

### Transport Layer Responsibilities

The transport crate should be **purely** about:
- Message serialization/deserialization
- Connection abstraction (TCP, in-memory, etc.)
- Synchronous send/receive operations
- NO background threads
- NO event routing logic

**Keep**:
- `TransportConnection` type (already has the right design)
- `SyncTransport` trait
- `DapTransport` trait for connection abstraction
- Message types (Event, Response, Request)
- Reader/Writer implementations

**Remove/Deprecate**:
- `Client` type with its background thread
- `RequestStore` (move responsibility to debugger layer)
- Event channel routing

**Transport API**:
```rust
pub trait SyncTransport {
    // Blocking read of next message
    fn receive_message(&mut self) -> Result<Option<Message>>;

    // Send request, return sequence number
    fn send_request(&mut self, body: RequestBody) -> Result<Seq>;

    // Send fire-and-forget
    fn send_execute(&mut self, body: RequestBody) -> Result<()>;
}

pub struct TransportConnection {
    // Already implements SyncTransport correctly
    // No background threads
}

impl TransportConnection {
    // Connection factories
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self>;
    pub fn with_transport<T: DapTransport>(transport: T) -> Result<Self>;
}
```

### Debugger Layer Responsibilities

The debugger crate should handle:
- ALL message polling
- Event processing and state machine transitions
- Request/response matching
- Command handling from main thread
- Background thread management (if needed)

**Single Background Thread Architecture**:

```rust
pub struct Debugger {
    internals: Arc<Mutex<DebuggerInternals>>,
    rx: crossbeam_channel::Receiver<Event>,
    command_tx: crossbeam_channel::Sender<Command>,
}

// One background thread that:
// 1. Polls transport.receive_message()
// 2. Routes messages:
//    - Events -> process with on_event_nonblocking()
//    - Responses -> match with pending requests
// 3. Listens for commands from main thread
// 4. Processes follow-up requests
```

**Message Flow**:
```
                    ┌─────────────────────────────────────┐
                    │      Main Thread (GUI/TUI)          │
                    └─────────┬───────────────────────────┘
                              │ Commands
                              │ (send_request, send_execute)
                              ▼
┌─────────────────────────────────────────────────────────┐
│              Debugger Background Thread                 │
│                                                          │
│  ┌────────────────────────────────────────────────┐    │
│  │ Event Loop (crossbeam_channel::select!)        │    │
│  │                                                 │    │
│  │  1. recv(command_rx)                           │    │
│  │     -> Process command (call transport)        │    │
│  │     -> Track pending request                   │    │
│  │                                                 │    │
│  │  2. Poll transport (non-blocking)              │    │
│  │     -> receive_message() with timeout          │    │
│  │     -> Match responses to pending requests     │    │
│  │     -> Process events with on_event_nonblocking│    │
│  │     -> Queue follow-up requests                │    │
│  │                                                 │    │
│  │  3. Process follow-up queue                    │    │
│  │                                                 │    │
│  └────────────────────────────────────────────────┘    │
│                                                          │
│  Uses:                                                   │
│  - TransportConnection (no threads)                     │
│  - PendingRequests (like old RequestStore)              │
│  - FollowUpQueue                                         │
└──────────────────────────────────────────────────────────┘
                              │
                              │ Events
                              │ (state changes)
                              ▼
                    ┌─────────────────────────────────────┐
                    │         Event Channel Rx            │
                    │    (GUI/TUI subscribes here)        │
                    └─────────────────────────────────────┘
```

## Implementation Plan

### Phase 1: Consolidate to Single Background Thread in Debugger

**Goal**: Remove the polling thread, keep only the background thread

**Changes**:
1. Modify `Debugger::on_port()` to NOT spawn polling thread
2. Update background thread to:
   - Own the `TransportConnection` (move into thread)
   - Add transport polling to the `crossbeam_channel::select!` loop
   - Use `recv_timeout()` or similar to poll transport without blocking forever

**Files to modify**:
- `crates/debugger/src/debugger.rs`
- `crates/debugger/src/internals.rs`

**Approach**:
```rust
// In debugger.rs
thread::spawn(move || {
    let mut transport = connection; // Move transport into thread
    let mut pending_requests = HashMap::new();
    let mut follow_up_queue = Vec::new();

    loop {
        // Poll transport with short timeout
        if let Ok(Some(message)) = transport.receive_message() {
            match message {
                Message::Event(event) => {
                    // Process event
                    let follow_ups = internals.on_event_nonblocking(event);
                    follow_up_queue.extend(follow_ups);
                }
                Message::Response(response) => {
                    // Match to pending request
                    if let Some(tx) = pending_requests.remove(&response.request_seq) {
                        let _ = tx.send(response);
                    }
                }
                _ => {}
            }
        }

        // Check for commands (non-blocking)
        if let Ok(command) = command_rx.try_recv() {
            match command {
                Command::SendRequest { body, response_tx } => {
                    let seq = transport.send_request(body)?;
                    pending_requests.insert(seq, response_tx);
                }
                Command::SendExecute { body, response_tx } => {
                    transport.send_execute(body)?;
                    let _ = response_tx.send(Ok(()));
                }
                Command::Shutdown => break,
            }
        }

        // Process follow-ups
        while let Some(follow_up) = follow_up_queue.pop() {
            let body = follow_up.to_request_body();
            let seq = transport.send_request(body)?;
            // ... handle response in next iteration
        }
    }
});
```

**Challenges**:
- `transport.receive_message()` is currently blocking
- Need timeout or non-blocking variant
- May need to use `select!` with a timeout channel

**Alternative**: Use `crossbeam_channel::select!` with an auxiliary thread that reads from transport and sends to a channel. But this re-introduces threading, so prefer making `receive_message()` timeout-based.

### Phase 2: Add Timeout Support to TransportConnection

**Goal**: Make transport polling non-blocking with timeout

**Changes**:
1. Add timeout parameter to `receive_message()`
2. OR: Add `try_receive_message()` variant
3. OR: Make reader configurable with `set_read_timeout()`

**Files to modify**:
- `crates/transport/src/client.rs`
- `crates/transport/src/reader.rs`

**Approach**:
```rust
impl SyncTransport for TransportConnection {
    fn receive_message(&mut self) -> Result<Option<Message>> {
        // Current implementation
    }

    fn try_receive_message(&mut self, timeout: Duration) -> Result<Option<Message>> {
        // Set socket read timeout
        // Call poll_message()
        // Handle WouldBlock / TimedOut as None
    }
}
```

Or update reader to support timeouts internally.

### Phase 3: Remove Old Client Type

**Goal**: Clean up deprecated code

**Changes**:
1. Mark `Client` as deprecated
2. Update any remaining usages to use `TransportConnection`
3. Eventually remove `Client` entirely

**Files to modify**:
- `crates/transport/src/client.rs`
- Any tests using `Client`

### Phase 4: Simplify DebuggerInternals

**Goal**: Remove `message_rx` channel, use transport directly

**Changes**:
1. Remove `message_rx` field from `DebuggerInternals`
2. Remove `send()` method's blocking loop over `message_rx`
3. Move request/response matching to background thread
4. Make `DebuggerInternals` methods return immediately after sending

**Files to modify**:
- `crates/debugger/src/internals.rs`

**Current issue**:
The `send()` method in `DebuggerInternals` currently:
1. Sends request to transport
2. Blocks waiting on `message_rx` for the response

This won't work if the background thread owns the transport. Instead:
- Background thread should own transport
- Background thread handles request/response matching
- `send()` should be a command sent to background thread

This is already partially implemented via the `Command` enum.

**Action**: Fully migrate to command-based architecture:
- Remove `send()` and `execute()` from `DebuggerInternals`
- OR: Move them to separate layer
- Keep only state manipulation in `DebuggerInternals`

## Benefits of New Architecture

1. **Clearer Separation of Concerns**:
   - Transport: serialization/deserialization only
   - Debugger: message handling and state management

2. **Fewer Threads**:
   - From 2-3 threads to 1 thread
   - Easier to reason about

3. **Simpler Message Flow**:
   - One place where messages are received
   - One place where events are processed
   - One place where responses are matched

4. **Better Foundation for Async**:
   - Transport layer already has sync API
   - Easy to make async later
   - Single event loop in debugger

5. **Reduced Complexity**:
   - No RequestStore in transport
   - No multiple channels for same data
   - Single source of truth

## Migration Strategy

1. Create feature flag `new_message_handling` (optional)
2. Implement Phase 1 behind flag (or directly on branch)
3. Update tests to pass
4. Implement Phase 2
5. Run all tests
6. Implement Phase 3 (deprecation)
7. Implement Phase 4 (cleanup)
8. Remove feature flag / merge to main

## Risks and Mitigations

**Risk**: Breaking existing functionality
- **Mitigation**: Comprehensive testing at each phase

**Risk**: Performance regression
- **Mitigation**: Benchmark message throughput before/after

**Risk**: Deadlocks with single thread
- **Mitigation**: Careful lock ordering, minimize lock hold time

**Risk**: Timeout tuning difficulty
- **Mitigation**: Make timeout configurable, use conservative default

## Testing Strategy

1. **Unit Tests**:
   - Test `TransportConnection` message send/receive
   - Test debugger event processing
   - Test command handling

2. **Integration Tests**:
   - Test full debugging sessions (already exist)
   - Test with debugpy and delve
   - Test breakpoint handling
   - Test step/continue operations

3. **Property Tests** (future):
   - All requests get responses
   - Events are not lost
   - State transitions are valid

## Files to Modify

### Transport Crate
- `crates/transport/src/client.rs` - Add timeouts, deprecate `Client`
- `crates/transport/src/reader.rs` - Support timeout-based reading
- `crates/transport/src/lib.rs` - Update exports

### Debugger Crate
- `crates/debugger/src/debugger.rs` - Consolidate to single background thread
- `crates/debugger/src/internals.rs` - Simplify send/execute methods
- `crates/debugger/src/commands.rs` - May need updates for new flow

### Tests
- Update any tests that rely on old threading model
- Ensure integration tests still pass

## Open Questions

1. Should we use `crossbeam_channel::select!` with timeout, or poll transport directly?
2. What timeout value is appropriate for transport polling?
3. Should follow-up requests use the same pending request tracking?
4. Do we need backpressure handling if events come faster than we can process?

## Alternatives Considered

### Alternative 1: Keep Transport Background Thread
- Pro: Transport layer handles threading
- Con: Doesn't solve the multiple thread problem
- Con: Transport responsibilities still unclear

### Alternative 2: No Background Threads at All
- Pro: Fully synchronous, easy to understand
- Con: Requires GUI/TUI to drive event loop
- Con: Harder to integrate with existing UI frameworks

### Alternative 3: Async/Await
- Pro: Modern, efficient
- Con: Large refactor
- Con: Requires async runtime
- Decision: Save for future work, but design for it

## Conclusion

The proposed refactoring simplifies the architecture by:
- Making transport purely about serialization/deserialization
- Consolidating message handling in a single debugger background thread
- Removing redundant threading and channels
- Creating a clearer foundation for future async migration

The implementation can be done incrementally with testing at each phase.
