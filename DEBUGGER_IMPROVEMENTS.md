# Debugger Crate: Problems and Proposed Improvements

## Executive Summary

This document outlines problems identified in the `debugger` crate and proposes concrete improvements. Issues are categorized by severity and type.

## Critical Issues (High Priority)

### 1. **Panic in Drop Implementation**
**Location:** `crates/debugger/src/debugger.rs:448-456`

**Problem:**
```rust
impl Drop for Debugger {
    fn drop(&mut self) {
        tracing::debug!("dropping debugger");
        self.execute(requests::RequestBody::Disconnect(Disconnect {
            terminate_debugee: true,
        }))
        .unwrap();  // ⚠️ Can cause double panic if drop is called during unwinding
    }
}
```

**Impact:** If the debugger is dropped during panic unwinding, this `.unwrap()` will cause a double panic, immediately aborting the process.

**Proposed Fix:**
```rust
impl Drop for Debugger {
    fn drop(&mut self) {
        tracing::debug!("dropping debugger");
        if let Err(e) = self.execute(requests::RequestBody::Disconnect(Disconnect {
            terminate_debugee: true,
        })) {
            tracing::warn!(error = %e, "failed to disconnect debugger during drop");
        }
    }
}
```

---

### 2. **Unbounded Background Thread Never Terminates**
**Location:** `crates/debugger/src/debugger.rs:180-194`

**Problem:**
```rust
thread::spawn(move || {
    loop {
        let event = background_events.recv().unwrap();  // ⚠️ Panics on channel disconnect
        // ... handle event ...
    }
});
```

**Impact:**
- Thread will panic when the channel is closed instead of terminating gracefully
- Thread keeps running even after `Debugger` is dropped, causing resource leaks
- No way to join or stop the background thread

**Proposed Fix:**
```rust
thread::spawn(move || {
    loop {
        match background_events.recv() {
            Ok(event) => {
                let lock_id = Uuid::new_v4().to_string();
                let span = tracing::trace_span!("", %lock_id);
                let _guard = span.enter();

                tracing::trace!(is_poisoned = %background_internals.is_poisoned(),
                               "trying to unlock background internals");

                if let Ok(mut b) = background_internals.lock() {
                    tracing::trace!(?event, "handling event");
                    b.on_event(event);
                    drop(b);
                    tracing::trace!("locked background internals");
                } else {
                    tracing::error!("mutex poisoned, terminating background thread");
                    break;
                }
            }
            Err(_) => {
                tracing::debug!("event channel closed, terminating background thread");
                break;
            }
        }
    }
});
```

**Better Architecture:** Store `JoinHandle` in `Debugger` and join on drop:
```rust
pub struct Debugger {
    internals: Arc<Mutex<DebuggerInternals>>,
    rx: crossbeam_channel::Receiver<Event>,
    background_thread: Option<thread::JoinHandle<()>>,  // Add this
}
```

---

### 3. **Panic on Stack Frame Count Assumption**
**Location:** `crates/debugger/src/internals.rs:243-244`

**Problem:**
```rust
if stack_frames.len() != 1 {
    panic!("unexpected number of stack frames: {}", stack_frames.len());
}
```

**Impact:** Crashes the entire application if DAP server returns unexpected stack frame count.

**Proposed Fix:**
```rust
if stack_frames.is_empty() {
    tracing::error!("no stack frames received in stopped event");
    return;
}

if stack_frames.len() != 1 {
    tracing::warn!(
        count = stack_frames.len(),
        "unexpected number of stack frames, using first frame"
    );
}

let source = stack_frames[0].source.as_ref()
    .ok_or_else(|| {
        tracing::error!("stack frame has no source information");
    });
```

---

### 4. **Poisoned Mutex Causes Panic**
**Location:** `crates/debugger/src/debugger.rs:439`

**Problem:**
```rust
fn with_internals<F, T>(&self, f: F) -> eyre::Result<T>
where
    F: FnOnce(&mut DebuggerInternals) -> eyre::Result<T>,
{
    tracing::trace!(poisoned = %self.internals.is_poisoned(), "trying to lock internals");
    let mut internals = self.internals.lock().unwrap();  // ⚠️ Panics if mutex poisoned
    // ...
}
```

**Impact:** If any thread panics while holding the mutex, all subsequent operations will panic.

**Proposed Fix:**
```rust
fn with_internals<F, T>(&self, f: F) -> eyre::Result<T>
where
    F: FnOnce(&mut DebuggerInternals) -> eyre::Result<T>,
{
    tracing::trace!(poisoned = %self.internals.is_poisoned(), "trying to lock internals");
    let mut internals = self.internals.lock()
        .map_err(|e| eyre::eyre!("debugger mutex poisoned: {}", e))?;
    tracing::trace!("executing operation");
    let res = f(&mut internals);
    drop(internals);
    tracing::trace!("unlocked internals");
    res
}
```

---

### 5. **Home Directory Unwrap Can Panic**
**Location:** `crates/debugger/src/utils.rs:6`

**Problem:**
```rust
pub fn normalise_path(path: &Path) -> Cow<'_, Path> {
    if path.starts_with("~") {
        let stub: String = path.display().to_string().chars().skip(2).collect();
        Cow::Owned(dirs::home_dir().unwrap().join(stub))  // ⚠️ home_dir() can return None
    } else {
        Cow::Borrowed(path)
    }
}
```

**Impact:** Will panic on systems where home directory cannot be determined.

**Proposed Fix:**
```rust
pub fn normalise_path(path: &Path) -> Cow<'_, Path> {
    if let Some(stripped) = path.strip_prefix("~") {
        if let Some(home) = dirs::home_dir() {
            return Cow::Owned(home.join(stripped));
        }
        tracing::warn!("cannot determine home directory, using path as-is");
    }
    Cow::Borrowed(path)
}
```

---

## High Priority Issues

### 6. **Duplicate Stack Trace Requests**
**Location:** `crates/debugger/src/internals.rs:218-272`

**Problem:** In the `Stopped` event handler, we request stack traces twice:
- First with `levels: Some(1)` to get just the top frame (line 233)
- Then with full stack trace (line 265)

**Impact:** Wastes network round-trips and increases latency on every breakpoint hit.

**Proposed Fix:**
```rust
transport::events::Event::Stopped(transport::events::StoppedEventBody {
    thread_id,
    ..
}) => {
    self.current_thread_id = Some(thread_id);

    // Fetch full stack trace once
    let responses::Response {
        body: Some(responses::ResponseBody::StackTrace(
            responses::StackTraceResponse { stack_frames }
        )),
        success: true,
        ..
    } = self
        .client
        .send(requests::RequestBody::StackTrace(requests::StackTrace {
            thread_id,
            ..Default::default()
        }))
        .unwrap()
    else {
        unreachable!()
    };

    if stack_frames.is_empty() {
        tracing::error!("no stack frames received");
        return;
    }

    let top_frame = &stack_frames[0];
    let current_source = FileSource {
        line: top_frame.line,
        file_path: top_frame.source.as_ref().and_then(|s| s.path.clone()),
    };
    self.current_source = Some(current_source);

    let paused_frame = self
        .compute_paused_frame(top_frame)
        .expect("building paused frame construct");

    self.set_state(DebuggerState::Paused {
        stack: stack_frames,
        paused_frame: Box::new(paused_frame),
        breakpoints: self.breakpoints.values().cloned().collect(),
    });
}
```

---

### 7. **Hardcoded Event Wait Limit**
**Location:** `crates/debugger/src/debugger.rs:402-421`

**Problem:**
```rust
pub fn wait_for_event<F>(&self, pred: F) -> Event
where
    F: Fn(&Event) -> bool,
{
    let mut n = 0;
    loop {
        let evt = self.rx.recv().unwrap();  // ⚠️ Also panics on disconnect
        if n >= 100 {  // ⚠️ Arbitrary limit
            panic!("did not receive event");
        }
        // ...
    }
}
```

**Impact:**
- Arbitrary limit of 100 events
- Panics instead of returning an error
- Used in tests, making them fragile

**Proposed Fix:**
```rust
pub fn wait_for_event_timeout<F>(
    &self,
    pred: F,
    timeout: Duration
) -> eyre::Result<Event>
where
    F: Fn(&Event) -> bool,
{
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());

        match self.rx.recv_timeout(remaining) {
            Ok(evt) => {
                if pred(&evt) {
                    tracing::debug!(event = ?evt, "received expected event");
                    return Ok(evt);
                } else {
                    tracing::trace!(event = ?evt, "non-matching event");
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                return Err(eyre::eyre!("timeout waiting for event"));
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                return Err(eyre::eyre!("event channel disconnected"));
            }
        }
    }
}

// Keep old API for compatibility
pub fn wait_for_event<F>(&self, pred: F) -> Event
where
    F: Fn(&Event) -> bool,
{
    self.wait_for_event_timeout(pred, Duration::from_secs(30))
        .expect("waiting for event")
}
```

---

### 8. **Incomplete LLDB Implementation**
**Location:** `crates/debugger/src/debugger.rs:70-82`

**Problem:**
```rust
LaunchConfiguration::LLDB(lldb) => match lldb.request.as_str() {
    "launch" => {
        #[allow(unreachable_code)]
        InitialiseArguments::Launch(LaunchArguments {
            working_directory: None,
            language: crate::Language::DebugPy,  // ⚠️ Wrong language!
            program: todo!(),  // ⚠️ Unimplemented
        })
    }
    other => todo!("{other}"),
},
```

**Impact:** LLDB support is advertised but not functional.

**Proposed Fix:**
Either:
1. Remove LLDB from the match arms and return an error
2. Implement it properly
3. Add a compile-time feature flag

```rust
LaunchConfiguration::LLDB(_lldb) => {
    return Err(eyre::eyre!(
        "LLDB support is not yet implemented. Only DebugPy is currently supported."
    ));
}
```

---

### 9. **Missing Null Checks on Path Canonicalization**
**Location:** `crates/debugger/src/state.rs:137-143`

**Problem:**
```rust
pub fn to_request(self) -> requests::RequestBody {
    let program = self
        .program
        .canonicalize()
        .expect("launch target not a valid path");  // ⚠️ Panics instead of returning error
    // ...
}
```

**Impact:** Panic instead of proper error propagation.

**Proposed Fix:**
```rust
impl LaunchArguments {
    pub fn to_request(self) -> eyre::Result<requests::RequestBody> {
        let program = self
            .program
            .canonicalize()
            .wrap_err_with(|| format!("program path is not valid: {}", self.program.display()))?;

        let cwd = self
            .working_directory
            .unwrap_or_else(|| {
                program.parent()
                    .expect("program path has no parent directory")
                    .to_path_buf()
            });

        Ok(match self.language {
            Language::DebugPy => requests::RequestBody::Launch(requests::Launch {
                program,
                launch_arguments: Some(transport::requests::LaunchArguments::Debugpy(
                    DebugpyLaunchArguments {
                        just_my_code: true,
                        cwd,
                        show_return_value: true,
                        debug_options: vec![
                            "DebugStdLib".to_string(),
                            "ShowReturnValue".to_string(),
                        ],
                        stop_on_entry: false,
                        is_output_redirected: false,
                    },
                )),
            }),
            Language::Delve => {
                return Err(eyre::eyre!("Delve support is not yet implemented"));
            }
        })
    }
}
```

Then update call sites to handle the `Result`.

---

## Medium Priority Issues

### 10. **Breakpoints Method Unwraps Unnecessarily**
**Location:** `crates/debugger/src/debugger.rs:256-261`

**Problem:**
```rust
pub fn breakpoints(&self) -> Vec<types::Breakpoint> {
    self.with_internals(|internals| {
        Ok(internals.breakpoints.clone().values().cloned().collect())
    })
    .unwrap()  // ⚠️ Discards error information
}
```

**Impact:** API hides potential errors from users.

**Proposed Fix:**
```rust
pub fn breakpoints(&self) -> eyre::Result<Vec<types::Breakpoint>> {
    self.with_internals(|internals| {
        Ok(internals.breakpoints.values().cloned().collect())
    })
}
```

---

### 11. **Unreachable Code Instead of Error Handling**
**Location:** Multiple locations in `internals.rs`

**Problem:**
```rust
let responses::Response {
    body: Some(responses::ResponseBody::StackTrace(...)),
    success: true,
    ..
} = self.client.send(req).unwrap()
else {
    unreachable!()  // ⚠️ Assumes response type, but server could return anything
};
```

**Impact:** If DAP server sends unexpected response format, program crashes instead of handling gracefully.

**Proposed Fix:**
```rust
match self.client.send(req).context("requesting stack trace")? {
    responses::Response {
        body: Some(responses::ResponseBody::StackTrace(
            responses::StackTraceResponse { stack_frames }
        )),
        success: true,
        ..
    } => {
        // Handle success case
    }
    resp => {
        return Err(eyre::eyre!(
            "unexpected response to StackTrace request: success={}, body={:?}",
            resp.success,
            resp.body
        ));
    }
}
```

---

### 12. **Dead Code Not Removed**
**Location:** `crates/debugger/src/internals.rs:202-205`

**Problem:**
```rust
#[allow(dead_code)]
fn get_stack_frames(&self) -> eyre::Result<Vec<StackFrame>> {
    todo!()
}
```

**Impact:** Code clutter, maintenance burden.

**Proposed Fix:** Remove the dead code entirely.

---

### 13. **Redundant Clone Operations**
**Location:** `crates/debugger/src/state.rs:51`

**Problem:**
```rust
paused_frame: *paused_frame.clone(),  // ⚠️ Clone then deref - just deref!
```

**Impact:** Unnecessary heap allocation.

**Proposed Fix:**
```rust
paused_frame: **paused_frame,  // Or just *paused_frame if we change Box storage
```

---

### 14. **Test Code Has Logic Errors**
**Location:** `crates/debugger/tests/debugger.rs:115, 189`

**Problem:**
```rust
if file_path == file_path && breakpoint_line == breakpoint_line
    // ⚠️ Variables compared to themselves!
```

**Impact:** Test doesn't actually validate anything.

**Proposed Fix:**
```rust
// In test setup, give expected values distinct names:
let expected_file_path = file_path.clone();
let expected_line = breakpoint_line;

// In assertion:
if file_path == expected_file_path && breakpoint_line == expected_line
```

---

## Code Quality Issues (Lower Priority)

### 15. **TODO Items Should Be Addressed**

**Locations:**
- `debugger.rs:67, 77, 80` - Unhandled request types and LLDB implementation
- `state.rs:162` - Delve language support
- `internals.rs:204` - Unimplemented `get_stack_frames`
- `internals.rs:324` - "don't assume breakpoints are for the same file" (but this is already handled)

**Proposed Fix:** Create GitHub issues for each TODO and either implement or remove the code.

---

### 16. **Inconsistent Error Handling**

**Problem:** Mix of `.unwrap()`, `.expect()`, `?`, and panic across the codebase.

**Proposed Fix:** Establish and document error handling conventions:
- Use `?` for propagating errors in fallible functions
- Use `.unwrap()` only when proven impossible to fail (document why)
- Never use `.unwrap()` or `.expect()` in Drop implementations
- Convert panics to errors wherever possible

---

### 17. **Missing Source Validation**
**Location:** `crates/debugger/src/internals.rs:247-254`

**Problem:**
```rust
let source = stack_frames[0].source.as_ref().unwrap();  // ⚠️ Assumes source exists
let line = stack_frames[0].line;
```

**Impact:** Panics if debug adapter doesn't include source information.

**Proposed Fix:**
```rust
let source = stack_frames[0].source.as_ref()
    .ok_or_else(|| eyre::eyre!("stack frame has no source information"))?;
let line = stack_frames[0].line;

let current_source = FileSource {
    line,
    file_path: source.path.clone(),
};
```

---

### 18. **Path Normalization Bug**
**Location:** `crates/debugger/src/utils.rs:3-10`

**Problem:**
```rust
if path.starts_with("~") {
    let stub: String = path.display().to_string().chars().skip(2).collect();
    // ⚠️ skip(2) assumes "~/" but check is just "~"
}
```

**Impact:** Path `~file` would become `ile` after normalization.

**Proposed Fix:** Already included in fix #5 above.

---

## Architectural Improvements

### 19. **Add Graceful Shutdown Mechanism**

**Problem:** No way to gracefully stop the debugger and clean up resources.

**Proposed Solution:**
```rust
pub struct Debugger {
    internals: Arc<Mutex<DebuggerInternals>>,
    rx: crossbeam_channel::Receiver<Event>,
    background_thread: Option<thread::JoinHandle<()>>,
    shutdown_tx: crossbeam_channel::Sender<()>,  // Add shutdown signal
}

impl Debugger {
    pub fn shutdown(mut self) -> eyre::Result<()> {
        // Signal background thread to stop
        let _ = self.shutdown_tx.send(());

        // Send disconnect
        self.execute(requests::RequestBody::Disconnect(Disconnect {
            terminate_debugee: true,
        }))?;

        // Wait for background thread
        if let Some(handle) = self.background_thread.take() {
            handle.join().map_err(|_| eyre::eyre!("background thread panicked"))?;
        }

        Ok(())
    }
}
```

---

### 20. **Add Request Timeout Configuration**

**Problem:** No way to configure timeouts for DAP requests.

**Proposed Solution:** Add timeout configuration to `Debugger::new()` and pass to transport layer.

---

## Summary

**Critical Issues:** 5 (must fix - cause crashes/panics)
**High Priority:** 9 (should fix - correctness and robustness)
**Medium Priority:** 5 (nice to fix - API quality)
**Code Quality:** 6 (cleanup and consistency)
**Architectural:** 2 (future improvements)

**Total Issues Identified:** 27

## Recommended Fix Order

1. Fix critical panics in Drop and background thread (Issues #1, #2)
2. Fix panic on unexpected DAP responses (Issues #3, #5, #17)
3. Fix mutex poisoning and error handling (Issue #4)
4. Fix performance issue with duplicate requests (Issue #6)
5. Address incomplete implementations (Issues #8, #9, #15)
6. Clean up error handling patterns (Issues #10, #11, #16)
7. Remove dead code and fix minor bugs (Issues #12, #13, #14, #18)
8. Consider architectural improvements (Issues #19, #20)
