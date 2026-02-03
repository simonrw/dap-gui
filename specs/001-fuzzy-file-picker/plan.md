# Implementation Plan: Fuzzy File Picker for TUI Debugger

**Branch**: `001-fuzzy-file-picker` | **Date**: 2026-02-03 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-fuzzy-file-picker/spec.md`

## Summary

Extend the existing command palette (Ctrl+P) in tui-poc to support fuzzy finding of all Git-tracked files in the repository, rather than only files already loaded in the file cache. When a file is selected, load it into the code view where users can navigate and place breakpoints. Implement state persistence so breakpoints are saved when added/removed and restored on startup.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: Rust 2024 edition, MSRV 1.72.0
**Primary Dependencies**: ratatui 0.30.0, crossterm 0.29.0, tokio 1.48, serde/serde_json, nucleo-matcher 0.3
**Storage**: JSON file via state crate (~/.config/dap-tui/state.json or --state argument)
**Testing**: cargo nextest run --locked --all-features --all-targets
**Target Platform**: Cross-platform terminal (macOS, Linux, Windows)
**Project Type**: Rust workspace, multi-crate architecture
**Performance Goals**: <100ms fuzzy search updates, <500ms file picker activation for 50k files
**Constraints**: <200MB memory, 60 FPS UI rendering, no UI thread blocking
**Scale/Scope**: Support repositories with up to 50,000 tracked files

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | Status |
|-----------|-------------|--------|
| I. Code Quality | Code compiles, formatted, passes clippy, public APIs documented | PLANNED |
| I. Code Quality | Error handling uses Result types, no panics in library code | PLANNED |
| II. Testing Standards | New functionality includes tests before merge | PLANNED |
| II. Testing Standards | Tests run via cargo nextest | PLANNED |
| III. UX Consistency | State changes communicated within 100ms | PLANNED (FR-003) |
| III. UX Consistency | Session state persists across restarts | PLANNED (US3) |
| III. UX Consistency | UI remains responsive during long operations | PLANNED |
| IV. Performance | UI maintains 60 FPS | PLANNED |
| IV. Performance | Startup under 500ms (excluding adapter init) | PLANNED (SC-005) |
| IV. Performance | Memory under 200MB for typical sessions | PLANNED |

**Gate Status**: PASS - All requirements can be satisfied by the planned implementation.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
crates/
├── tui-poc/
│   ├── src/
│   │   ├── main.rs           # Main TUI - modify command palette, add Git file listing
│   │   ├── async_bridge.rs   # Async communication (minimal changes if any)
│   │   └── fuzzy.rs          # NEW: Fuzzy matching module
│   └── Cargo.toml            # Add nucleo-matcher dependency
├── state/
│   └── src/
│       └── lib.rs            # State persistence - verify save API works correctly
└── debugger/
    └── src/
        └── types.rs          # Breakpoint type (existing)
```

**Structure Decision**: This feature modifies the existing tui-poc crate within the established multi-crate workspace. A new `fuzzy.rs` module encapsulates fuzzy matching logic. State persistence uses the existing `state` crate API.

## Complexity Tracking

> No constitution violations requiring justification.

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| Fuzzy library | Use `nucleo-matcher` crate | High-performance, proven in Helix editor, optimized for TUI |
| Git integration | Shell out to `git ls-files` | Simple, reliable, avoids libgit2 complexity |
| File caching | Lazy load on selection | Avoids memory bloat from pre-loading all files |
| Save trigger | On each breakpoint add/remove | Ensures no data loss, acceptable overhead |

## Post-Design Constitution Re-check

*Re-evaluated after Phase 1 design completion.*

| Principle | Verification | Status |
|-----------|--------------|--------|
| I. Code Quality | fuzzy.rs module uses Result for errors, no unwrap/panic | VERIFIED in contracts |
| I. Code Quality | Public functions documented in contracts | VERIFIED |
| II. Testing | Unit tests planned for fuzzy module | TO BE IMPLEMENTED |
| II. Testing | Integration test for persistence planned | TO BE IMPLEMENTED |
| III. UX Consistency | <100ms filter updates per contracts | VERIFIED (target 20ms) |
| III. UX Consistency | State saved on every breakpoint change | VERIFIED in contract |
| III. UX Consistency | Async Git enumeration prevents UI block | VERIFIED in design |
| IV. Performance | 50k file support within 500ms | VERIFIED (target 200ms) |
| IV. Performance | Memory: file paths ~5MB for 50k files | VERIFIED within budget |

**Post-Design Gate Status**: PASS - Design satisfies all constitution requirements.

## Generated Artifacts

| Artifact | Path | Purpose |
|----------|------|---------|
| research.md | specs/001-fuzzy-file-picker/research.md | Technology decisions and rationale |
| data-model.md | specs/001-fuzzy-file-picker/data-model.md | Entity definitions and state transitions |
| fuzzy-module.md | specs/001-fuzzy-file-picker/contracts/fuzzy-module.md | Fuzzy matching module API |
| state-persistence.md | specs/001-fuzzy-file-picker/contracts/state-persistence.md | Persistence integration |
| command-palette.md | specs/001-fuzzy-file-picker/contracts/command-palette.md | UI modifications |
| quickstart.md | specs/001-fuzzy-file-picker/quickstart.md | Usage guide and testing |

## Next Steps

Run `/speckit.tasks` to generate the implementation task list.
