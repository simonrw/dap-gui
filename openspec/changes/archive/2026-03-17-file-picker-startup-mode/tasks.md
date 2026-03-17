## 1. CLI & App Mode

- [x] 1.1 Add `AppMode` enum (`FilePicker` / `Debugger`) to the gui crate
- [x] 1.2 Make `config_path` an optional positional argument in `Args`
- [x] 1.3 Update `main.rs` startup logic: if no config path and no breakpoints, create `AppMode::FilePicker` with CWD as project root; otherwise proceed with existing debugger flow

## 2. File Picker Sidebar

- [x] 2.1 Extract file picker rendering into a reusable sidebar variant that can be shown inline (persistent left panel) rather than only as a Ctrl+P overlay
- [x] 2.2 In file-picker mode, render the sidebar file picker on startup with search input auto-focused
- [x] 2.3 Wire file selection to set the displayed file in the code viewer

## 3. Code Viewer Without Debugger

- [x] 3.1 Ensure `CodeView` can render without an active debugger session (no current frame, no AsyncBridge) — use the existing `file_override` path
- [x] 3.2 Enable gutter-click breakpoint toggling that writes to `ui_breakpoints` without sending commands to a debugger
- [x] 3.3 Display persisted breakpoints in the gutter when viewing files in file-picker mode

## 4. Minimal Renderer

- [x] 4.1 Add a `render_file_picker_mode` method to `Renderer` that only renders the file picker sidebar and central code viewer — no control panel, call stack, variables, REPL, or status bar
- [x] 4.2 Dispatch to the correct render method based on `AppMode` in the main `update()` loop

## 5. State Persistence

- [x] 5.1 On exit from file-picker mode, persist `ui_breakpoints` to the state file
- [x] 5.2 On startup in file-picker mode, load persisted breakpoints from the state file and display them in the gutter

## 6. Verification

- [x] 6.1 Ensure `cargo check --all-targets --all-features` passes
- [x] 6.2 Ensure `cargo xtask test && cargo xtask doctest` passes
- [x] 6.3 Capture a screenshot of the file-picker startup mode to verify the UX
