# Feature Specification: Fuzzy File Picker for TUI Debugger

**Feature Branch**: `001-fuzzy-file-picker`
**Created**: 2026-02-03
**Status**: Draft
**Input**: User description: "Build out a feature in tui-poc that lets me select files using a fuzzy finder using the existing file picker interface. It should allow me to fuzzy find any version control tracked file in the current directory. The file should be opened in the code view window so that I can place breakpoints. The breakpoints should be persisted to the state file"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Find and Open a File by Name (Priority: P1)

A developer debugging their application wants to set a breakpoint in a file that hasn't been visited during execution yet. They press the file picker shortcut (Ctrl+P), type part of the filename, and the fuzzy finder shows matching files from the project's version control. They select a file and it opens in the code view, ready for breakpoint placement.

**Why this priority**: This is the core functionality - without being able to find and open files, no other features in this spec can work. It directly addresses the main pain point: the current file picker only shows already-loaded files.

**Independent Test**: Can be fully tested by launching the TUI, pressing Ctrl+P, typing a partial filename, and verifying the correct file opens in the code view.

**Acceptance Scenarios**:

1. **Given** the TUI is running and the file picker is closed, **When** the user presses Ctrl+P, **Then** the file picker overlay appears with an empty search field and a list of all version-controlled files.

2. **Given** the file picker is open with the query "mod", **When** the user types additional characters like "el", **Then** the file list updates in real-time to show only files matching "model" (e.g., "models.py", "data_model.rs", "user_model.go").

3. **Given** the file picker shows filtered results, **When** the user presses Enter on a selected file, **Then** the file picker closes and the selected file appears in the code view with its contents displayed.

4. **Given** the file picker is open, **When** the user presses Escape, **Then** the file picker closes without changing the current code view.

---

### User Story 2 - Place Breakpoints in Newly Opened File (Priority: P2)

After opening a file via the fuzzy finder, the developer navigates to a specific line using existing code view navigation (j/k, g/G, etc.) and toggles a breakpoint with the 'b' key. The breakpoint appears in both the code view margin and the breakpoints panel.

**Why this priority**: This enables the primary use case - setting breakpoints in files before they're reached during execution. Depends on P1 (file must be openable first).

**Independent Test**: Open a file via fuzzy finder, navigate to a line, press 'b', and verify the breakpoint marker appears and is listed in the breakpoints panel.

**Acceptance Scenarios**:

1. **Given** a file is opened via the fuzzy finder, **When** the user navigates to line 25 and presses 'b', **Then** a breakpoint marker (*) appears in the code view margin at line 25.

2. **Given** a breakpoint is added to a newly opened file, **When** the user views the breakpoints panel, **Then** the new breakpoint appears in the list with the correct file path and line number.

3. **Given** a breakpoint exists at line 25, **When** the user navigates to line 25 and presses 'b' again, **Then** the breakpoint is removed from both the code view and breakpoints panel.

---

### User Story 3 - Persist Breakpoints Across Sessions (Priority: P3)

The developer adds breakpoints to several files during a debugging session. When they close and restart the TUI, all breakpoints they set are restored, including those in files that were opened via the fuzzy finder.

**Why this priority**: Persistence ensures work isn't lost between sessions. Without this, users would need to re-add breakpoints every time they start the debugger. Depends on P1 and P2 being functional.

**Independent Test**: Add breakpoints, close the TUI, reopen it, and verify all breakpoints are present in the breakpoints panel and code views.

**Acceptance Scenarios**:

1. **Given** the user has added breakpoints to files A.py (line 10) and B.py (line 20), **When** the user terminates and restarts the TUI with the same state file, **Then** both breakpoints appear in the breakpoints panel.

2. **Given** the state file contains breakpoints from a previous session, **When** the user opens a file that has persisted breakpoints via the fuzzy finder, **Then** the breakpoint markers appear in the code view margin at the correct lines.

3. **Given** the user removes a breakpoint during a session, **When** the session ends and restarts, **Then** the removed breakpoint is not present in the state.

---

### Edge Cases

- What happens when the project has no version control initialized? The fuzzy finder shows an informative message ("No version-controlled files found") and the user can dismiss the picker.

- What happens when the user searches for a file that doesn't exist? The file list shows empty with a "No matches" indicator; pressing Enter does nothing.

- What happens when a version-controlled file is deleted from disk after being indexed? When selected, the system shows an error message ("File not found") and remains on the current view.

- What happens when the state file is corrupted or invalid? The system logs a warning, starts with empty breakpoints, and creates a fresh state on next save.

- What happens when a breakpoint is set on a line that no longer exists in the file? The breakpoint is preserved in state but visually indicated as invalid (different marker or warning).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST enumerate all files tracked by the project's version control system (Git) in the current working directory.

- **FR-002**: System MUST support fuzzy matching of filenames, where the search query matches characters in order but not necessarily consecutively (e.g., "mcon" matches "main_controller.py").

- **FR-003**: System MUST update the filtered file list as the user types, within 100ms of each keystroke.

- **FR-004**: System MUST display matching files with their relative paths from the project root to disambiguate files with the same basename.

- **FR-005**: System MUST load the selected file's contents into the code view when the user confirms their selection.

- **FR-006**: System MUST allow breakpoint placement on any line of a file opened via the fuzzy finder, using the existing 'b' key toggle.

- **FR-007**: System MUST save all breakpoints (including file path and line number) to the state file when breakpoints are added or removed.

- **FR-008**: System MUST load breakpoints from the state file on startup and restore them to the breakpoints panel.

- **FR-009**: System MUST display breakpoint markers in the code view when a file with existing breakpoints is opened.

- **FR-010**: System MUST handle version control repositories with up to 50,000 tracked files without degraded search performance.

### Key Entities

- **Tracked File**: A file under version control, represented by its path relative to the repository root. Attributes: relative path, absolute path, filename (basename).

- **Fuzzy Match Result**: A file that matches the search query with a relevance score. Attributes: file reference, match score, matched character positions (for highlighting).

- **Breakpoint**: A marker at a specific file location where execution should pause. Attributes: file path, line number, enabled status.

- **State File**: Persistent storage for user's debugging configuration. Contains: list of projects, each with associated breakpoints.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can locate and open any version-controlled file within 5 seconds of opening the file picker, for projects with up to 10,000 files.

- **SC-002**: Fuzzy search results update within 100ms of each keystroke on reference hardware (M1 MacBook or equivalent).

- **SC-003**: 100% of breakpoints set during a session are persisted and restored on the next session launch.

- **SC-004**: Users can set a breakpoint in a file not yet visited during execution without needing to trigger execution first.

- **SC-005**: File picker displays results within 500ms of activation, even for repositories with 50,000 tracked files.

## Assumptions

- Git is the version control system in use. The feature will use `git ls-files` or equivalent to enumerate tracked files.

- The TUI is invoked from within a Git repository or a subdirectory thereof.

- File paths are UTF-8 encoded and do not contain null characters.

- The state file location follows the existing convention (~/.config/dap-tui/state.json or --state argument).

- Fuzzy matching will use a substring-based algorithm initially, with potential for more sophisticated scoring (like fzf) as an enhancement.
