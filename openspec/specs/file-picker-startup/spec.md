### Requirement: App starts in file-picker mode when no breakpoints provided
When the user launches the GUI without the `-b/--breakpoints` argument and without a config path, the app SHALL start in file-picker mode instead of connecting to a debugger.

#### Scenario: No arguments provided
- **WHEN** the user runs the app with no arguments
- **THEN** the app starts in file-picker mode using the current working directory as the project root

#### Scenario: Config path provided without breakpoints
- **WHEN** the user runs the app with only a config path and no `-b` flag
- **THEN** the app starts in file-picker mode using the config's project directory

#### Scenario: Breakpoints provided
- **WHEN** the user runs the app with `-b file:line` arguments
- **THEN** the app starts in normal debugger mode (existing behavior unchanged)

### Requirement: File-picker mode shows only file picker and code viewer
In file-picker mode, the UI SHALL display only a file picker sidebar and a source code viewer. No control panel, call stack, variables panel, REPL, or status bar SHALL be rendered.

#### Scenario: File-picker mode UI layout
- **WHEN** the app is in file-picker mode
- **THEN** a left sidebar shows the file picker with a search input and results list
- **THEN** a central panel shows the source code viewer with syntax highlighting

#### Scenario: Debug UI elements are hidden
- **WHEN** the app is in file-picker mode
- **THEN** the control panel (continue, step over, step in, step out) is not rendered
- **THEN** the call stack panel is not rendered
- **THEN** the variables/REPL bottom panel is not rendered
- **THEN** the status bar is not rendered

### Requirement: File picker is shown inline as sidebar
In file-picker mode, the file picker SHALL be displayed as a persistent left sidebar panel, not as a Ctrl+P overlay. It SHALL enumerate git-tracked files from the project root.

#### Scenario: File picker visible on startup
- **WHEN** the app starts in file-picker mode
- **THEN** the file picker is immediately visible in the left sidebar with the search input focused

#### Scenario: File search filters results
- **WHEN** the user types in the file picker search input
- **THEN** the results list updates with fuzzy-matched files from the project

#### Scenario: File selection opens in code viewer
- **WHEN** the user selects a file from the picker results (click or Enter)
- **THEN** the file's source code is displayed in the central code viewer with syntax highlighting

### Requirement: Gutter breakpoints work without a debugger
In file-picker mode, the user SHALL be able to toggle breakpoints by clicking the code viewer gutter. Breakpoints SHALL be stored locally in UI state without requiring a debugger connection.

#### Scenario: Add breakpoint via gutter click
- **WHEN** the user clicks the gutter next to a line in the code viewer
- **THEN** a breakpoint is added for that file and line
- **THEN** the gutter shows a red indicator on that line

#### Scenario: Remove breakpoint via gutter click
- **WHEN** the user clicks the gutter on a line that already has a breakpoint
- **THEN** the breakpoint is removed
- **THEN** the red gutter indicator disappears

### Requirement: Breakpoints persist across sessions
Breakpoints set in file-picker mode SHALL be persisted to the state file so they are available in future sessions.

#### Scenario: Breakpoints saved on exit
- **WHEN** the user exits the app after setting breakpoints in file-picker mode
- **THEN** the breakpoints are saved to the persistent state file

#### Scenario: Breakpoints restored on next launch
- **WHEN** the user launches the app again in file-picker mode
- **THEN** previously set breakpoints are restored and shown in the gutter
