# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/simonrw/dap-gui/compare/dap-gui-ui-core-v0.1.0...dap-gui-ui-core-v0.1.1) - 2026-04-07

### Added

- add automatic dark/light theme switching based on system preference ([#397](https://github.com/simonrw/dap-gui/pull/397))
- add log rotation, non-blocking writes, and CLI log flags ([#398](https://github.com/simonrw/dap-gui/pull/398))
- use launch config cwd and show file browser overflow ([#396](https://github.com/simonrw/dap-gui/pull/396))
- add configurable keybindings with TOML config file ([#393](https://github.com/simonrw/dap-gui/pull/393))

## [0.1.0](https://github.com/simonrw/dap-gui/releases/tag/dap-gui-ui-core-v0.1.0) - 2026-04-05

### Added

- make config_path optional, defaulting to .vscode/launch.json
- add user-facing documentation to CLI argument parser ([#390](https://github.com/simonrw/dap-gui/pull/390))

### Fixed

- rename workspace crates with dap-gui- prefix for cargo package compatibility ([#391](https://github.com/simonrw/dap-gui/pull/391))

### Other

- add version fields to path dependencies for release-plz compatibility
- add 163 tests for tui and ui-core crates ([#388](https://github.com/simonrw/dap-gui/pull/388))
- extract shared ui-core crate from gui and tui ([#387](https://github.com/simonrw/dap-gui/pull/387))
