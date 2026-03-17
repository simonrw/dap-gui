## Why

Currently, the only way to start a debug session is via CLI arguments (`--config`, `--name`, `--breakpoints`). There is no way to launch or re-launch execution from within the GUI itself. Once a session terminates, the user must restart the entire application. The file picker and code view already let users browse and set breakpoints, but there's no "Run" action to actually start debugging the program they're looking at. This makes the workflow clunky — users should be able to pick a launch configuration and start execution without leaving the GUI.

## What Changes

- Add a launch configuration selector UI that lets users pick from available configurations in their `launch.json`
- Add a "Run" / "Start Debugging" action accessible from the toolbar and via keyboard shortcut
- Support re-launching a debug session after termination without restarting the app
- Wire up the full launch/attach flow (server spawn, connect, configure breakpoints, start) to be triggerable from the GUI
- Use the currently viewed file to resolve `${file}` placeholders in launch configurations

## Capabilities

### New Capabilities
- `launch-from-gui`: Ability to select a launch configuration and start/restart a debug session from within the GUI, including configuration selection, server lifecycle management, and session restart after termination.

### Modified Capabilities

## Impact

- **`crates/gui/src/main.rs`**: State machine needs to support transitions back from `Terminated` to `Initialising`/`Running`. The initialization logic currently in `main()` needs to be extractable and re-invocable.
- **`crates/gui/src/renderer.rs`**: New toolbar UI for configuration selection and launch button. Terminated state needs a "restart" affordance instead of just showing "Program terminated".
- **`crates/gui/src/async_bridge.rs`**: New commands for launching/attaching a session, and potentially tearing down an existing one.
- **`crates/server/`**: Server lifecycle needs to be managed by the GUI rather than just spawned once at startup.
- **`crates/launch_configuration/`**: Configuration loading needs to be accessible from the GUI layer, not just at CLI parse time.
