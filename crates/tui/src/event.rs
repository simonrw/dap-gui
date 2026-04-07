use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{self, Event, KeyEvent, MouseEvent};

use crate::theme::ThemeMode;

/// All events the application loop can receive.
#[derive(Debug)]
#[allow(dead_code)] // Variants/fields used as phases are implemented
#[allow(clippy::large_enum_variant)]
pub enum AppEvent {
    /// A key press from the terminal.
    Key(KeyEvent),
    /// A mouse event from the terminal.
    Mouse(MouseEvent),
    /// The terminal was resized.
    Resize(u16, u16),
    /// A debugger event from the async bridge.
    Debugger(debugger::Event),
    /// Periodic tick for UI refresh (cursor blink, status expiry, etc.).
    Tick,
    /// The system color scheme changed.
    ThemeChanged(ThemeMode),
}

/// Background event handler that multiplexes terminal input, debugger events,
/// and periodic ticks into a single channel.
pub struct EventHandler {
    rx: Receiver<AppEvent>,
    // Keep handles so threads are joined on drop.
    _thread: std::thread::JoinHandle<()>,
    _theme_thread: Option<std::thread::JoinHandle<()>>,
}

impl EventHandler {
    /// Spawn the event handler.
    ///
    /// `wakeup_rx` receives notifications from the async bridge when debugger
    /// events are available. This unblocks the poll wait so the TUI redraws
    /// promptly.
    ///
    /// When `theme_preference` is `Auto`, a background thread periodically
    /// polls `dark_light::detect()` and sends `ThemeChanged` events when the
    /// system color scheme changes.
    pub fn new(
        tick_rate: Duration,
        wakeup_rx: Receiver<()>,
        theme_preference: config::ThemePreference,
        initial_mode: ThemeMode,
    ) -> (Self, Sender<AppEvent>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let event_tx = tx.clone();

        let thread = std::thread::Builder::new()
            .name("tui-event-handler".into())
            .spawn(move || {
                loop {
                    // Use a short poll timeout so we can check the wakeup channel.
                    // This is a compromise: short enough to react to debugger events
                    // promptly, long enough to not burn CPU.
                    let poll_timeout = tick_rate;

                    let has_event = event::poll(poll_timeout).unwrap_or(false);
                    if has_event {
                        match event::read() {
                            Ok(Event::Key(key)) => {
                                if event_tx.send(AppEvent::Key(key)).is_err() {
                                    break;
                                }
                            }
                            Ok(Event::Mouse(mouse)) => {
                                if event_tx.send(AppEvent::Mouse(mouse)).is_err() {
                                    break;
                                }
                            }
                            Ok(Event::Resize(w, h)) => {
                                if event_tx.send(AppEvent::Resize(w, h)).is_err() {
                                    break;
                                }
                            }
                            Ok(_) => {} // FocusGained, FocusLost, Paste
                            Err(_) => break,
                        }
                    }

                    // Check if there are wakeup notifications (debugger events ready).
                    // Drain all pending wakeups and send a single Tick to trigger redraw.
                    let mut woke = false;
                    while wakeup_rx.try_recv().is_ok() {
                        woke = true;
                    }

                    if woke || !has_event {
                        // Either a debugger event arrived or the poll timed out.
                        // Send a tick to trigger event draining and redraw.
                        if event_tx.send(AppEvent::Tick).is_err() {
                            break;
                        }
                    }
                }
            })
            .expect("failed to spawn event handler thread");

        // Spawn a separate thread for theme detection so the blocking D-Bus
        // call does not interfere with terminal event polling.
        tracing::warn!(?theme_preference, "read theme preference");
        let theme_thread = if theme_preference == config::ThemePreference::Auto {
            let theme_tx = tx.clone();
            Some(
                std::thread::Builder::new()
                    .name("tui-theme-watcher".into())
                    .spawn(move || {
                        tracing::warn!("spawning theme watcher thread");
                        let mut current = initial_mode;
                        loop {
                            std::thread::sleep(Duration::from_secs(2));
                            let detected = crate::theme::detect_theme_mode();
                            tracing::warn!(?detected, "detected current theme");
                            if detected != current {
                                tracing::warn!(?current, ?detected, "switching themes");
                                current = detected;
                                if theme_tx.send(AppEvent::ThemeChanged(detected)).is_err() {
                                    break;
                                }
                            }
                        }
                    })
                    .expect("failed to spawn theme watcher thread"),
            )
        } else {
            None
        };

        let handler = Self {
            rx,
            _thread: thread,
            _theme_thread: theme_thread,
        };
        (handler, tx)
    }

    /// Receive the next event (blocking).
    pub fn recv(&self) -> eyre::Result<AppEvent> {
        self.rx
            .recv()
            .map_err(|_| eyre::eyre!("event channel closed"))
    }
}
