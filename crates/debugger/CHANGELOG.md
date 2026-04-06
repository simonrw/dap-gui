# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/simonrw/dap-gui/releases/tag/dap-gui-debugger-v0.1.0) - 2026-04-05

### Added

- add Event::Error variant to surface background task failures

### Fixed

- rename workspace crates with dap-gui- prefix for cargo package compatibility ([#391](https://github.com/simonrw/dap-gui/pull/391))
- start DAP sequence numbers at 1 instead of 0
- replace panic with error propagation in stopped event handler

### Other

- add version fields to path dependencies for release-plz compatibility
- add 163 tests for tui and ui-core crates ([#388](https://github.com/simonrw/dap-gui/pull/388))
- scaffold TUI crate with ratatui (Phase 1)
- surface Output and Thread events from DAP
- add missing unit tests for error paths and scope changes
- move command-name mapping into RequestBody and simplify argument serialisation
- extract send_and_expect_success helper to eliminate boilerplate
- gate testing module behind cfg to exclude from release builds
- Deduplicate DAP session initialization with SessionArgs/StartMode enums
- Add file:line breakpoint input for remote attach workflows ([#386](https://github.com/simonrw/dap-gui/pull/386))
- Rename transport2 crate to async-transport
- Remove transport crate, sync debugger, and repl crate
- Replace hand-written DAP types in transport with auto-generated dap-types
- Move tokio to workspace dependency to fix minimal-versions CI
- Add 5 GUI improvements: breakpoints, variable tree, syntax highlighting, async migration, error handling
- Fix cargo fmt formatting across 7 files
- Robustness improvements for startup procedure
- Add more tests
- Fix minimal versions
- Error handling for bad responses
- Shortly hold lock
- Try to get implementation working
- Integrate launch configuration and staged debugger initialization
- Fix clippy lints
- Complete debugger transport integration phases ([#383](https://github.com/simonrw/dap-gui/pull/383))
- Clean out old POC crates
- Add testing infrastructure for async debugger (Phase 5) ([#382](https://github.com/simonrw/dap-gui/pull/382))
- Implement transport2 integration phases 1 and 2 ([#381](https://github.com/simonrw/dap-gui/pull/381))
- Add integration plan for debugger + transport2 ([#380](https://github.com/simonrw/dap-gui/pull/380))
- Fix try_poll_message to handle partial reads across timeouts ([#378](https://github.com/simonrw/dap-gui/pull/378))
- Add infrastructure for non-blocking request handling ([#375](https://github.com/simonrw/dap-gui/pull/375))
- Add refactoring plan for message receiving architecture ([#374](https://github.com/simonrw/dap-gui/pull/374))
- Switch debugger to use TransportConnection instead of Client ([#372](https://github.com/simonrw/dap-gui/pull/372))
- Migrate to latest iced version ([#373](https://github.com/simonrw/dap-gui/pull/373))
- Refactor debugger background thread to use command pattern ([#371](https://github.com/simonrw/dap-gui/pull/371))
- Add Command infrastructure for background thread communication ([#370](https://github.com/simonrw/dap-gui/pull/370))
- Add non-blocking event processing foundation ([#367](https://github.com/simonrw/dap-gui/pull/367))
- Fix stack frame handling panics in Stopped event ([#362](https://github.com/simonrw/dap-gui/pull/362))
- Fix home directory panic in path normalization ([#364](https://github.com/simonrw/dap-gui/pull/364))
- Fix runaway background thread termination ([#361](https://github.com/simonrw/dap-gui/pull/361))
- Fix race condition in test_remote_attach event handling ([#365](https://github.com/simonrw/dap-gui/pull/365))
- Fix poisoned mutex panic in with_internals ([#363](https://github.com/simonrw/dap-gui/pull/363))
- Fix panic in Drop implementation ([#360](https://github.com/simonrw/dap-gui/pull/360))
- Add 30-second timeout for DAP request responses ([#355](https://github.com/simonrw/dap-gui/pull/355))
- Ensure launch program exists
- Use mise and fix test issues
- Create crates/ subdir
