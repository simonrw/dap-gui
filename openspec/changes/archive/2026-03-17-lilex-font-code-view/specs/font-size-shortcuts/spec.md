## ADDED Requirements

### Requirement: Increase font size shortcut
The application SHALL provide a keyboard shortcut `Ctrl + =` (or `Cmd + =` on macOS) to increase the code view font size by 1.0 point.

#### Scenario: User increases font size
- **WHEN** the user presses `Ctrl + =` (or `Cmd + =` on macOS)
- **THEN** the code view font size SHALL increase by 1.0 point

#### Scenario: Font size does not exceed maximum
- **WHEN** the code view font size is at the maximum (32.0 points) and the user presses the increase shortcut
- **THEN** the font size SHALL remain at 32.0 points

### Requirement: Decrease font size shortcut
The application SHALL provide a keyboard shortcut `Ctrl + -` (or `Cmd + -` on macOS) to decrease the code view font size by 1.0 point.

#### Scenario: User decreases font size
- **WHEN** the user presses `Ctrl + -` (or `Cmd + -` on macOS)
- **THEN** the code view font size SHALL decrease by 1.0 point

#### Scenario: Font size does not go below minimum
- **WHEN** the code view font size is at the minimum (8.0 points) and the user presses the decrease shortcut
- **THEN** the font size SHALL remain at 8.0 points

### Requirement: Font size is persisted across sessions
The user's chosen code view font size SHALL be saved and restored when the application is restarted. The default font size SHALL be 14.0 points.

#### Scenario: Font size survives restart
- **WHEN** the user changes the font size and restarts the application
- **THEN** the code view SHALL use the previously set font size

#### Scenario: Default font size on first launch
- **WHEN** the application is launched for the first time with no saved preferences
- **THEN** the code view font size SHALL be 14.0 points

### Requirement: Font size only affects code view
Font size changes via keyboard shortcuts SHALL only affect the code view widget. Other UI elements (panels, buttons, labels) SHALL remain at their default sizes.

#### Scenario: UI elements unaffected by font size change
- **WHEN** the user increases or decreases the code view font size
- **THEN** all non-code-view UI elements SHALL retain their original font sizes
