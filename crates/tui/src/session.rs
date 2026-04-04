use std::path::PathBuf;

use crossbeam_channel::Sender;
use debugger::{Breakpoint, Event};
use eyre::Context;
use launch_configuration::LaunchConfiguration;
use ui_core::session_bootstrap::connect_debugger;
use ui_core::session_state::SessionState;

use crate::async_bridge::AsyncBridge;

type StackFrameId = i64;

/// Re-export SessionState as DebuggerState for TUI compatibility.
pub(crate) type DebuggerState = SessionState;

/// An active debug session, bundling the async bridge and server process.
///
/// Dropping this struct tears down the session: the bridge command channel
/// closes, which causes the async runtime to shut down. The server process
/// (if any) is killed on drop.
pub(crate) struct Session {
    pub bridge: AsyncBridge,
    pub state: DebuggerState,
    pub current_frame_id: Option<StackFrameId>,
    /// Channel to receive debugger events. The bridge's event forwarding task
    /// sends events here; the app drains this each tick.
    pub debugger_event_rx: crossbeam_channel::Receiver<Event>,
    _server: Option<Box<dyn server::Server + Send>>,
}

impl Session {
    /// Start a new debug session from a launch configuration.
    ///
    /// This spawns the DAP server (for launch mode), connects to the debugger,
    /// configures breakpoints, and starts execution.
    ///
    /// The `wakeup_tx` channel is used to nudge the TUI event loop when a
    /// debugger event arrives, ensuring the UI redraws promptly.
    pub fn start(
        config: &LaunchConfiguration,
        breakpoints: &[Breakpoint],
        debug_root_dir: &mut PathBuf,
        wakeup_tx: Sender<()>,
    ) -> eyre::Result<Self> {
        let mut connected = connect_debugger(config, breakpoints, debug_root_dir)?;

        // Channel for debugger events: bridge -> app
        let (debugger_event_tx, debugger_event_rx) = crossbeam_channel::unbounded();
        let event_receiver = connected.debugger.take_events();

        let wakeup: Box<dyn Fn() + Send + Sync + 'static> = Box::new(move || {
            let _ = wakeup_tx.send(());
        });
        let bridge = AsyncBridge::spawn(
            connected.debugger,
            event_receiver,
            debugger_event_tx,
            wakeup,
            connected.runtime,
        )
        .context("creating async bridge")?;

        Ok(Self {
            bridge,
            state: DebuggerState::Running,
            current_frame_id: None,
            debugger_event_rx,
            _server: connected.server,
        })
    }

    /// Process a debugger event, updating session state.
    pub fn handle_event(&mut self, event: &Event) {
        let old_state = self.state.clone();
        self.state = old_state.apply(event);

        if let DebuggerState::Paused { paused_frame, .. } = &self.state {
            self.current_frame_id = Some(paused_frame.frame.id);
        } else if matches!(self.state, DebuggerState::Running) {
            self.current_frame_id = None;
        }
    }
}
