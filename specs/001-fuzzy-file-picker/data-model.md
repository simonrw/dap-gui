# Data Model: Fuzzy File Picker for TUI Debugger

**Date**: 2026-02-03
**Feature**: 001-fuzzy-file-picker

## Entities

### TrackedFile

Represents a file under Git version control.

| Field | Type | Description |
|-------|------|-------------|
| relative_path | PathBuf | Path relative to repository root (e.g., `src/main.rs`) |
| absolute_path | PathBuf | Full filesystem path |
| display_name | String | Formatted for display (relative path) |

**Derivation**: `absolute_path` = repo_root + relative_path

**Source**: Output of `git ls-files`

### FuzzyMatchResult

A file that matches the fuzzy search query.

| Field | Type | Description |
|-------|------|-------------|
| file | TrackedFile | The matched file |
| score | i64 | Match quality score (higher = better) |
| matched_indices | Vec<usize> | Character positions that matched (for highlighting) |

**Ordering**: Results sorted by score descending, then alphabetically by path.

### Breakpoint (existing)

From `crates/debugger/src/types.rs`:

| Field | Type | Description |
|-------|------|-------------|
| name | Option<String> | Optional breakpoint name |
| path | PathBuf | Absolute file path |
| line | usize | 1-indexed line number |

### UiBreakpoint (existing in tui-poc)

| Field | Type | Description |
|-------|------|-------------|
| id | Option<u64> | Debugger-assigned ID (None if unconfirmed) |
| path | PathBuf | Absolute file path |
| line | usize | 1-indexed line number |
| enabled | bool | Whether breakpoint is active |

### Persistence (existing)

From `crates/state/src/lib.rs`:

| Field | Type | Description |
|-------|------|-------------|
| version | String | Schema version (currently "0.1.0") |
| projects | Vec<PerFile> | Per-project breakpoint lists |

### PerFile (existing)

| Field | Type | Description |
|-------|------|-------------|
| path | PathBuf | Project/file identifier |
| breakpoints | Vec<Breakpoint> | Breakpoints for this project |

## New State Fields (App struct)

| Field | Type | Description |
|-------|------|-------------|
| git_files | Vec<TrackedFile> | Cached list of Git-tracked files |
| git_files_loaded | bool | Whether git_files has been populated |
| repo_root | Option<PathBuf> | Git repository root directory |
| state_path | PathBuf | Path to state file for saving |

## State Transitions

### Command Palette Lifecycle

```
Closed
  │
  ├─[Ctrl+P]─> Opening
  │              │
  │              ├─[git ls-files success]─> Open (with files)
  │              │
  │              └─[git ls-files fail]─> Open (with error message)
  │
Open
  │
  ├─[typing]─> Filtering (update filtered list)
  │
  ├─[Enter on file]─> Loading File
  │                     │
  │                     ├─[success]─> Closed (file displayed)
  │                     │
  │                     └─[fail]─> Open (with error message)
  │
  └─[Escape]─> Closed (no change)
```

### Breakpoint Persistence Flow

```
Breakpoint Added/Removed
  │
  ├─> Update UI state (breakpoints vec)
  │
  ├─> Update breakpoints panel display
  │
  └─> Persist to state file
        │
        ├─[success]─> Done
        │
        └─[fail]─> Log warning, continue (non-fatal)
```

## Validation Rules

### TrackedFile

- `relative_path` must not be empty
- `relative_path` must not start with `/` or `\`
- `absolute_path` must exist on disk (at time of selection)

### Breakpoint

- `line` must be >= 1
- `path` must be an absolute path
- Duplicate breakpoints (same path + line) not allowed

### FuzzyMatchResult

- `score` is valid for any i64 value (can be negative for poor matches)
- `matched_indices` values must be valid indices into the filename string

## JSON Schema (State File)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["version", "projects"],
  "properties": {
    "version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$"
    },
    "projects": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "breakpoints"],
        "properties": {
          "path": { "type": "string" },
          "breakpoints": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["path", "line"],
              "properties": {
                "name": { "type": ["string", "null"] },
                "path": { "type": "string" },
                "line": { "type": "integer", "minimum": 1 }
              }
            }
          }
        }
      }
    }
  }
}
```

## Example State File

```json
{
  "version": "0.1.0",
  "projects": [
    {
      "path": "/Users/dev/myproject",
      "breakpoints": [
        {
          "name": null,
          "path": "/Users/dev/myproject/src/main.py",
          "line": 42
        },
        {
          "name": "login check",
          "path": "/Users/dev/myproject/src/auth.py",
          "line": 15
        }
      ]
    }
  ]
}
```
