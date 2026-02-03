# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

DAP GUI is a general-purpose GUI debugger built on the [Debug Adapter Protocol (DAP)](https://microsoft.github.io/debug-adapter-protocol/) using Rust and egui. The project follows a multi-crate architecture separating concerns into distinct layers.

## Build and Test Commands

### Building

```bash
cargo build                    # Debug build
cargo build --release          # Release build
```

### Testing

```bash
# Run all tests (requires Python venv with debugpy)
uv venv
uv pip install -r requirements.txt
source .venv/bin/activate
cargo nextest run --locked --all-features --all-targets

# Run doc tests
cargo test --locked --all-features --doc

# Run a specific test
cargo nextest run <test_name>
```

### Linting and Formatting

```bash
cargo fmt                      # Format code
cargo fmt --check              # Check formatting
cargo clippy                   # Run linter
```

### Python Bindings

```bash
# Build and install pythondap
maturin develop --manifest-path crates/pythondap/Cargo.toml

# Run Python tests
source .venv/bin/activate
python -m pytest crates/pythondap/tests
```

## Architecture

The codebase is organized as a Rust workspace with distinct crates representing architectural layers:

### Core Protocol Stack (bottom to top)

1. **`transport`** - Wire protocol layer
   - Serialization/deserialization of DAP messages
   - Send requests with/without responses (request-response pattern)
   - Publish received events via channels
   - Background polling thread in `Client` (`crates/transport/src/client.rs`)
   - Request tracking via `RequestStore`

2. **`server`** - Process management
   - Abstraction over running DAP server processes
   - Handles spawning and lifecycle of debug adapters

3. **`debugger`** - High-level debugger API
   - Main entry point: `Debugger` type in `crates/debugger/src/debugger.rs`
   - State machine for debugger lifecycle (see state transitions below)
   - High-level controls: continue, step, breakpoint management
   - Initialization and configuration of debug sessions
   - Two initialization modes: `Launch` (start new process) or `Attach` (connect to running process)

### Supporting Crates

- **`launch_configuration`** - Launch config parsing (e.g., VSCode `launch.json` files)
- **`state`** - Cross-session state persistence (breakpoints, project settings)
- **`dap-codec`** / **`dap-interface`** - DAP protocol types and interfaces

### UI Implementations

- **`gui`** - Main egui/eframe GUI
- **`gui2`** - Alternative GUI implementation
- **`tui`** - Terminal UI
- **`repl`** - REPL interface

### Utilities

- **`pcaplog`** - Print messages from pcap(ng) captures (debugging tool)
- **`pythondap`** - Python bindings (PyO3/maturin)

## Debugger State Machine

The `debugger::Debugger` type follows this state machine (from `crates/debugger/src/state.rs`):

```
Initialized → Running → Paused → Running → Terminated
                ↓         ↑  ↓
                ↓         ↑  ScopeChange
                ↓         ↑      ↓
                ↓         ←──────┘
                ↓
             Terminated
```

States:

- **Initialized**: Connected, waiting to start
- **Running**: Program executing
- **Paused**: Hit breakpoint/stopped, can inspect state
- **ScopeChange**: User changed stack frame or variable scope
- **Terminated**: Debugging session ended

Events are published through channels to notify UI layers of state changes.

## DAP Initialization Sequence

The startup sequence for a debugging session (from `notes/startup_sequence.md`):

1. Load persisted state (breakpoints, function breakpoints, exception breakpoints)
2. Send `Initialize` request (with `lines_starting_at_one = true`)
3. Send `Launch` or `Attach` request (adapter-specific options)
4. Wait for `Initialized` event (adapter ready for breakpoint config)
5. Configure pre-existing breakpoints
6. Send `ConfigurationDone` to indicate breakpoints are set
7. Wait for `Stopped` event

## Language Support

Currently supports:

- **DebugPy** (Python) - via `debugpy` adapter
- **Delve** (Go) - via `dlv` (requires Go and delve installed)

Language is specified in `Language` enum (`crates/debugger/src/state.rs`).

## Common Patterns

### Channel-based Communication

The codebase uses `crossbeam-channel` extensively for async communication between the transport layer and higher levels. Events flow from `transport::Client` background thread → `debugger::Debugger` → UI.

### Request-Response Pattern

Transport layer uses `RequestStore` with oneshot channels to match responses to requests by sequence number.

### Persistence

The `state` crate handles saving/loading breakpoints and session state as JSON files. State is automatically persisted in platform-specific directories (via `dirs` crate).

## Dependencies

- Python 3.11+ with `debugpy` package (for Python debugging)
- Go 1.21+ with `delve` (for Go debugging)
- Rust stable or beta toolchain

## MSRV

Minimum Supported Rust Version: **1.72.0**

## Configuration

- Uses Rust edition **2024**
- Release builds include debug symbols (`debug = true` in `Cargo.toml`)
- Workspace uses `resolver = "2"`

## Notes

- Tests require a Python virtual environment with `debugpy` installed
- End-to-end tests run actual debug sessions, so they need debugpy and delve available
- Use `cargo nextest` (not plain `cargo test`) for test execution
- *ALWAYS* perform the following steps before pushing a commit to GitHub:
    - format the code with `cargo fmt`
    - make sure the code compiles with `cargo check --all-features --all-targets` (don't worry about warnings)
    - run the tests with `cargo nextest run  --exclude pythondap --workspace --locked --all-features --all-targets` and `cargo test --doc`
- Set `RUST_LOG` environment variable for tracing output during tests
- Always format code after writing with `cargo fmt`

# Code Generation Rules
- IMPORTANT: Under NO circumstances should you ever use emoji characters in your responses or in any code you generate.
- Never use emojis in documentation, comments, or commit messages.

## Active Technologies
- Rust 2024 edition, MSRV 1.72.0 + ratatui 0.30.0, crossterm 0.29.0, tokio 1.48, serde/serde_json (001-fuzzy-file-picker)
- JSON file via state crate (~/.config/dap-tui/state.json or --state argument) (001-fuzzy-file-picker)

## Recent Changes
- 001-fuzzy-file-picker: Added Rust 2024 edition, MSRV 1.72.0 + ratatui 0.30.0, crossterm 0.29.0, tokio 1.48, serde/serde_json
