## ADDED Requirements

### Requirement: User can set breakpoints via text input
The breakpoints panel SHALL provide a text input field where the user can type a breakpoint location in `file:line` format and press Enter to add it.

#### Scenario: Add breakpoint with relative path
- **WHEN** the user types `src/main.py:42` and presses Enter
- **THEN** the system SHALL resolve the path relative to the debug session's `cwd` and add a breakpoint at line 42 of the resolved file

#### Scenario: Add breakpoint with absolute path
- **WHEN** the user types `/home/user/project/app.py:10` and presses Enter
- **THEN** the system SHALL add a breakpoint at line 10 of `/home/user/project/app.py` without modifying the path

#### Scenario: Breakpoint appears in breakpoints list
- **WHEN** the user adds a breakpoint via the text input
- **THEN** the breakpoint SHALL appear in the breakpoints panel list identically to gutter-click breakpoints

#### Scenario: Breakpoint is synced to debug adapter
- **WHEN** the user adds a breakpoint via the text input while a debug session is active
- **THEN** the system SHALL send a `SetBreakpoints` DAP request for the resolved file, including the new breakpoint

### Requirement: All paths resolved to absolute immediately
All user-provided paths SHALL be resolved to absolute paths at the point of entry. Relative paths are a UI convenience only — internally, all crates MUST operate exclusively on absolute paths. No relative path SHALL be stored in `Breakpoint::path`, sent in DAP messages, or written to persistence.

#### Scenario: Relative path resolved to absolute against cwd
- **WHEN** the launch configuration has `cwd` set to `/home/user/project`
- **AND** the user types `lib/utils.py:5`
- **THEN** the breakpoint path SHALL be stored as `/home/user/project/lib/utils.py` (absolute)

#### Scenario: Fallback to process working directory
- **WHEN** no `cwd` is configured in the launch configuration
- **AND** the user types `main.py:1`
- **THEN** the breakpoint path SHALL be resolved to an absolute path using the process working directory

#### Scenario: Absolute path is canonicalized
- **WHEN** the user types `/home/user/project/../other/file.py:3`
- **THEN** the breakpoint path SHALL be stored as `/home/user/other/file.py` (canonicalized absolute path)

#### Scenario: No relative paths in internal state
- **WHEN** a breakpoint is added via any input method
- **THEN** the `Breakpoint::path` field SHALL always contain an absolute path
- **AND** the path sent in the DAP `SetBreakpoints` request SHALL be absolute
- **AND** the path written to the persistence store SHALL be absolute

### Requirement: Input validation
The system SHALL validate the text input and reject malformed breakpoint specifications.

#### Scenario: Missing line number
- **WHEN** the user types `main.py` (no colon or line number) and presses Enter
- **THEN** the system SHALL not add a breakpoint and SHALL display an error indication

#### Scenario: Non-numeric line number
- **WHEN** the user types `main.py:abc` and presses Enter
- **THEN** the system SHALL not add a breakpoint and SHALL display an error indication

#### Scenario: Empty input
- **WHEN** the user presses Enter with an empty input field
- **THEN** the system SHALL do nothing

### Requirement: Input field clears on success
After a breakpoint is successfully added via the text input, the input field SHALL be cleared.

#### Scenario: Successful add clears input
- **WHEN** the user types `main.py:10` and presses Enter
- **AND** the breakpoint is successfully added
- **THEN** the input field SHALL be empty and ready for the next entry
