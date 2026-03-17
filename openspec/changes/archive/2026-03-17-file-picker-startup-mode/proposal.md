## Why

The GUI currently requires a debug session to be useful — the user must provide a launch configuration and optionally `-b/--breakpoints` to set initial breakpoints. There is no way to browse project files and set breakpoints *before* connecting to a debugger. When the user omits `-b/--breakpoints`, the app should start in a lightweight file-picker mode where they can explore source files and mark breakpoints, providing a more natural pre-debug workflow.

## What Changes

- When `-b/--breakpoints` is not provided, the GUI starts in a **file-picker startup mode** instead of immediately connecting to a debugger.
- In this mode, only the file picker and source code viewer are shown — no control panel, call stack, variables, REPL, or status bar.
- The user can browse project files using the existing file picker (Ctrl+P or shown by default) and view source code with syntax highlighting.
- The user can click the gutter to toggle breakpoints on files they're viewing.
- Once the user is ready, they can proceed to the normal debug session with their selected breakpoints.
- The app should allow browsing files based on the `cwd` specified in the launch configuration

## Capabilities

### New Capabilities
- `file-picker-startup`: A startup mode that shows only the file picker and code viewer, allowing users to browse files and set breakpoints before a debug session begins.

### Modified Capabilities
<!-- No existing specs to modify -->

## Impact

- **`crates/gui/src/main.rs`**: Conditional startup logic based on whether `-b/--breakpoints` was provided.
- **`crates/gui/src/renderer.rs`**: New render path for the minimal file-picker-only UI.
- **`crates/gui/src/code_view.rs`**: Must work independently of an active debug session (no current frame, no debugger connection).
- **`crates/gui/src/ui/file_picker.rs`**: May need to be shown inline/prominently rather than as an overlay, since it's the primary navigation in this mode.
- No new external dependencies expected.
