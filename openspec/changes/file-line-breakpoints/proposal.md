## Why

The GUI currently only supports setting breakpoints by clicking the gutter in an already-open source file. In the remote attach workflow, the debuggee is already running and the user needs to set breakpoints by specifying `file:line` pairs before any source file is loaded in the viewer. Without this, users cannot effectively use dap-gui for remote debugging or attach scenarios.

## What Changes

- Add a text input in the GUI where users can type a breakpoint location as `<file>:<line>` (e.g. `main.py:42` or `/abs/path/src/app.py:10`)
- Resolve relative file paths against the launch configuration's `cwd` (falling back to the process `pwd` if no `cwd` is configured)
- Absolute paths are used as-is
- The existing `Breakpoint` struct already supports `path` + `line`, so no data model changes are needed
- Breakpoints added via text input behave identically to gutter-click breakpoints (persistence, DAP sync, display in breakpoint list)

## Capabilities

### New Capabilities
- `text-breakpoint-input`: A text-based input widget for setting breakpoints via `file:line` strings, with path resolution against the debug session's working directory

### Modified Capabilities
<!-- No existing specs to modify -->

## Impact

- **GUI crate**: New input widget in the breakpoints panel or toolbar
- **Debugger crate**: May need a helper to resolve relative paths against a known `cwd`
- **State crate**: No changes needed — breakpoints already persist with full paths
- **Launch configuration**: `cwd` is already parsed; just needs to be accessible for path resolution
