# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/simonrw/dap-gui/releases/tag/dap-gui-egui-v0.1.0) - 2026-04-05

### Added

- add Event::Error variant to surface background task failures

### Fixed

- rename workspace crates with dap-gui- prefix for cargo package compatibility ([#391](https://github.com/simonrw/dap-gui/pull/391))

### Other

- add version fields to path dependencies for release-plz compatibility
- extract shared ui-core crate from gui and tui ([#387](https://github.com/simonrw/dap-gui/pull/387))
- surface Output and Thread events from DAP
- Deduplicate DAP session initialization with SessionArgs/StartMode enums
- Add always-visible text search to no-session code viewer
- Merge file browser into main app's no-session view
- Add Cmd+F / Ctrl+F text search in code view
- Add GUI launch controls: start, stop, and restart debug sessions
- Add Lilex font for code view with Cmd+/- font size shortcuts
- Add file-picker startup mode when no breakpoints provided
- Add file:line breakpoint input for remote attach workflows ([#386](https://github.com/simonrw/dap-gui/pull/386))
- Rename transport2 crate to async-transport
- Remove transport crate, sync debugger, and repl crate
- Persist breakpoints to disk when modified via gutter clicks
- fmt
- Move tokio to workspace dependency to fix minimal-versions CI
- Fix breakpoint rendering
- Canonicalize all paths early to fix breakpoint gutter display
- Make code window fill full width of the code panel
- Fix breakpoint toggle: persist UI breakpoints across frames and match by path+line
- Fix code panel scroll stability: use viewport height and only scroll when needed
- Fix debug server lifetime: keep server handle alive for app duration
- Move CentralPanel into renderer and add status messages for empty states
- Add 5 GUI improvements: breakpoints, variable tree, syntax highlighting, async migration, error handling
- Fix cargo fmt formatting across 7 files
- Extract fuzzy crate and add file picker to egui GUI
- Robustness improvements for startup procedure
- Fix variable parsing
- Update controls
- Migrate to latest iced version ([#373](https://github.com/simonrw/dap-gui/pull/373))
- Bump versions
- Formatting
- Create crates/ subdir
