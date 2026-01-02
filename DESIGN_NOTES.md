# Design Notes for PR 2

## Current Problem

The debugger spawns two threads:
1. **Polling thread**: Owns `reader`, polls `transport.poll_message()`, forwards to channels
2. **Background thread**: Receives from channels, processes events and commands

This is redundant - one thread should be enough.

## Proposed Solution

Consolidate into a single background thread that:
1. Owns the `reader` directly
2. Polls `reader.poll_message()` with timeout
3. Processes events immediately
4. Handles commands
5. Tracks pending requests for responses

## Implementation Challenges

### Challenge 1: `internals.send()` currently blocks on `message_rx`

Current flow:
```
polling thread: reader.poll_message() -> message_rx
internals.send(): write request -> wait on message_rx for response
```

New flow:
```
background thread: reader.poll_message() -> process directly
SendRequest command: write request, track pending, continue polling
When response arrives: match to pending, send to response_tx
```

### Challenge 2: Need to interleave polling and command handling

Solution: Use non-blocking `try_recv()` on command channel, poll transport with short timeout

```rust
loop {
    // Poll transport (short timeout: 10ms)
    match reader.try_receive_message(Duration::from_millis(10))? {
        Some(Message::Event(event)) => process_event(event),
        Some(Message::Response(response)) => match_pending_response(response),
        None => {} // Timeout, continue
    }

    // Check for commands (non-blocking)
    match command_rx.try_recv() {
        Ok(command) => process_command(command),
        Err(TryRecvError::Empty) => {} // No command, continue
        Err(TryRecvError::Disconnected) => break,
    }

    // Process follow-ups
    ...
}
```

### Challenge 3: Refactoring `internals.send()`

Option A: Make `send()` non-blocking, return seq number
Option B: Remove `send()` from internals, handle in background thread directly
Option C: Keep `send()` but have it work differently (add to pending map)

Going with Option C for minimal changes.

## Implementation Plan

1. Add `PendingRequests` struct to track pending requests
2. Modify `DebuggerInternals` to not need `message_rx`
3. Rewrite background thread to own reader
4. Remove polling thread
5. Test thoroughly
