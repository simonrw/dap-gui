## Context

The dap-gui application currently supports breakpoints only via gutter clicks in the code viewer. The `Breakpoint` struct already stores `path: PathBuf` and `line: usize`, which is exactly what's needed. In remote attach workflows, users need to set breakpoints before any source file is open — they know the file and line but have no gutter to click.

The debugger already tracks `cwd` through `LaunchArguments::working_directory` and `AttachArguments::working_directory`, parsed from the launch configuration's `cwd` field.

## Goals / Non-Goals

**Goals:**
- Allow users to type `file:line` in a text input to set breakpoints
- Resolve relative paths against the session's `cwd` (from launch config), falling back to `std::env::current_dir()`
- Pass absolute paths through unchanged
- Integrate seamlessly with existing breakpoint infrastructure (persistence, DAP sync, gutter display)

**Non-Goals:**
- Conditional breakpoints or logpoints (future work)
- Function breakpoints (already partially supported, separate concern)
- Autocomplete/file picker for the path input
- Glob/wildcard patterns in file paths

## Decisions

### 1. Input location: Breakpoints panel

**Decision**: Add a text input field at the top of the existing breakpoints side panel.

**Rationale**: The breakpoints panel already lists all breakpoints. Placing input there keeps breakpoint management in one place. An alternative would be a command palette or toolbar, but those add complexity and the panel is the natural home.

### 2. Parsing format: `path:line`

**Decision**: Parse the last colon-separated component as the line number. Everything before it is the file path.

**Rationale**: This handles paths with colons (e.g., Windows `C:\foo\bar.py:10` — though less relevant on macOS/Linux) by taking only the last `:number` as the line. The format `file:line` is universally understood from compiler error output, grep results, etc.

### 3. Path resolution strategy — always absolute internally

**Decision**:
1. All user-provided paths MUST be resolved to absolute paths immediately at the point of entry (in `Breakpoint::parse`)
2. If the path is absolute, canonicalize it
3. If relative, join with the debug session's `cwd` (from launch config), then canonicalize
4. If no `cwd` is configured, join with `std::env::current_dir()`, then canonicalize
5. All internal representations (`Breakpoint::path`, DAP `Source::path`, persistence) MUST use absolute paths — relative paths are never stored or passed between crates

**Rationale**: Using absolute paths everywhere eliminates ambiguity when breakpoints are compared, persisted, or sent to the debug adapter. Relative paths are a UI convenience only — they are resolved at the boundary (user input) and absolute paths flow through all crates from that point on. This matches how debugpy and delve resolve paths. Canonicalization ensures breakpoints match regardless of how the path was specified.

### 4. Where to put the parsing logic

**Decision**: Add a `Breakpoint::parse(input: &str, cwd: &Path) -> Result<Breakpoint>` associated function to the existing `Breakpoint` type in the `debugger` crate.

**Rationale**: Keeps parsing close to the type definition. The `cwd` parameter makes the resolution explicit. The GUI just calls this function and handles the error (invalid format, non-numeric line).

## Risks / Trade-offs

- **[Path mismatch with debug adapter]** → The canonicalized path may not match what the debug adapter expects if path mappings are in play. Mitigation: This is an existing issue for gutter breakpoints too; path mapping support is orthogonal.
- **[Windows path handling]** → Colon-based parsing could conflict with drive letters. Mitigation: Parse from the right (last `:` followed by digits). Not a priority since the app targets macOS/Linux primarily.
- **[File doesn't exist]** → User may type a path that doesn't exist on disk. Mitigation: Allow it — the debug adapter will report the breakpoint as unverified, which is already handled in the UI. This is expected for remote attach where files may not be locally accessible.
