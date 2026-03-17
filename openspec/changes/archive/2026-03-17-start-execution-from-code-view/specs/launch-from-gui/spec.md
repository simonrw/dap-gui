## ADDED Requirements

### Requirement: User can start a debug session from the GUI
The system SHALL allow users to start a debug session by selecting a launch configuration and clicking a "Start" button in the toolbar, or pressing F5.

#### Scenario: Start session with selected configuration
- **WHEN** no debug session is active and the user selects a configuration from the dropdown and clicks "Start" (or presses F5)
- **THEN** the system SHALL spawn the debug adapter server (if launch mode), connect to the debugger, configure persisted breakpoints, and begin execution

#### Scenario: Start button disabled during active session
- **WHEN** a debug session is already active (Initialising, Running, or Paused state)
- **THEN** the "Start" button SHALL be disabled and the configuration dropdown SHALL be read-only

### Requirement: User can stop an active debug session
The system SHALL allow users to stop the current debug session via a "Stop" button or Shift+F5 keyboard shortcut.

#### Scenario: Stop a running session
- **WHEN** a debug session is active and the user clicks "Stop" or presses Shift+F5
- **THEN** the system SHALL terminate the debug adapter connection, kill the server process, and transition to the no-session state

#### Scenario: Stop not available when no session
- **WHEN** no debug session is active
- **THEN** the "Stop" button SHALL be disabled

### Requirement: User can restart a session after termination
The system SHALL allow users to start a new debug session after the previous one has terminated, without restarting the application.

#### Scenario: Restart after program termination
- **WHEN** the debugged program terminates (State::Terminated) and the user clicks "Start" or presses F5
- **THEN** the system SHALL tear down the old session, start a new debug session with the selected configuration, and preserve the user's breakpoints

#### Scenario: Breakpoints preserved across restarts
- **WHEN** a new session is started after a previous session ended
- **THEN** all breakpoints from the previous session SHALL be configured in the new session

### Requirement: Launch configuration selector in toolbar
The system SHALL display a dropdown/combo box in the top toolbar showing all available launch configurations parsed from the `launch.json` file.

#### Scenario: Multiple configurations available
- **WHEN** the `launch.json` contains multiple configurations
- **THEN** the dropdown SHALL list all configurations by name and the first one SHALL be selected by default

#### Scenario: Single configuration available
- **WHEN** the `launch.json` contains exactly one configuration
- **THEN** that configuration SHALL be pre-selected and the dropdown SHALL still be shown

#### Scenario: CLI-specified name pre-selects configuration
- **WHEN** the user provides `--name` on the CLI
- **THEN** the matching configuration SHALL be pre-selected in the dropdown

### Requirement: Stepping controls reflect session state
The system SHALL show or hide stepping controls (Continue, Step Over, Step Into, Step Out) based on whether a debug session is active.

#### Scenario: No active session
- **WHEN** no debug session is active
- **THEN** stepping controls (Continue, Step Over, Step Into, Step Out) SHALL NOT be displayed

#### Scenario: Active session
- **WHEN** a debug session is active
- **THEN** stepping controls SHALL be displayed as they are today

### Requirement: F5 keyboard shortcut for start/continue
The system SHALL use the F5 key as a dual-purpose shortcut: starting a new session when none is active, or continuing execution when paused.

#### Scenario: F5 with no session
- **WHEN** no debug session is active and the user presses F5
- **THEN** the system SHALL start a new session with the currently selected configuration

#### Scenario: F5 when paused
- **WHEN** the debug session is paused and the user presses F5
- **THEN** the system SHALL send a Continue command to the debugger

#### Scenario: F5 when running
- **WHEN** the debug session is running (not paused) and the user presses F5
- **THEN** the system SHALL ignore the keypress (no action)

### Requirement: No-session welcome view
The system SHALL display a welcome/launch view when no debug session is active, replacing the current "Initialising debugger..." screen.

#### Scenario: App starts without auto-launch
- **WHEN** the application starts and no `--name` flag was provided to auto-select a configuration
- **THEN** the system SHALL display the configuration selector and start button prominently in the central area

#### Scenario: App starts with auto-launch
- **WHEN** the application starts with a `--name` flag that matches exactly one configuration
- **THEN** the system SHALL auto-start the session (preserving current CLI behavior)
