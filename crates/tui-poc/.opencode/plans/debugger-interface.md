# Debugger-Style Interface POC Plan

## Overview

Build a GDB/terminal-style debugger interface using ratatui. Single-file implementation in `main.rs` with panel focus switching.

## Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ Debugger                                            [RUNNING]   │
├───────────────┬─────────────────────────────────────────────────┤
│ Breakpoints   │    1 │ fn main() {                              │
│───────────────│    2 │     let x = 10;                          │
│  ● line 5     │    3 │     let y = 20;                          │
│  ● line 12    │    4 │     let result = process(x, y);          │
│               │ >> 5 │     println!("{}", result);              │
│───────────────│    6 │ }                                        │
│ Call Stack    │    7 │                                          │
│───────────────│    8 │ fn process(a: i32, b: i32) -> i32 {      │
│  main         │    9 │     validate(a);                         │
│  process_data │   10 │     validate(b);                         │
│  validate     │   11 │     a + b                                │
│               │ ● 12 │ }                                        │
├───────────────┴─────────────────────────────────────────────────┤
│ State Explorer                                                  │
│ x = 10                                                          │
│ y = 20                                                          │
│ result = 30                                                     │
│ > _                                                             │
└─────────────────────────────────────────────────────────────────┘
```

## Data Structures

```rust
enum PanelFocus {
    LeftPanel,
    CodeWindow,
    BottomPanel,
}

struct App {
    focus: PanelFocus,
    code_lines: Vec<&'static str>,
    current_line: usize,
    breakpoints: HashSet<usize>,
    call_stack: Vec<&'static str>,
    state_input: String,
    state_output: Vec<String>,
    exit: bool,
}
```

## Visual Design (GDB/Terminal Style)

- **Borders**: Simple box-drawing characters, no gaps between panels
- **Colors**: Minimal
  - Red `●` for breakpoints
  - `>>` prefix or reverse video for current line
  - Dim/gray for unfocused panels
  - Bold for focused panel titles
- **Layout**: Dense, compact, no padding between sections

## Key Bindings

| Key | Action |
|-----|--------|
| `Tab` | Cycle focus: Left → Code → Bottom → Left |
| `q` | Quit application |
| Typing (bottom panel) | Append to input buffer |
| `Enter` (bottom panel) | Submit mock command |
| `Backspace` (bottom panel) | Delete character |

## Mock Data

- **Code**: 15-20 lines of sample Rust code (hardcoded)
- **Breakpoints**: Lines 5 and 12
- **Current line**: Line 5
- **Call stack**: `["main", "process_data", "validate"]`
- **State output**: Pre-populated with sample variable values

## Implementation Steps

1. Replace existing `App` struct with debugger state
2. Add `PanelFocus` enum
3. Create three-pane layout using `Layout::vertical` and `Layout::horizontal`
4. Implement header bar with title and status
5. Implement left panel:
   - Breakpoints section (list of line numbers with `●`)
   - Call Stack section (list of function names)
   - Internal horizontal divider between sections
6. Implement code window:
   - Line numbers (right-aligned)
   - Breakpoint markers in gutter (`●` or space)
   - Current line indicator (`>>` prefix)
   - Syntax: just plain text for POC
7. Implement bottom panel (State Explorer):
   - Output area showing variable state
   - Input line with `>` prompt
8. Add focus styling:
   - Focused panel: bold title, normal border
   - Unfocused panel: dim title, dim border
9. Implement key handling:
   - Tab cycles focus
   - q quits
   - Text input when bottom panel focused
10. Wire up rendering in `draw()` method

## Files Changed

- `src/main.rs` - Complete rewrite with debugger interface
