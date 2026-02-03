# Quickstart: Fuzzy File Picker for TUI Debugger

## Prerequisites

- Rust toolchain (edition 2024, MSRV 1.72.0)
- Git installed and in PATH
- Python 3.11+ with debugpy (for testing with Python debugger)

## Building

```bash
# From repository root
cargo build -p tui-poc
```

## Running

```bash
# Start TUI debugger with a Python project
cargo run -p tui-poc -- --config path/to/launch.json

# With custom state file location
cargo run -p tui-poc -- --config launch.json --state ./my-state.json
```

## Using the Fuzzy File Picker

### Opening Files

1. Press `Ctrl+P` to open the file picker
2. Type part of a filename (fuzzy matching supported)
   - Example: `mcon` matches `main_controller.py`
   - Example: `tst` matches `test_auth.py`, `tests/unit.py`
3. Use `j/k` or arrow keys to navigate results
4. Press `Enter` to open the selected file
5. Press `Escape` to cancel

### Setting Breakpoints

1. Open a file using the fuzzy picker (`Ctrl+P`)
2. Navigate to the desired line:
   - `j/k` - move down/up one line
   - `g/G` - jump to top/bottom
   - `H/L` - half page up/down
3. Press `b` to toggle a breakpoint at the cursor line
4. Breakpoint appears as `*` in the left margin

### Verifying Persistence

1. Set some breakpoints in various files
2. Quit the debugger (`q` or `Ctrl+C`)
3. Restart the debugger with the same config
4. Verify breakpoints appear in the breakpoints panel
5. Open a file with breakpoints - markers should be visible

## Troubleshooting

### "Not in a git repository"

The fuzzy finder requires Git. Ensure:
- You're running from within a Git repository
- Git is installed and accessible via PATH

### Files not appearing in picker

The picker shows files tracked by Git. To see a file:
- Ensure it's committed or staged (`git add`)
- Check `.gitignore` isn't excluding it
- Run `git ls-files` to verify what Git sees

### Breakpoints not persisting

Check state file location:
```bash
# Default location
cat ~/.config/dap-tui/state.json

# Or check custom location if --state was used
```

### Performance issues with large repos

For repositories with >50,000 files:
- First `Ctrl+P` may take up to 500ms
- Subsequent opens use cached file list
- Typing filters incrementally (should feel instant)

## Development

### Running Tests

```bash
cargo nextest run -p tui-poc --locked --all-features
```

### Checking Code Quality

```bash
cargo fmt --check
cargo clippy -p tui-poc
```

### Viewing Logs

```bash
# Logs written to /tmp/dap-gui.log by default
tail -f /tmp/dap-gui.log

# Or specify custom log location
cargo run -p tui-poc -- --config launch.json --log ./debug.log
```

## Keyboard Reference

### File Picker (Ctrl+P mode)

| Key | Action |
|-----|--------|
| `Ctrl+P` | Open/close picker |
| `Escape` | Close picker |
| `Enter` | Open selected file |
| `j` / Down | Move selection down |
| `k` / Up | Move selection up |
| Type | Filter files (fuzzy) |
| Backspace | Delete last character |

### Code View (when focused)

| Key | Action |
|-----|--------|
| `j` / Down | Move cursor down |
| `k` / Up | Move cursor up |
| `g` | Jump to top |
| `G` | Jump to bottom |
| `H` | Half page up |
| `L` | Half page down |
| `b` | Toggle breakpoint |
| `0` | Jump to execution line |
| `{` / `}` | Jump to prev/next blank line |

### Global

| Key | Action |
|-----|--------|
| `Tab` | Cycle focus between panels |
| `Shift+Tab` | Cycle focus reverse |
| `q` | Quit debugger |
| `c` | Continue execution |
| `n` | Step over |
| `s` | Step into |
| `o` | Step out |
