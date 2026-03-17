## Context

Currently, the entire debug session lifecycle is wired into `DebuggerApp::new()` — launch configuration is loaded from CLI args, the server is spawned, the debugger connects, breakpoints are configured, and execution starts, all before the GUI event loop begins. If the program terminates, the user sees "Program terminated" with no way to restart. The `AsyncBridge` owns the `TcpAsyncDebugger` and tokio runtime for the lifetime of the app, and the server handle is held in `DebuggerApp._server`.

The file picker (`Ctrl+P`) and code view already let users browse files and set breakpoints — but there's no way to actually *run* anything from there.

## Goals / Non-Goals

**Goals:**
- Allow users to start a debug session from within the GUI (toolbar button + keyboard shortcut)
- Allow re-launching after a session terminates without restarting the app
- Let users select from available launch configurations in their `launch.json`
- Preserve breakpoints across session restarts
- Keep the existing CLI-driven launch path working (backwards compatible)

**Non-Goals:**
- Editing `launch.json` from within the GUI
- Supporting `${file}` placeholder resolution (future work — requires more UX thought)
- Hot-reloading launch configurations while a session is active
- Multi-session debugging (running two debug sessions simultaneously)
- Creating launch configurations from scratch in the GUI

## Decisions

### 1. Introduce a `SessionState` enum separate from `State`

**Decision:** Add a top-level session lifecycle (`NoSession` / `Active(State)`) that wraps the existing `State` enum, rather than adding new variants to `State`.

**Rationale:** The existing `State` enum models the *debugger* state (Initialising/Running/Paused/Terminated). A "no session yet" state is fundamentally different — there's no debugger, no bridge, no server. Mixing these concerns into one enum would require `Option<AsyncBridge>` everywhere. A wrapper keeps the active-session code paths unchanged.

**Alternative considered:** Adding `State::Idle` — rejected because it would require guarding every `self.bridge` access, and the renderer logic for "no session" is completely different from debugging states.

### 2. Extract session initialization into a reusable `Session` struct

**Decision:** Extract the initialization logic from `DebuggerApp::new()` into a `Session` struct that bundles the `AsyncBridge`, server handle, event thread, and config metadata. `DebuggerApp` holds an `Option<Session>` plus the shared UI state (breakpoints, file cache, state manager, etc.).

**Rationale:** This makes session teardown natural (drop the `Session`) and restart straightforward (create a new `Session`). The tokio runtime, server process, and event forwarding thread are all scoped to the session lifetime.

**Alternative considered:** Making `AsyncBridge` support reconnection — rejected as significantly more complex (would need to handle partial state in the command loop, reconnect the event stream, etc.).

### 3. Load launch configurations eagerly, select via combo box

**Decision:** Parse `launch.json` at app startup (or when the config path is provided) and store all configurations. Show a combo box / dropdown in the toolbar for selection. The "Start" button launches the selected configuration.

**Rationale:** Launch configurations are typically small (< 20 entries). Eager loading keeps the UI responsive. A combo box is the standard pattern (VS Code, IntelliJ) and requires minimal screen space.

**Alternative considered:** A modal picker like the file picker — overkill for typically < 10 configs, and the combo box is more discoverable.

### 4. Config path provided via CLI, not file dialog

**Decision:** The path to `launch.json` is still provided via CLI args. The GUI selects *which* configuration within it, but doesn't browse for the file itself.

**Rationale:** Keeps scope small. Users already launch the app from a project directory with a config path. Adding a file dialog for config selection is a separate feature.

### 5. Toolbar placement for launch controls

**Decision:** Add the configuration selector and start/restart button to the existing top control panel, to the left of the stepping controls. Stepping controls are disabled/hidden when no session is active.

**Rationale:** Keeps all execution controls in one place. The top panel already has Continue/Step buttons, so adding launch controls there is natural.

### 6. F5 as the keyboard shortcut for start/continue

**Decision:** Use `F5` to start a new session (when no session is active) or continue (when paused), matching VS Code conventions. `Shift+F5` to stop the current session.

**Rationale:** Familiar to most developers. Avoids conflicts with existing `Ctrl+P` (file picker).

## Risks / Trade-offs

**[Server port conflict on restart]** → If a session is stopped but the OS hasn't released the port yet, restarting may fail to bind. Mitigation: use a small retry with backoff, or pick a random available port for the server.

**[Tokio runtime per session]** → Creating a new tokio runtime per session has some overhead (~1ms). This is negligible for a GUI app where restarts are user-initiated. The alternative (sharing a runtime) would complicate lifecycle management.

**[State leakage between sessions]** → File cache, variables cache, and REPL history from the previous session could confuse users. Mitigation: clear `variables_cache` and `repl_output` on session start; keep `file_cache` (it's just file contents, still valid) and `ui_breakpoints` (intentionally preserved).
