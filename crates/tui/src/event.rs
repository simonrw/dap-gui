use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{self, Event, KeyEvent, MouseEvent};

/// All events the application loop can receive.
#[derive(Debug)]
#[allow(dead_code)] // Variants used as phases are implemented
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
}

/// Background event handler that multiplexes terminal input, debugger events,
/// and periodic ticks into a single channel.
pub struct EventHandler {
    rx: Receiver<AppEvent>,
    // Keep the handle so the thread is joined on drop.
    _thread: std::thread::JoinHandle<()>,
}

impl EventHandler {
    /// Spawn the event handler.
    ///
    /// `debugger_rx` is `None` initially (no session) and can be connected
    /// later by sending debugger events directly into the returned sender.
    pub fn new(tick_rate: Duration) -> (Self, Sender<AppEvent>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let event_tx = tx.clone();

        let thread = std::thread::Builder::new()
            .name("tui-event-handler".into())
            .spawn(move || {
                loop {
                    // Poll terminal events with the tick_rate as timeout.
                    let has_event = event::poll(tick_rate).unwrap_or(false);
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
                    } else {
                        // No terminal event within tick_rate — emit a tick.
                        if event_tx.send(AppEvent::Tick).is_err() {
                            break;
                        }
                    }
                }
            })
            .expect("failed to spawn event handler thread");

        let handler = Self {
            rx,
            _thread: thread,
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
