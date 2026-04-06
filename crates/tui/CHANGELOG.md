# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/simonrw/dap-gui/releases/tag/dap-tui-v0.1.0) - 2026-04-05

### Fixed

- rename workspace crates with dap-gui- prefix for cargo package compatibility ([#391](https://github.com/simonrw/dap-gui/pull/391))

### Other

- rename main crate to dap-tui again
- add version fields to path dependencies for release-plz compatibility
- set up release pipeline with release-plz and GitHub Actions ([#389](https://github.com/simonrw/dap-gui/pull/389))
- add zen mode to maximize code view
- add 163 tests for tui and ui-core crates ([#388](https://github.com/simonrw/dap-gui/pull/388))
- extract shared ui-core crate from gui and tui ([#387](https://github.com/simonrw/dap-gui/pull/387))
- add inline evaluation annotations in code view (Phase C)
- add visual line selection in code view (Phase B)
- add Ctrl+E evaluate expression popup (Phase A)
- remove modal REPL input, type directly when focused
- no-session polish and robustness (Phase 5)
- interactive breakpoints, REPL, and variable tree (Phase 4)
- connect to debugger and hit breakpoints (Phase 3)
- file browsing with syntax highlighting (Phase 2)
- scaffold TUI crate with ratatui (Phase 1)
- Clean out old POC crates
- Create crates/ subdir
