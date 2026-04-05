# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/simonrw/dap-gui/releases/tag/dap-gui-pcaplog-v0.1.0) - 2026-04-05

### Fixed

- rename workspace crates with dap-gui- prefix for cargo package compatibility ([#391](https://github.com/simonrw/dap-gui/pull/391))

### Other

- add version fields to path dependencies for release-plz compatibility
- Move bytes to workspace dependency to fix minimal-versions CI
- Rename transport2 crate to async-transport
- Remove transport crate, sync debugger, and repl crate
- Use mise and fix test issues
- Create crates/ subdir
