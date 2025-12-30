# GUI Proof of Concept

A proof-of-concept debugger UI built with egui to demonstrate the layout and interaction patterns for a DAP debugger interface.

## Features

This POC demonstrates:

### Layout Structure
- **Top Panel**: Control buttons (Continue/Pause, Step Over, Step Into, Step Out, Stop, Restart)
- **Left Sidebar**: Call stack panel showing stack frames
- **Right Sidebar**: Breakpoints panel with enable/disable toggles
- **Bottom Panel**: Tabbed interface for Variables, Breakpoints, and Console
- **Central Panel**: Code viewer with:
  - Python syntax highlighting (keywords, strings, comments, etc.)
  - Line numbers
  - Breakpoint indicators
  - Current line highlighting with yellow arrow marker
  - Automatic dark/light theme adaptation

### Interactions
- Click control buttons to simulate debugger actions
- Select stack frames from the call stack
- Toggle breakpoints on/off
- Switch between tabs in the bottom panel
- View mock variables, breakpoints, and console output

## Mock Data

The POC uses hardcoded mock data to demonstrate the UI:
- Example Python code with line numbers
- Sample variables (x, name, items)
- Pre-set breakpoints at specific lines
- Mock call stack (main → process_data → calculate)
- Console messages showing debugger events

## Technical Details

### Syntax Highlighting

The code viewer uses **egui_extras** with the `syntect` feature for professional syntax highlighting:
- Powered by the [syntect](https://github.com/trishume/syntect) library (used by ripgrep, bat, and many other tools)
- Supports Python, Rust, C/C++, TOML, and many other languages
- Automatically adapts colors to dark/light mode
- Optimized with memoization for repeated highlighting
- Current implementation hardcoded for Python (`"py"`)

### Architecture Note

This crate is **excluded from the workspace** (see root `Cargo.toml`). This allows it to use the latest `eframe` version (0.33.3) without conflicting with other GUI implementations in the workspace (like `gui2` which uses `iced`). The exclusion gives `gui-poc` its own independent `Cargo.lock` file.

## Running

```bash
cargo run --manifest-path crates/gui-poc/Cargo.toml
```

The POC runs as a standalone application with its own dependency tree.

## Next Steps

To integrate with the real debugger:
1. Replace MockState with real AppState connected to debugger crate
2. Wire up control buttons to actual debugger commands
3. Connect event loop to receive debugger events
4. Load and display real source files
5. Implement proper variable tree expansion
6. Add REPL functionality for expression evaluation
7. Handle breakpoint creation/deletion through the debugger API

## Dependencies

- **eframe 0.33.3**: egui framework for building the GUI
- **egui_extras 0.33.3** (with `syntect` feature): Syntax highlighting support

## Code Structure

- `main.rs`: Single-file POC containing:
  - MockState with sample data
  - App struct implementing eframe::App
  - Panel layouts and widget rendering
  - Mock debugger state and interactions
  - Syntax-highlighted code display using egui_extras
