# Tasks: Fuzzy File Picker for TUI Debugger

**Input**: Design documents from `/specs/001-fuzzy-file-picker/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: No explicit tests requested in specification - implementation only.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Repository uses Rust workspace with multi-crate architecture:
- `crates/tui-poc/src/` - Main TUI application
- `crates/state/src/` - State persistence
- `crates/debugger/src/` - Debugger types

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and dependency setup

- [X] T001 Add nucleo-matcher dependency to crates/tui-poc/Cargo.toml
- [X] T002 [P] Run cargo fmt to ensure formatting is correct
- [ ] T003 [P] Run cargo clippy to verify no warnings

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core fuzzy matching module that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Create fuzzy matching module in crates/tui-poc/src/fuzzy.rs
- [X] T005 [P] Implement TrackedFile struct in crates/tui-poc/src/fuzzy.rs
- [X] T006 [P] Implement FuzzyMatch struct in crates/tui-poc/src/fuzzy.rs
- [X] T007 Implement find_repo_root() function in crates/tui-poc/src/fuzzy.rs
- [X] T008 Implement list_git_files() function in crates/tui-poc/src/fuzzy.rs
- [X] T009 Implement fuzzy_filter() function using nucleo-matcher in crates/tui-poc/src/fuzzy.rs
- [X] T010 Add module declaration for fuzzy in crates/tui-poc/src/main.rs

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Find and Open a File by Name (Priority: P1) 🎯 MVP

**Goal**: Enable users to fuzzy search all Git-tracked files and open them in the code view

**Independent Test**: Launch TUI, press Ctrl+P, type partial filename, verify file opens in code view

### Implementation for User Story 1

- [X] T011 [P] [US1] Add git_files field (Vec<TrackedFile>) to App struct in crates/tui-poc/src/main.rs
- [X] T012 [P] [US1] Add git_files_loaded field (bool) to App struct in crates/tui-poc/src/main.rs
- [X] T013 [P] [US1] Add repo_root field (Option<PathBuf>) to App struct in crates/tui-poc/src/main.rs
- [X] T014 [US1] Change command_palette_filtered type from Vec<String> to Vec<FuzzyMatch> in crates/tui-poc/src/main.rs
- [X] T015 [US1] Implement load_git_files() method in App impl in crates/tui-poc/src/main.rs
- [X] T016 [US1] Modify open_command_palette() to call load_git_files() if not loaded in crates/tui-poc/src/main.rs
- [X] T017 [US1] Replace update_filtered_files() implementation to use fuzzy::fuzzy_filter in crates/tui-poc/src/main.rs
- [X] T018 [US1] Modify select_command_palette_item() to use FuzzyMatch and absolute paths in crates/tui-poc/src/main.rs
- [X] T019 [US1] Add error handling for file not found in select_command_palette_item in crates/tui-poc/src/main.rs
- [X] T020 [US1] Update command palette rendering to display relative paths in crates/tui-poc/src/main.rs
- [X] T021 [US1] Implement match highlighting in palette item rendering in crates/tui-poc/src/main.rs
- [X] T022 [US1] Add empty state messages (no git repo, no matches, etc.) in crates/tui-poc/src/main.rs

**Checkpoint**: At this point, User Story 1 should be fully functional - can fuzzy find and open any Git-tracked file

---

## Phase 4: User Story 2 - Place Breakpoints in Newly Opened File (Priority: P2)

**Goal**: Enable breakpoint placement in files opened via fuzzy finder

**Independent Test**: Open file via fuzzy finder, navigate to line, press 'b', verify breakpoint appears

**Note**: User Story 2 is primarily supported by existing breakpoint functionality. The main requirement is ensuring files opened via fuzzy finder integrate correctly with the existing breakpoint system, which was addressed in US1 by properly loading files into the cache.

### Implementation for User Story 2

- [X] T023 [US2] Verify breakpoint toggle ('b' key) works in files opened via fuzzy finder in crates/tui-poc/src/main.rs
- [X] T024 [US2] Verify breakpoint markers display correctly in code view margin for fuzzy-opened files in crates/tui-poc/src/main.rs
- [X] T025 [US2] Verify breakpoints panel shows correct file paths for fuzzy-opened files in crates/tui-poc/src/main.rs

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently - can find files and place breakpoints

---

## Phase 5: User Story 3 - Persist Breakpoints Across Sessions (Priority: P3)

**Goal**: Save and restore breakpoints across TUI restarts

**Independent Test**: Add breakpoints, close TUI, reopen, verify breakpoints restored

### Implementation for User Story 3

- [X] T026 [P] [US3] Add state_path field (PathBuf) to App struct in crates/tui-poc/src/main.rs
- [X] T027 [US3] Implement breakpoints_for_persistence() method in App impl in crates/tui-poc/src/main.rs
- [X] T028 [US3] Implement save_breakpoints() method in App impl in crates/tui-poc/src/main.rs
- [X] T029 [US3] Add save_breakpoints() call after breakpoint add in crates/tui-poc/src/main.rs
- [X] T030 [US3] Add save_breakpoints() call after breakpoint remove in crates/tui-poc/src/main.rs
- [X] T031 [US3] Verify state file format matches existing Persistence schema in crates/state/src/lib.rs
- [X] T032 [US3] Add error logging for save failures (non-fatal) in crates/tui-poc/src/main.rs

**Checkpoint**: All user stories should now be independently functional - complete feature implementation

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final quality checks and validation

- [ ] T033 [P] Run cargo fmt on all modified files
- [ ] T034 [P] Run cargo clippy and fix any warnings
- [ ] T035 Run cargo build to verify compilation
- [ ] T036 Run cargo nextest run -p tui-poc to verify tests pass
- [ ] T037 Manual validation following quickstart.md scenarios
- [ ] T038 Verify performance targets (<100ms filter, <500ms palette open for 50k files)
- [ ] T039 Test edge cases from spec.md (no git repo, deleted files, corrupted state)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-5)**: All depend on Foundational phase completion
  - User Story 1 (P1): Can start after Foundational - No dependencies on other stories
  - User Story 2 (P2): Can start after Foundational - Minimal integration with US1 (uses same file loading)
  - User Story 3 (P3): Can start after Foundational - Uses breakpoint system but independent of US1/US2
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Independent - Core fuzzy file finder
- **User Story 2 (P2)**: Independent - Relies on existing breakpoint system, minimal US1 integration
- **User Story 3 (P3)**: Independent - State persistence orthogonal to file finding and breakpoint placement

### Within Each User Story

- **User Story 1**: 
  - App struct fields (T011-T014) can run in parallel [P]
  - Methods must be sequential (T015-T022) due to dependencies
  - Rendering updates at end (T020-T022)

- **User Story 2**:
  - All tasks are verification/integration tasks (T023-T025)
  - Can run sequentially as quick checks

- **User Story 3**:
  - State_path field (T026) independent [P]
  - Methods (T027-T028) sequential
  - Integration calls (T029-T032) sequential

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel (T002, T003)
- All Foundational struct definitions marked [P] can run in parallel (T005, T006)
- User Story 1 struct fields marked [P] can run in parallel (T011, T012, T013)
- Once Foundational phase completes, User Stories 1, 2, and 3 can be worked on in parallel by different team members
- Polish tasks marked [P] can run in parallel (T033, T034)

---

## Parallel Example: User Story 1 Initial Setup

```bash
# Launch all struct field additions for User Story 1 together:
Task: "Add git_files field to App struct in crates/tui-poc/src/main.rs"
Task: "Add git_files_loaded field to App struct in crates/tui-poc/src/main.rs"
Task: "Add repo_root field to App struct in crates/tui-poc/src/main.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test fuzzy file finding independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (fuzzy file finder)
   - Developer B: User Story 2 (breakpoint verification)
   - Developer C: User Story 3 (persistence)
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- No tests explicitly requested in spec - focus on implementation
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Tests are optional and not included per specification
