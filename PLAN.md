# TUI Headless Testing Framework

## Context

The TUI debugger (`crates/tui-poc`) currently has no tests for its UI logic, rendering, or input handling. Only the `fuzzy.rs` module has tests. The `App` struct is tightly coupled to `AsyncBridge` (which requires a real debugger connection over TCP) and to crossterm's global `event::read()`, making it impossible to test in isolation.

The goal is to build a headless testing framework that can:
1. Construct `App` without a real debugger
2. Simulate keyboard input and observe state changes
3. Render to an in-memory buffer and capture output
4. Use `insta` snapshot testing for regression detection

## Approach

Use ratatui's built-in `TestBackend` for headless rendering, mock `AsyncBridge` via raw channels (no TCP/tokio tasks), and `insta` for snapshot testing. All test infrastructure lives behind `#[cfg(test)]` so there is zero impact on production code, aside from one small refactor to extract `handle_key_press()`.

## Implementation Steps

### Step 1: Add dev-dependencies

**File:** `crates/tui-poc/Cargo.toml`

```toml
[dev-dependencies]
insta = "1.41"
tempfile = "3"
```

### Step 2: Add `AsyncBridge::mock()` constructor

**File:** `crates/tui-poc/src/async_bridge.rs`

Add a `#[cfg(test)]` impl block with a `mock()` method that creates an `AsyncBridge` from raw channels — no TCP, no debugger connection. Returns the bridge plus both channel endpoints so tests can inject `StateUpdate`s and observe `UiCommand`s:

```rust
#[cfg(test)]
impl AsyncBridge {
    pub fn mock() -> (
        Self,
        mpsc::UnboundedSender<StateUpdate>,
        mpsc::UnboundedReceiver<UiCommand>,
    ) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime");
        (Self { runtime, command_tx, update_rx }, update_tx, command_rx)
    }
}
```

### Step 3: Extract `handle_key_press()` from `handle_events()`

**File:** `crates/tui-poc/src/main.rs`

Small production refactor: extract the key dispatch logic from `handle_events()` into a new `handle_key_press(&mut self, code: KeyCode, modifiers: KeyModifiers)` method. Then `handle_events()` becomes a thin wrapper:

```rust
fn handle_events(&mut self) -> io::Result<()> {
    if let Event::Key(key) = event::read()? {
        if key.kind != KeyEventKind::Press { return Ok(()); }
        self.handle_key_press(key.code, key.modifiers);
    }
    Ok(())
}

fn handle_key_press(&mut self, code: KeyCode, modifiers: KeyModifiers) {
    // ... existing dispatch logic moved here ...
}
```

This is the only production code change. It's behavior-preserving and makes all input handling directly testable without crossterm.

### Step 4: Add `#[cfg(test)]` accessor methods on `App`

**File:** `crates/tui-poc/src/main.rs`

Add read-only test accessors for private fields (`focus()`, `debug_state()`, `code_cursor_line()`, `breakpoints()`, `state_output()`, `is_exited()`, etc.) and a `load_synthetic_file()` method that injects file content into `file_cache` without disk I/O.

### Step 5: Create the `TestHarness`

**File:** `crates/tui-poc/src/test_harness.rs` (new, `#[cfg(test)]` gated)

```rust
pub struct TestHarness {
    pub app: App,
    pub terminal: Terminal<TestBackend>,
    pub update_tx: mpsc::UnboundedSender<StateUpdate>,
    pub command_rx: mpsc::UnboundedReceiver<UiCommand>,
}
```

Key methods:
- `new()` / `with_size(w, h)` — create harness with 80x24 terminal
- `press_key(KeyCode)` — simulate key press (calls `handle_key_press`)
- `press_key_with(KeyCode, KeyModifiers)` — key press with modifiers
- `inject_update(StateUpdate)` — inject debugger event, drain channel
- `render() -> String` — draw to TestBackend, extract buffer as text
- `expect_command() -> Option<UiCommand>` — read command the app sent
- `load_file(path, content)` — inject synthetic file into cache

Plus a `buffer_to_plain_text()` helper that walks the ratatui `Buffer` cell-by-cell to produce a plain text string (one line per terminal row, trailing spaces trimmed).

### Step 6: Write tests

**File:** `crates/tui-poc/src/main.rs` (at bottom, `#[cfg(test)] mod tests`)

Organized into categories:

| Category | What it tests | Example |
|----------|--------------|---------|
| Panel navigation | Tab/BackTab cycling, Esc behavior | `tab_cycles_through_panels` |
| Code window nav | j/k/g/G/H/L/{/} movement | `code_window_j_k_navigation` |
| Quit behavior | q exits except in BottomPanel | `q_does_not_exit_in_bottom_panel` |
| Debugger commands | F7/F8/F9 send correct UiCommand | `f9_sends_continue_command` |
| State updates | Injected events update app state | `paused_event_updates_debug_state` |
| Breakpoint mgmt | b toggles breakpoints, a adds | `toggle_breakpoint_in_code_window` |
| Rendering snapshots | insta snapshots of rendered output | `snapshot_empty_app`, `snapshot_with_source_file` |
| User flows | Multi-step interaction sequences | `navigate_and_set_breakpoint_flow` |

### Step 7: Generate and review snapshots

Run `cargo insta test` to generate initial `.snap` files in `crates/tui-poc/src/snapshots/`, then `cargo insta review` to accept them.

## Files to modify

- `crates/tui-poc/Cargo.toml` — add dev-dependencies
- `crates/tui-poc/src/async_bridge.rs` — add `#[cfg(test)] AsyncBridge::mock()`
- `crates/tui-poc/src/main.rs` — extract `handle_key_press()`, add `#[cfg(test)]` accessors and test module

## Files to create

- `crates/tui-poc/src/test_harness.rs` — `TestHarness` struct and helpers
- `crates/tui-poc/src/snapshots/` — auto-generated by insta

## Verification

1. `cargo check --all-targets --all-features` compiles
2. `cargo xtask test && cargo xtask doctest` passes (all existing + new tests)
3. `cargo insta test -p tui-poc` runs snapshot tests
4. `cargo insta review` to inspect snapshot files
