## 1. Refactor App State for Session Lifecycle

- [x] 1.1 Create a `Session` struct that bundles `AsyncBridge`, server handle (`Option<Box<dyn Server + Send>>`), event thread join handle, and tokio runtime — extracted from what `DebuggerApp` currently holds
- [x] 1.2 Change `DebuggerApp` to hold `Option<Session>` instead of inlining the bridge and server handle. Move `DebuggerAppState` fields that are session-independent (breakpoints, file cache, state manager, debug root dir, file picker state, config path, launch configurations list) to persist across sessions
- [x] 1.3 Replace `State` usage in `DebuggerAppState` with a `SessionState` enum: `NoSession` or `Active { state: State, session: Session }`. Update `handle_event` and the `From<Event>` impl accordingly
- [x] 1.4 Extract the initialization logic from `DebuggerApp::new()` (lines ~253-404: config parsing, server spawn, debugger connect, breakpoint config, bridge spawn) into a `Session::start(config: &LaunchConfiguration, breakpoints: &[Breakpoint], ...) -> Result<Session>` method

## 2. Launch Configuration Loading

- [x] 2.1 Change `Args` to make `name` optional and parse all configurations from `launch.json` at startup, storing `Vec<LaunchConfiguration>` (with names) in `DebuggerAppState`
- [x] 2.2 Add a `selected_config_index: usize` field to `DebuggerAppState` to track which configuration is selected in the dropdown
- [x] 2.3 If `--name` is provided, pre-select the matching configuration index. If no match is found, show an error in the status bar rather than exiting the process

## 3. Toolbar UI — Configuration Selector and Launch Controls

- [x] 3.1 Add a `ComboBox` to `render_controls_window` (before the stepping buttons) that shows configuration names and updates `selected_config_index`
- [x] 3.2 Add a "Start" button that calls `Session::start()` with the selected configuration and current breakpoints, storing the result in `DebuggerAppState`
- [x] 3.3 Add a "Stop" button (enabled only when a session is active) that drops the current `Session` and transitions to `NoSession`
- [x] 3.4 Conditionally show/hide stepping controls (Continue, Step Over, Step Into, Step Out) based on whether a session is active
- [x] 3.5 Disable the configuration combo box while a session is active

## 4. Keyboard Shortcuts

- [x] 4.1 Add F5 handler in `render_ui`: if no session → start session; if paused → send Continue; if running → no-op
- [x] 4.2 Add Shift+F5 handler: if session is active → stop session (drop `Session`, transition to `NoSession`)

## 5. No-Session Welcome View

- [x] 5.1 Add a `render_no_session` method to `Renderer` that shows the configuration selector prominently in the central panel with a "Start Debugging" button and F5 hint
- [x] 5.2 Wire `render_ui` to call `render_no_session` when `SessionState::NoSession`, and the existing `render_paused_or_running_ui` / state-specific views when `SessionState::Active`

## 6. Session Restart and Cleanup

- [x] 6.1 On session start, clear `variables_cache` and `repl_output`. Preserve `ui_breakpoints`, `file_cache`, and `file_override`
- [x] 6.2 On session termination (State::Terminated), show a "Restart" button alongside the "Program terminated" message that starts a new session with the same configuration
- [x] 6.3 Ensure dropping `Session` cleanly shuts down the tokio runtime, kills the server process, and stops the event forwarding thread

## 7. Backwards Compatibility

- [x] 7.1 When `--name` matches exactly one config, auto-start the session on app launch (preserving current CLI behavior)
- [x] 7.2 Ensure existing CLI args (`--breakpoints`) still work and are applied to the auto-started session

## 8. Testing and Verification

- [x] 8.1 Verify `cargo check --all-targets --all-features` passes
- [x] 8.2 Run `cargo xtask test && cargo xtask doctest` and fix any failures
- [x] 8.3 Capture a screenshot showing the no-session view with configuration selector
- [x] 8.4 Capture a screenshot showing an active debug session with the new toolbar layout
