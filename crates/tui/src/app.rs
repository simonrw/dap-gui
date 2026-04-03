use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use launch_configuration::LaunchConfiguration;

use crate::event::AppEvent;

/// The current mode of the application.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Variants used as phases are implemented
pub enum AppMode {
    /// No debug session is active.
    NoSession,
    /// A debug session is starting up.
    Initialising,
    /// The debugee is running.
    Running,
    /// The debugee is paused at a breakpoint or step.
    Paused,
    /// The debugee has terminated.
    Terminated,
}

/// Which pane currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    CodeView,
    CallStack,
    Breakpoints,
    Variables,
    Output,
    Repl,
}

impl Focus {
    const ORDER: &[Focus] = &[
        Focus::CodeView,
        Focus::CallStack,
        Focus::Breakpoints,
        Focus::Variables,
        Focus::Output,
        Focus::Repl,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ORDER.iter().position(|&f| f == self).unwrap_or(0);
        Self::ORDER[(idx + 1) % Self::ORDER.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ORDER.iter().position(|&f| f == self).unwrap_or(0);
        Self::ORDER[(idx + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// Which tab is visible in the bottom panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Variables,
    Output,
    Repl,
}

#[allow(dead_code)] // Fields used as phases are implemented
pub struct App {
    pub mode: AppMode,
    pub focus: Focus,
    pub bottom_tab: BottomTab,
    pub should_quit: bool,

    // Configuration
    pub configs: Vec<LaunchConfiguration>,
    pub config_names: Vec<String>,
    pub selected_config_index: usize,
    pub config_path: PathBuf,
    pub debug_root_dir: PathBuf,
}

impl App {
    pub fn new(
        configs: Vec<LaunchConfiguration>,
        config_names: Vec<String>,
        selected_config_index: usize,
        config_path: PathBuf,
        debug_root_dir: PathBuf,
    ) -> Self {
        Self {
            mode: AppMode::NoSession,
            focus: Focus::CodeView,
            bottom_tab: BottomTab::Variables,
            should_quit: false,
            configs,
            config_names,
            selected_config_index,
            config_path,
            debug_root_dir,
        }
    }

    /// Process a single event. Returns after the event is handled.
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Resize(_, _) => {} // ratatui handles resize automatically
            AppEvent::Tick => {}         // triggers a redraw
            AppEvent::Mouse(_) => {}     // mouse support later
            AppEvent::Debugger(_) => {}  // wired up in Phase 3
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            // Quit
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            // Focus cycling
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.focus = self.focus.prev();
            }
            KeyCode::Tab => {
                self.focus = self.focus.next();
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev();
            }
            // Bottom tab switching
            KeyCode::Char('1') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.bottom_tab = BottomTab::Variables;
            }
            KeyCode::Char('2') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.bottom_tab = BottomTab::Output;
            }
            KeyCode::Char('3') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.bottom_tab = BottomTab::Repl;
            }
            _ => {}
        }
    }
}
