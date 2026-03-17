## ADDED Requirements

### Requirement: Lilex font is used for code display
The code view widget SHALL render source code using the Lilex font. The font SHALL be bundled within the application binary and loaded at startup. The Lilex font SHALL be registered as the egui `Monospace` font family.

#### Scenario: Code view renders with Lilex font
- **WHEN** the application starts and displays source code in the code view
- **THEN** the code view SHALL render text using the Lilex font

#### Scenario: Font is available without external files
- **WHEN** the application binary is distributed to a new machine
- **THEN** the Lilex font SHALL be available without requiring any external font files

### Requirement: Font loading works across all theme variants
The Lilex font SHALL be loaded in all egui-based GUI theme variants (base, catppuccin, dracula, material, nord, native). Font loading logic SHALL be shared via a common module to avoid duplication.

#### Scenario: Themed variant uses Lilex
- **WHEN** a user runs any egui-based theme variant of the application
- **THEN** the code view SHALL render using the Lilex font
