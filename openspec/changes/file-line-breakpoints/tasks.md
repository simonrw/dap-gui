## 1. Breakpoint Parsing

- [x] 1.1 Add `Breakpoint::parse(input: &str, cwd: &Path) -> Result<Breakpoint>` to `crates/debugger/src/types.rs` — parse `file:line` format, resolve relative paths against `cwd`, pass absolute paths through, canonicalize when possible
- [x] 1.2 Add unit tests for `Breakpoint::parse`: relative path, absolute path, missing line number, non-numeric line, empty input, paths with special characters

## 2. Expose CWD to the GUI

- [x] 2.1 Ensure `debug_root_dir` (or the session's `cwd`) is accessible from the renderer/breakpoints panel context so it can be passed to `Breakpoint::parse`

## 3. Breakpoints Panel Text Input

- [x] 3.1 Add a text input field (`egui::TextEdit`) to the breakpoints panel in the GUI with placeholder text like `file:line`
- [x] 3.2 On Enter, call `Breakpoint::parse` with the input and current `cwd`, add the breakpoint to `ui_breakpoints`, and trigger DAP sync via `AsyncBridge::AddBreakpoint`
- [x] 3.3 Display error indication (e.g., red border or tooltip) when parsing fails
- [x] 3.4 Clear the input field on successful breakpoint addition

## 4. Verification

- [x] 4.1 Run `cargo check --all-targets --all-features` to ensure compilation
- [x] 4.2 Run `cargo xtask test && cargo xtask doctest` to ensure all tests pass
- [x] 4.3 Capture a screenshot with `bin/capture_screenshot` to verify the UI (requires running app instance)
