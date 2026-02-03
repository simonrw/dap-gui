# Contract: State Persistence Integration

**Location**: `crates/tui-poc/src/main.rs` (modifications to App struct and methods)

## Interface Changes

### App Struct Additions

```rust
impl App {
    /// Path to the state file for persistence
    state_path: PathBuf,
}
```

### New Methods

```rust
impl App {
    /// Save current breakpoints to the state file.
    /// Called after each breakpoint add/remove operation.
    /// Errors are logged but do not interrupt operation.
    fn save_breakpoints(&self);

    /// Convert UiBreakpoints to debugger::Breakpoints for persistence.
    fn breakpoints_for_persistence(&self) -> Vec<debugger::Breakpoint>;
}
```

## Behavior Specification

### save_breakpoints

**Trigger Points**:
1. After `add_breakpoint()` succeeds
2. After `remove_breakpoint()` succeeds
3. After `toggle_breakpoint_enabled()` changes state

**Implementation**:
```rust
fn save_breakpoints(&self) {
    let breakpoints = self.breakpoints_for_persistence();
    let persistence = state::Persistence {
        version: "0.1.0".to_string(),
        projects: vec![state::PerFile {
            path: self.current_file_path.clone(),
            breakpoints,
        }],
    };

    if let Err(e) = state::save_to(&persistence, &self.state_path) {
        tracing::warn!("Failed to save breakpoints: {}", e);
    }
}
```

**Error Handling**:
- Errors logged at WARN level
- Operation continues (non-fatal)
- User not interrupted by save failures

### breakpoints_for_persistence

**Conversion**:
```rust
fn breakpoints_for_persistence(&self) -> Vec<debugger::Breakpoint> {
    self.breakpoints
        .iter()
        .filter(|bp| bp.enabled) // Only persist enabled breakpoints
        .map(|bp| debugger::Breakpoint {
            name: None,
            path: bp.path.clone(),
            line: bp.line,
        })
        .collect()
}
```

## State File Location

**Resolution Order**:
1. `--state` command-line argument (if provided)
2. `~/.config/dap-tui/state.json` (default)

**Directory Creation**:
- Parent directories created automatically by `state::save_to()`

## Existing Load Path (unchanged)

```rust
fn load_breakpoints(state_path: &PathBuf) -> Vec<debugger::Breakpoint> {
    match state::StateManager::new(state_path) {
        Ok(manager) => manager
            .current()
            .projects
            .iter()
            .flat_map(|p| p.breakpoints.clone())
            .collect(),
        Err(e) => {
            eprintln!("Warning: Could not load state: {}", e);
            vec![]
        }
    }
}
```

## Integration Points

### Breakpoint Add (lines ~572-602)

```rust
// After successful add:
self.breakpoints.push(UiBreakpoint { ... });
self.save_breakpoints(); // NEW
```

### Breakpoint Remove (lines ~604-620)

```rust
// After successful remove:
self.breakpoints.retain(|bp| bp.id != Some(id));
self.save_breakpoints(); // NEW
```

### Breakpoint Toggle (if enabled tracking added)

```rust
// After toggle:
bp.enabled = !bp.enabled;
self.save_breakpoints(); // NEW
```

## Performance Requirements

| Operation | Target | Max |
|-----------|--------|-----|
| save_breakpoints (100 breakpoints) | 5ms | 20ms |
| save_breakpoints (1000 breakpoints) | 20ms | 50ms |

## Compatibility

- State file format unchanged (version "0.1.0")
- Backward compatible with existing state files
- Forward compatible (unknown fields ignored on load)
