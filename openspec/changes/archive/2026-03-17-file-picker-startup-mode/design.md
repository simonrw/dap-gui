## Context

The GUI app currently requires a full debug session to be useful. On startup, it parses CLI args (config path, optional name, optional breakpoints), spawns a debug server or attaches to one, and renders the full debugger UI. When users don't know which breakpoints they need upfront, they have no way to browse files and set breakpoints before launching.

The app already has the building blocks: a file picker with fuzzy search (`Ctrl+P`), a syntax-highlighted code viewer with gutter-click breakpoint toggling, and persistent breakpoint state. The goal is to compose these into a standalone pre-debug mode.

## Goals / Non-Goals

**Goals:**
- Provide a lightweight startup mode when `-b/--breakpoints` is omitted, showing only the file picker and code viewer.
- Allow users to browse project files and toggle breakpoints via gutter clicks before a debug session starts.
- Reuse existing file picker and code view components with minimal modification.

**Non-Goals:**
- Launching a debug session from within the file-picker mode (future work).
- Modifying the file picker's search/matching behavior.
- Supporting breakpoint conditions or logpoints in this mode.
- Changing the normal debugger startup flow when `-b` is provided.

## Decisions

### 1. App mode enum rather than conditional fields

Introduce an `AppMode` enum (`FilePicker` vs `Debugger`) set at startup based on whether breakpoints were provided. The renderer dispatches to a different render path per mode. This keeps the two UIs cleanly separated rather than threading `Option`s through every panel.

**Alternative considered**: Making the debugger connection optional throughout the existing UI. Rejected because it would require every panel (call stack, variables, REPL, control bar) to handle a "no debugger" state, adding complexity for no benefit.

### 2. File picker shown inline, not as overlay

In file-picker mode, the file picker is displayed as a persistent left sidebar panel rather than the current `Ctrl+P` overlay window. This makes it the primary navigation element. The code viewer occupies the central panel as it does today.

**Alternative considered**: Keeping the overlay and showing a "press Ctrl+P to start" prompt. Rejected because it adds an unnecessary step and doesn't feel like a cohesive standalone mode.

### 3. Breakpoints stored in UI state, persisted on exit

Breakpoints set in file-picker mode are stored in `DebuggerAppState.ui_breakpoints` (the same `HashSet<Breakpoint>` used during debugging) and persisted to the state file on exit. This means they'll be available when the user later launches a debug session.

### 4. No new CLI arguments

The mode is determined implicitly: no `-b` flag → file-picker mode. This avoids adding flags and keeps the UX simple. The existing `config_path` argument remains required since the project context is needed for file enumeration.

**Revisited**: Actually, file-picker mode doesn't need a debug config at all — it only needs to know the project directory for file enumeration. We should make `config_path` optional (or add a separate `--browse` flag). However, to keep scope minimal, we'll make `config_path` optional: if omitted, the app starts in file-picker mode using CWD as the project root.

### 5. Minimal renderer for file-picker mode

The file-picker mode renderer only renders:
- A left sidebar with the file picker (search input + results list)
- A central panel with the code viewer (syntax highlighting + gutter breakpoints)

No top control panel, no bottom panel, no status bar. This keeps the UI focused and uncluttered.

## Risks / Trade-offs

- **[Code viewer without debugger]** → The `CodeView` widget currently expects frame context for execution line highlighting. Mitigation: it already handles `file_override` mode which skips execution highlighting — file-picker mode will use this same path.
- **[Breakpoint sync without debugger]** → In normal mode, breakpoints are synced to the debugger via `AsyncBridge`. In file-picker mode there's no debugger, so breakpoints are only stored locally. Mitigation: breakpoints are already persisted to state independently of the debugger.
- **[Making config_path optional]** → This is a CLI breaking change for scripts that rely on positional args. Mitigation: make it an `Option<PathBuf>` — if provided, behavior is unchanged; if omitted, file-picker mode activates.
