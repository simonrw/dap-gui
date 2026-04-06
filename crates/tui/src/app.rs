use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::async_bridge::UiCommand;
use crate::event::AppEvent;
use crate::session::Session;
use crossterm::event::KeyEvent;
use launch_configuration::LaunchConfiguration;
use state::StateManager;

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

/// Whether the app is currently capturing text input (suppresses normal keybindings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Normal keybindings active.
    Normal,
    /// Typing into file picker search.
    FilePicker,
    /// Typing into code view search.
    Search,
    /// Typing into breakpoint add input.
    BreakpointInput,
    /// Typing into evaluate expression popup.
    EvaluatePopup,
}

// ── Code view state ──────────────────────────��────────────────────────────

/// Re-export the shared code view state.
pub use ui_core::code_view::CodeViewState;

// ── Search state ──────────────────────────────────────────────────────────

/// State for in-file text search.
/// Re-export the shared search state.
pub use ui_core::search::{SearchMatch, SearchState};

// ── File picker state ─────────────────────────────────────────────────────

/// Re-export the shared file picker state.
pub use ui_core::file_picker::FilePickerState;

// ── Main app struct ───────────────────────────────────────────────────────

pub struct App {
    pub mode: AppMode,
    pub focus: Focus,
    pub bottom_tab: BottomTab,
    pub input_mode: InputMode,
    pub should_quit: bool,

    // Configuration
    pub configs: Vec<LaunchConfiguration>,
    pub config_names: Vec<String>,
    pub selected_config_index: usize,
    #[allow(dead_code)] // Used in Phase 5 for session restart
    pub config_path: PathBuf,
    pub debug_root_dir: PathBuf,

    // Session
    pub session: Option<Session>,
    /// Wakeup sender passed to new sessions so they can nudge the event loop.
    pub wakeup_tx: crossbeam_channel::Sender<()>,

    // UI breakpoints (survive across sessions)
    pub ui_breakpoints: HashSet<debugger::Breakpoint>,

    // Output buffer: (category, text)
    pub output_lines: Vec<(String, String)>,

    // Thread info: (thread_id, reason)
    pub threads: Vec<(i64, String)>,

    // Variables child cache: variables_reference -> children
    pub variables_cache: HashMap<i64, Vec<dap_types::Variable>>,
    pub variables_cursor: usize,

    // Call stack cursor
    pub call_stack_cursor: usize,

    // Persistence
    pub state_manager: StateManager,

    // File content cache: path -> content
    pub file_cache: ui_core::file_cache::FileCache,

    // Code view
    pub code_view: CodeViewState,

    // Search
    pub search: SearchState,

    // File picker
    pub file_picker: FilePickerState,

    // Help overlay
    pub show_help: bool,

    // Zen mode: hide all panels except code view and status bar
    pub zen_mode: bool,

    // REPL
    pub repl_input: String,
    pub repl_history: Vec<(String, String, bool)>, // (input, output, is_error)
    pub repl_input_history: Vec<String>,           // history of past inputs for Up/Down
    pub repl_history_cursor: Option<usize>,        // position in input history (None = new input)

    // Breakpoint panel input (for "a" add mode)
    pub breakpoint_input: Option<String>,

    // Breakpoint panel cursor
    pub breakpoints_cursor: usize,

    // Breakpoint IDs returned by the debugger (for removal)
    pub breakpoint_ids: HashMap<debugger::Breakpoint, u64>,

    // Output auto-scroll
    pub output_auto_scroll: bool,
    pub output_scroll_offset: usize,

    // Status line messages
    pub status_message: Option<String>,
    pub status_error: Option<String>,

    // File browser sidebar (no-session mode)
    pub file_browser_query: String,
    pub file_browser_cursor: usize,
    pub file_browser_results: Vec<fuzzy::FuzzyMatch>,
    pub file_browser_files: Vec<fuzzy::TrackedFile>,
    pub file_browser_loaded: bool,

    // Keybindings
    pub keybindings: config::keybindings::KeybindingConfig,

    // Evaluate expression popup
    pub evaluate_popup_open: bool,
    pub evaluate_input: String,
    pub evaluate_result: Option<(String, bool)>, // (result_text, is_error)

    // Inline evaluation annotations: line (0-indexed) -> result string
    pub inline_evaluations: HashMap<usize, String>,
}

impl App {
    pub fn new(
        configs: Vec<LaunchConfiguration>,
        config_names: Vec<String>,
        selected_config_index: usize,
        config_path: PathBuf,
        debug_root_dir: PathBuf,
        state_manager: StateManager,
        wakeup_tx: crossbeam_channel::Sender<()>,
        initial_breakpoints: Vec<debugger::Breakpoint>,
        keybindings: config::keybindings::KeybindingConfig,
    ) -> Self {
        Self {
            mode: AppMode::NoSession,
            focus: Focus::CallStack,
            bottom_tab: BottomTab::Variables,
            input_mode: InputMode::Normal,
            should_quit: false,
            configs,
            config_names,
            selected_config_index,
            config_path,
            debug_root_dir,
            session: None,
            wakeup_tx,
            ui_breakpoints: initial_breakpoints.into_iter().collect(),
            output_lines: Vec::new(),
            threads: Vec::new(),
            variables_cache: HashMap::new(),
            variables_cursor: 0,
            call_stack_cursor: 0,
            state_manager,
            file_cache: Default::default(),
            code_view: CodeViewState::default(),
            search: SearchState::default(),
            file_picker: FilePickerState::default(),
            show_help: false,
            zen_mode: false,
            repl_input: String::new(),
            repl_history: Vec::new(),
            repl_input_history: Vec::new(),
            repl_history_cursor: None,
            breakpoint_input: None,
            breakpoints_cursor: 0,
            breakpoint_ids: HashMap::new(),
            output_auto_scroll: true,
            output_scroll_offset: 0,
            status_message: None,
            status_error: None,
            file_browser_query: String::new(),
            file_browser_cursor: 0,
            file_browser_results: Vec::new(),
            file_browser_files: Vec::new(),
            file_browser_loaded: false,
            keybindings,
            evaluate_popup_open: false,
            evaluate_input: String::new(),
            evaluate_result: None,
            inline_evaluations: HashMap::new(),
        }
    }

    /// Process a single event.
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Resize(_, _) => {} // ratatui handles resize automatically
            AppEvent::Tick => self.drain_debugger_events(),
            AppEvent::Mouse(_) => {}
            AppEvent::Debugger(_) => {} // events arrive via session channel, drained on tick
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        crate::input::handle_key(self, key);
    }

    /// Drain all pending debugger events from the session's channel.
    pub fn drain_debugger_events(&mut self) {
        // Collect events and errors while briefly borrowing the session
        let (events, errors) = {
            let Some(session) = &mut self.session else {
                return;
            };
            let events: Vec<debugger::Event> = session.debugger_event_rx.try_iter().collect();
            let errors = session.bridge.drain_errors();
            (events, errors)
        };

        for err in errors {
            tracing::error!(operation = err.operation, error = %err.error, "bridge error");
            self.status_error = Some(format!("{}: {}", err.operation, err.error));
        }

        for event in &events {
            // Update session state (borrows self.session briefly)
            if let Some(session) = &mut self.session {
                session.handle_event(event);
            }

            // Handle event side effects (borrows self freely)
            match event {
                debugger::Event::Paused(state) => {
                    self.mode = AppMode::Paused;
                    self.ui_breakpoints = state.breakpoints.iter().cloned().collect();
                    self.variables_cache.clear();
                    self.inline_evaluations.clear();
                    self.jump_to_execution_line(state);
                }
                debugger::Event::ScopeChange(state) => {
                    self.mode = AppMode::Paused;
                    self.ui_breakpoints = state.breakpoints.iter().cloned().collect();
                    self.variables_cache.clear();
                    self.jump_to_execution_line(state);
                }
                debugger::Event::Running => {
                    self.mode = AppMode::Running;
                    self.inline_evaluations.clear();
                    self.status_message = Some("Running...".to_string());
                }
                debugger::Event::Initialised => {
                    self.mode = AppMode::Running;
                    self.status_message = Some("Debugger initialised".to_string());
                }
                debugger::Event::Ended => {
                    self.mode = AppMode::Terminated;
                    self.status_message = Some("Debugee terminated".to_string());
                }
                debugger::Event::Output { category, output } => {
                    self.output_lines.push((category.clone(), output.clone()));
                    // Ring buffer: keep max 10,000 lines
                    if self.output_lines.len() > crate::ui::output::MAX_OUTPUT_LINES {
                        let excess = self.output_lines.len() - crate::ui::output::MAX_OUTPUT_LINES;
                        self.output_lines.drain(..excess);
                    }
                }
                debugger::Event::Thread { reason, thread_id } => match reason.as_str() {
                    "started" => {
                        self.threads.push((*thread_id, reason.clone()));
                    }
                    "exited" => {
                        self.threads.retain(|(id, _)| *id != *thread_id);
                    }
                    _ => {}
                },
                debugger::Event::Error(msg) => {
                    tracing::error!(msg, "debugger error event");
                    self.status_error = Some(msg.clone());
                }
                debugger::Event::Uninitialised => {}
            }
        }
    }

    /// Jump code view to the current execution line.
    fn jump_to_execution_line(&mut self, state: &debugger::ProgramState) {
        let frame = &state.paused_frame.frame;
        if let Some(source) = &frame.source {
            if let Some(path) = &source.path {
                // Open the file if it's different from the current one
                if self.code_view.file_path.as_ref() != Some(path) {
                    self.open_file(path.clone());
                }
                // Jump cursor to execution line (DAP lines are 1-indexed)
                let line = (frame.line as usize).saturating_sub(1);
                self.code_view.cursor_line = line;
            }
        }
    }

    /// Start a debug session with the currently selected configuration.
    pub fn start_session(&mut self) {
        if self.session.is_some() {
            self.status_error = Some("Session already active".to_string());
            return;
        }

        if self.configs.is_empty() {
            self.status_error = Some("No launch configurations available".to_string());
            return;
        }

        let config = self.configs[self.selected_config_index].clone();
        let breakpoints: Vec<debugger::Breakpoint> = self.collect_all_breakpoints();

        // Persist selected config name
        let config_name = config.name().to_string();
        if let Err(e) = self.state_manager.set_last_selected_config(config_name) {
            tracing::warn!(error = %e, "failed to persist selected config");
        }

        self.mode = AppMode::Initialising;
        self.status_message = Some("Starting debug session...".to_string());
        self.status_error = None;
        self.output_lines.clear();
        self.threads.clear();
        self.variables_cache.clear();

        match Session::start(
            &config,
            &breakpoints,
            &mut self.debug_root_dir,
            self.wakeup_tx.clone(),
        ) {
            Ok(session) => {
                self.session = Some(session);
                self.mode = AppMode::Running;
                self.status_message = Some("Session started".to_string());
            }
            Err(e) => {
                self.mode = AppMode::NoSession;
                self.status_error = Some(format!("Failed to start session: {e}"));
                tracing::error!(error = %e, "failed to start debug session");
            }
        }
    }

    /// Stop the current debug session.
    pub fn stop_session(&mut self) {
        if let Some(session) = &self.session {
            session.bridge.send(UiCommand::Terminate);
        }
    }

    /// Shut down the current debug session and clean up.
    pub fn shutdown_session(&mut self) {
        if let Some(session) = &self.session {
            session.bridge.send(UiCommand::Shutdown);
        }
        self.session = None;
        self.mode = AppMode::NoSession;
        self.status_message = Some("Session ended".to_string());
    }

    /// Restart the debug session: stop the current session and start a new one.
    pub fn restart_session(&mut self) {
        if self.session.is_some() {
            // Terminate + shutdown the current session
            if let Some(session) = &self.session {
                session.bridge.send(UiCommand::Terminate);
            }
            // Drop session (sends shutdown implicitly)
            self.session = None;
            self.mode = AppMode::NoSession;
        }
        // Start a fresh session
        self.start_session();
    }

    /// Collect all breakpoints (UI + persisted) for session start.
    fn collect_all_breakpoints(&self) -> Vec<debugger::Breakpoint> {
        ui_core::breakpoints::collect_all_breakpoints(
            &self.state_manager,
            &self.debug_root_dir,
            &self.ui_breakpoints,
        )
    }

    /// Persist current breakpoints to state file.
    pub fn persist_breakpoints(&mut self) {
        ui_core::breakpoints::persist_breakpoints(
            &mut self.state_manager,
            &self.debug_root_dir,
            &self.ui_breakpoints,
        );
    }

    /// Open a file in the code view. Loads from cache or reads from disk.
    pub fn open_file(&mut self, path: PathBuf) {
        // Load content if not cached
        let Some(content) = self.file_cache.get_or_load(&path) else {
            return;
        };
        let total_lines = content.lines().count();
        self.code_view.open_file(path, total_lines);
        self.search.reset();
    }

    /// Get the current file content, if a file is open.
    pub fn current_file_content(&self) -> Option<&str> {
        let path = self.code_view.file_path.as_ref()?;
        self.file_cache.get(path)
    }

    // ── Breakpoint operations ─────────────────────────────────────────

    /// Toggle a breakpoint at the current cursor line in the code view.
    pub fn toggle_breakpoint_at_cursor(&mut self) {
        let Some(path) = self.code_view.file_path.clone() else {
            return;
        };
        let line = self.code_view.cursor_line + 1; // DAP is 1-indexed

        let bp = debugger::Breakpoint {
            name: None,
            path,
            line,
        };

        if self.ui_breakpoints.contains(&bp) {
            self.remove_breakpoint(&bp);
        } else {
            self.add_breakpoint(bp);
        }
    }

    /// Add a breakpoint (to UI set, to debugger if session active, and persist).
    pub fn add_breakpoint(&mut self, bp: debugger::Breakpoint) {
        self.ui_breakpoints.insert(bp.clone());

        if let Some(session) = &self.session {
            let result = session.bridge.send_sync(|reply| UiCommand::AddBreakpoint {
                breakpoint: bp.clone(),
                reply,
            });
            match result {
                Ok(id) => {
                    self.breakpoint_ids.insert(bp, id);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to add breakpoint to debugger");
                    self.status_error = Some(format!("Add breakpoint: {e}"));
                }
            }
        }

        self.persist_breakpoints();
    }

    /// Remove a breakpoint (from UI set, from debugger if session active, and persist).
    pub fn remove_breakpoint(&mut self, bp: &debugger::Breakpoint) {
        self.ui_breakpoints.remove(bp);

        if let Some(id) = self.breakpoint_ids.remove(bp) {
            if let Some(session) = &self.session {
                let result = session
                    .bridge
                    .send_sync(|reply| UiCommand::RemoveBreakpoint { id, reply });
                if let Err(e) = result {
                    tracing::warn!(error = %e, "failed to remove breakpoint from debugger");
                    self.status_error = Some(format!("Remove breakpoint: {e}"));
                }
            }
        }

        self.persist_breakpoints();
    }

    /// Add a breakpoint from a "file:line" string (used by breakpoint panel input).
    pub fn add_breakpoint_from_str(&mut self, input: &str) {
        match debugger::Breakpoint::parse(input, &self.debug_root_dir) {
            Ok(bp) => self.add_breakpoint(bp),
            Err(e) => {
                self.status_error = Some(format!("Invalid breakpoint: {e}"));
            }
        }
    }

    /// Remove the n-th breakpoint (sorted order) from the breakpoints panel.
    pub fn remove_breakpoint_by_index(&mut self, index: usize) {
        let mut bps: Vec<_> = self.ui_breakpoints.iter().cloned().collect();
        bps.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        if let Some(bp) = bps.get(index).cloned() {
            self.remove_breakpoint(&bp);
        }
    }

    /// Reconcile breakpoints with the debugger's authoritative set.
    /// Marks breakpoints the adapter rejected as unverified.
    #[allow(dead_code)] // Will be fully implemented when ListBreakpoints UiCommand is added
    pub fn reconcile_breakpoints(&mut self) {
        // TODO: send UiCommand::ListBreakpoints and diff against ui_breakpoints.
        // For now, we trust the debugger's Paused event breakpoints list.
    }

    // ── REPL operations ───────────────────────────────────────────────

    /// Evaluate the current REPL input and display the result.
    pub fn evaluate_repl(&mut self) {
        let input = self.repl_input.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Save to input history
        self.repl_input_history.push(input.clone());
        self.repl_history_cursor = None;

        let frame_id = self.session.as_ref().and_then(|s| s.current_frame_id);

        let Some(frame_id) = frame_id else {
            self.repl_history
                .push((input, "Not paused".to_string(), true));
            self.repl_input.clear();
            return;
        };

        let Some(session) = &self.session else {
            self.repl_history
                .push((input, "No session".to_string(), true));
            self.repl_input.clear();
            return;
        };

        let result = session.bridge.send_sync(|reply| UiCommand::Evaluate {
            expression: input.clone(),
            frame_id,
            reply,
        });

        match result {
            Ok(eval) => {
                self.repl_history.push((input, eval.output, eval.error));
            }
            Err(e) => {
                self.repl_history.push((input, format!("{e}"), true));
            }
        }

        self.repl_input.clear();
    }

    /// Navigate REPL input history upward.
    pub fn repl_history_up(&mut self) {
        if self.repl_input_history.is_empty() {
            return;
        }
        match self.repl_history_cursor {
            None => {
                self.repl_history_cursor = Some(self.repl_input_history.len() - 1);
                self.repl_input = self.repl_input_history.last().unwrap().clone();
            }
            Some(0) => {} // Already at oldest
            Some(ref mut idx) => {
                *idx -= 1;
                self.repl_input = self.repl_input_history[*idx].clone();
            }
        }
    }

    /// Navigate REPL input history downward.
    pub fn repl_history_down(&mut self) {
        match self.repl_history_cursor {
            None => {} // Not navigating
            Some(idx) if idx >= self.repl_input_history.len() - 1 => {
                self.repl_history_cursor = None;
                self.repl_input.clear();
            }
            Some(ref mut idx) => {
                *idx += 1;
                self.repl_input = self.repl_input_history[*idx].clone();
            }
        }
    }

    // ── Evaluate expression operations ────────────────────────────────

    /// Open the evaluate expression popup, optionally pre-filling with selection or cursor line.
    pub fn open_evaluate_popup(&mut self) {
        self.evaluate_popup_open = true;
        self.evaluate_result = None;
        self.input_mode = InputMode::EvaluatePopup;

        // Pre-fill with visual selection if active
        if self.focus == Focus::CodeView {
            if let Some((start, end)) = self.code_view.selection_range() {
                if let Some(content) = self.current_file_content() {
                    let text: String = content
                        .lines()
                        .skip(start)
                        .take(end - start + 1)
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.evaluate_input = text;
                    self.code_view.selection_anchor = None;
                    return;
                }
            }
            // Fall back to trimmed cursor line
            if let Some(word) = self.word_under_cursor() {
                self.evaluate_input = word;
                return;
            }
        }
        self.evaluate_input.clear();
    }

    /// Close the evaluate popup.
    pub fn close_evaluate_popup(&mut self) {
        self.evaluate_popup_open = false;
        self.evaluate_input.clear();
        self.evaluate_result = None;
        self.input_mode = InputMode::Normal;
    }

    /// Evaluate the expression currently in the evaluate popup input.
    pub fn evaluate_popup_expression(&mut self) {
        let input = self.evaluate_input.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Save to shared input history (reuse REPL history)
        self.repl_input_history.push(input.clone());
        self.repl_history_cursor = None;

        let frame_id = self.session.as_ref().and_then(|s| s.current_frame_id);

        let Some(frame_id) = frame_id else {
            self.evaluate_result = Some(("Not paused".to_string(), true));
            return;
        };

        let Some(session) = &self.session else {
            self.evaluate_result = Some(("No session".to_string(), true));
            return;
        };

        let result = session.bridge.send_sync(|reply| UiCommand::Evaluate {
            expression: input,
            frame_id,
            reply,
        });

        match result {
            Ok(eval) => {
                self.evaluate_result = Some((eval.output, eval.error));
            }
            Err(e) => {
                self.evaluate_result = Some((format!("{e}"), true));
            }
        }
    }

    /// Extract the trimmed content of the cursor line in the code view.
    fn word_under_cursor(&self) -> Option<String> {
        let content = self.current_file_content()?;
        let line = content.lines().nth(self.code_view.cursor_line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.to_string())
    }

    /// Evaluate the current cursor line (or selection) and show result as inline annotation.
    pub fn evaluate_inline(&mut self) {
        if self.mode != AppMode::Paused {
            return;
        }

        let (expression, target_line) = if let Some((start, end)) = self.code_view.selection_range()
        {
            // Evaluate selected lines
            if let Some(content) = self.current_file_content() {
                let text: String = content
                    .lines()
                    .skip(start)
                    .take(end - start + 1)
                    .collect::<Vec<_>>()
                    .join("\n");
                (text, end) // Annotate the last selected line
            } else {
                return;
            }
        } else {
            // Evaluate current cursor line (trimmed)
            if let Some(content) = self.current_file_content() {
                let line = content
                    .lines()
                    .nth(self.code_view.cursor_line)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                (line, self.code_view.cursor_line)
            } else {
                return;
            }
        };

        if expression.is_empty() {
            return;
        }

        let frame_id = self.session.as_ref().and_then(|s| s.current_frame_id);
        let Some(frame_id) = frame_id else {
            return;
        };
        let Some(session) = &self.session else {
            return;
        };

        let result = session.bridge.send_sync(|reply| UiCommand::Evaluate {
            expression,
            frame_id,
            reply,
        });

        match result {
            Ok(eval) => {
                let display = if eval.error {
                    format!("!! {}", eval.output)
                } else {
                    format!("= {}", eval.output)
                };
                self.inline_evaluations.insert(target_line, display);
            }
            Err(e) => {
                self.inline_evaluations
                    .insert(target_line, format!("!! {e}"));
            }
        }

        // Clear selection after evaluating
        self.code_view.selection_anchor = None;
    }

    // ── Variable operations ───────────────────────────────────────────

    /// Fetch child variables for an expandable variable.
    pub fn fetch_variables(&mut self, reference: i64) {
        if self.variables_cache.contains_key(&reference) {
            return; // Already cached
        }

        let Some(session) = &self.session else {
            return;
        };

        let result = session
            .bridge
            .send_sync(|reply| UiCommand::FetchVariables { reference, reply });

        match result {
            Ok(vars) => {
                self.variables_cache.insert(reference, vars);
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to fetch variables");
                self.status_error = Some(format!("Fetch variables: {e}"));
            }
        }
    }

    /// Copy a variable's value to the system clipboard via OSC 52 escape sequence.
    pub fn yank_variable_value(&self, value: &str) {
        use std::io::Write;
        let encoded = data_encoding::BASE64.encode(value.as_bytes());
        // OSC 52: \x1b]52;c;<base64>\x07
        let osc52 = format!("\x1b]52;c;{encoded}\x07");
        let _ = std::io::stdout().write_all(osc52.as_bytes());
        let _ = std::io::stdout().flush();
    }

    // ── File browser operations ──────────────────────────────────────

    /// Ensure file browser files are loaded (lazy).
    pub fn ensure_file_browser_loaded(&mut self) {
        if self.file_browser_loaded {
            return;
        }
        self.file_browser_loaded = true;
        if let Some(root) = fuzzy::find_repo_root() {
            match fuzzy::list_git_files(&root) {
                Ok(files) => self.file_browser_files = files,
                Err(e) => tracing::warn!(error = %e, "failed to list git files for browser"),
            }
        }
        self.refilter_file_browser();
    }

    /// Recompute file browser results from current query.
    pub fn refilter_file_browser(&mut self) {
        self.file_browser_results =
            fuzzy::fuzzy_filter(&self.file_browser_files, &self.file_browser_query);
        if self.file_browser_results.is_empty() {
            self.file_browser_cursor = 0;
        } else {
            self.file_browser_cursor = self
                .file_browser_cursor
                .min(self.file_browser_results.len() - 1);
        }
    }

    /// Select the current file browser item and open it in the code view.
    pub fn select_file_browser_item(&mut self) {
        if self.file_browser_results.is_empty() {
            return;
        }
        let path = self.file_browser_results[self.file_browser_cursor]
            .file
            .absolute_path
            .clone();
        self.open_file(path);
        self.focus = Focus::CodeView;
    }
}

// ── Test helpers ──────────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use std::path::PathBuf;

    /// Create a temporary `App` and run a closure against it.
    ///
    /// The closure receives `&mut App`. The `TempDir` that backs the
    /// `StateManager` stays alive for the duration of the closure, so all
    /// persistence operations work correctly.
    pub fn with_test_app(f: impl FnOnce(&mut App)) {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let state_path = dir.path().join("state.json");
        let state_manager = StateManager::new(&state_path).expect("failed to create StateManager");
        let (wakeup_tx, _wakeup_rx) = crossbeam_channel::unbounded();

        let mut app = App::new(
            vec![], // no configs
            vec![], // no config names
            0,
            PathBuf::from("/tmp/test"), // fake config_path
            PathBuf::from("/tmp/test"), // fake debug_root_dir
            state_manager,
            wakeup_tx,
            vec![], // no initial breakpoints
            Default::default(),
        );

        f(&mut app);
    }

    /// Create a temporary `App` with named configs.
    pub fn with_test_app_configs(config_names: Vec<String>, f: impl FnOnce(&mut App)) {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let state_path = dir.path().join("state.json");
        let state_manager = StateManager::new(&state_path).expect("failed to create StateManager");
        let (wakeup_tx, _wakeup_rx) = crossbeam_channel::unbounded();

        // Create dummy LaunchConfigurations from JSON for each name
        let configs: Vec<LaunchConfiguration> = config_names
            .iter()
            .map(|name| {
                let json = serde_json::json!({
                    "name": name,
                    "type": "python",
                    "request": "launch",
                    "program": "/tmp/test.py"
                });
                serde_json::from_value(json).expect("failed to create test config")
            })
            .collect();

        let mut app = App::new(
            configs,
            config_names,
            0,
            PathBuf::from("/tmp/test"),
            PathBuf::from("/tmp/test"),
            state_manager,
            wakeup_tx,
            vec![],
            Default::default(),
        );

        f(&mut app);
    }

    /// Create a temporary file with the given content and return its path.
    /// The `TempDir` is returned so the caller can keep it alive.
    pub fn write_temp_file(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let file_path = dir.path().join("test.py");
        std::fs::write(&file_path, content).expect("failed to write temp file");
        (dir, file_path)
    }

    /// Create a mock `Session` backed by channels.
    ///
    /// Returns the session and a sender for injecting debugger events.
    /// The `AsyncBridge` is a dummy that silently drops commands.
    pub fn mock_session() -> (Session, crossbeam_channel::Sender<debugger::Event>) {
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let (bridge, _error_tx) = crate::async_bridge::AsyncBridge::dummy();
        let session = Session::new_for_test(bridge, event_rx);
        (session, event_tx)
    }

    /// Helper to create a `ProgramState` for Paused/ScopeChange events.
    pub fn make_program_state(file: &str, line: usize) -> debugger::ProgramState {
        use dap_types::{Source, StackFrame};
        debugger::ProgramState {
            stack: vec![],
            breakpoints: vec![],
            paused_frame: debugger::PausedFrame {
                frame: StackFrame {
                    id: 1,
                    name: "test_fn".to_string(),
                    line,
                    column: 0,
                    source: Some(Source {
                        path: Some(PathBuf::from(file)),
                        name: None,
                        adapter_data: None,
                        checksums: None,
                        origin: None,
                        presentation_hint: None,
                        source_reference: None,
                        sources: None,
                    }),
                    can_restart: None,
                    end_column: None,
                    end_line: None,
                    instruction_pointer_reference: None,
                    module_id: None,
                    presentation_hint: None,
                },
                variables: vec![],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;

    // ── Focus cycling ─────────────────────────────────────────────────

    #[test]
    fn focus_next_cycles_through_all_panes() {
        let mut f = Focus::CodeView;
        f = f.next();
        assert_eq!(f, Focus::CallStack);
        f = f.next();
        assert_eq!(f, Focus::Breakpoints);
        f = f.next();
        assert_eq!(f, Focus::Variables);
        f = f.next();
        assert_eq!(f, Focus::Output);
        f = f.next();
        assert_eq!(f, Focus::Repl);
        f = f.next();
        assert_eq!(f, Focus::CodeView); // wraps
    }

    #[test]
    fn focus_prev_cycles_backwards() {
        let mut f = Focus::CodeView;
        f = f.prev();
        assert_eq!(f, Focus::Repl);
        f = f.prev();
        assert_eq!(f, Focus::Output);
        f = f.prev();
        assert_eq!(f, Focus::Variables);
        f = f.prev();
        assert_eq!(f, Focus::Breakpoints);
        f = f.prev();
        assert_eq!(f, Focus::CallStack);
        f = f.prev();
        assert_eq!(f, Focus::CodeView); // wraps
    }

    // ── App::open_file ────────────────────────────────────────────────

    #[test]
    fn open_file_loads_content_and_sets_code_view() {
        let (_dir, file_path) = write_temp_file("line1\nline2\nline3\n");

        with_test_app(|app| {
            app.open_file(file_path.clone());

            assert_eq!(app.code_view.file_path.as_ref(), Some(&file_path));
            assert_eq!(app.code_view.total_lines, 3);
            assert_eq!(app.code_view.cursor_line, 0);
            assert_eq!(app.code_view.scroll_offset, 0);
        });
    }

    #[test]
    fn open_file_resets_search_matches() {
        let (_dir, file_path) = write_temp_file("hello world\n");

        with_test_app(|app| {
            // Set up some search state with cached matches
            app.search.query = "hello".to_string();
            app.search.active = true;
            app.search.matches.push(ui_core::search::SearchMatch {
                line: 0,
                byte_start_in_line: 0,
                byte_offset: 0,
                length: 5,
            });
            app.search.current_match = 0;

            app.open_file(file_path);

            // Matches should be cleared (reset), but query/active are preserved
            assert!(app.search.matches.is_empty());
        });
    }

    #[test]
    fn open_file_nonexistent_is_noop() {
        with_test_app(|app| {
            app.open_file(PathBuf::from("/nonexistent/file.py"));

            assert!(app.code_view.file_path.is_none());
        });
    }

    #[test]
    fn current_file_content_returns_cached_content() {
        let (_dir, file_path) = write_temp_file("hello\nworld\n");

        with_test_app(|app| {
            assert!(app.current_file_content().is_none());

            app.open_file(file_path);

            let content = app.current_file_content().unwrap();
            assert_eq!(content, "hello\nworld\n");
        });
    }

    // ── Breakpoint operations ─────────────────────────────────────────

    #[test]
    fn toggle_breakpoint_adds_and_removes() {
        let (_dir, file_path) = write_temp_file("line1\nline2\nline3\n");

        with_test_app(|app| {
            app.open_file(file_path.clone());
            app.code_view.cursor_line = 1; // 0-indexed

            // Toggle on
            app.toggle_breakpoint_at_cursor();
            assert_eq!(app.ui_breakpoints.len(), 1);

            let bp = app.ui_breakpoints.iter().next().unwrap();
            assert_eq!(bp.path, file_path);
            assert_eq!(bp.line, 2); // DAP is 1-indexed

            // Toggle off
            app.toggle_breakpoint_at_cursor();
            assert!(app.ui_breakpoints.is_empty());
        });
    }

    #[test]
    fn toggle_breakpoint_without_file_is_noop() {
        with_test_app(|app| {
            app.toggle_breakpoint_at_cursor();
            assert!(app.ui_breakpoints.is_empty());
        });
    }

    #[test]
    fn add_breakpoint_from_str_valid() {
        with_test_app(|app| {
            app.add_breakpoint_from_str("/tmp/test.py:42");
            assert_eq!(app.ui_breakpoints.len(), 1);

            let bp = app.ui_breakpoints.iter().next().unwrap();
            assert_eq!(bp.path, PathBuf::from("/tmp/test.py"));
            assert_eq!(bp.line, 42);
        });
    }

    #[test]
    fn add_breakpoint_from_str_invalid_sets_error() {
        with_test_app(|app| {
            app.add_breakpoint_from_str("not_a_breakpoint");
            assert!(app.ui_breakpoints.is_empty());
            assert!(app.status_error.is_some());
            assert!(
                app.status_error
                    .as_ref()
                    .unwrap()
                    .contains("Invalid breakpoint")
            );
        });
    }

    #[test]
    fn remove_breakpoint_by_index_works() {
        with_test_app(|app| {
            // Add two breakpoints
            app.ui_breakpoints.insert(debugger::Breakpoint {
                name: None,
                path: PathBuf::from("/a.py"),
                line: 1,
            });
            app.ui_breakpoints.insert(debugger::Breakpoint {
                name: None,
                path: PathBuf::from("/b.py"),
                line: 2,
            });
            assert_eq!(app.ui_breakpoints.len(), 2);

            // Remove by index (sorted: /a.py:1, /b.py:2)
            app.remove_breakpoint_by_index(0);
            assert_eq!(app.ui_breakpoints.len(), 1);

            let remaining = app.ui_breakpoints.iter().next().unwrap();
            assert_eq!(remaining.path, PathBuf::from("/b.py"));
        });
    }

    // ── REPL history ──────────────────────────────────────────────────

    #[test]
    fn repl_history_up_and_down() {
        with_test_app(|app| {
            app.repl_input_history = vec![
                "expr1".to_string(),
                "expr2".to_string(),
                "expr3".to_string(),
            ];

            // Up from fresh (cursor = None)
            app.repl_history_up();
            assert_eq!(app.repl_input, "expr3");
            assert_eq!(app.repl_history_cursor, Some(2));

            app.repl_history_up();
            assert_eq!(app.repl_input, "expr2");
            assert_eq!(app.repl_history_cursor, Some(1));

            app.repl_history_up();
            assert_eq!(app.repl_input, "expr1");
            assert_eq!(app.repl_history_cursor, Some(0));

            // Already at oldest - stays
            app.repl_history_up();
            assert_eq!(app.repl_input, "expr1");
            assert_eq!(app.repl_history_cursor, Some(0));

            // Down
            app.repl_history_down();
            assert_eq!(app.repl_input, "expr2");
            assert_eq!(app.repl_history_cursor, Some(1));

            app.repl_history_down();
            assert_eq!(app.repl_input, "expr3");
            assert_eq!(app.repl_history_cursor, Some(2));

            // Down past end clears
            app.repl_history_down();
            assert!(app.repl_input.is_empty());
            assert_eq!(app.repl_history_cursor, None);
        });
    }

    #[test]
    fn repl_history_up_empty_is_noop() {
        with_test_app(|app| {
            app.repl_history_up();
            assert!(app.repl_input.is_empty());
            assert_eq!(app.repl_history_cursor, None);
        });
    }

    #[test]
    fn repl_history_down_from_fresh_is_noop() {
        with_test_app(|app| {
            app.repl_input_history = vec!["expr1".to_string()];
            // Without first going up, down is a no-op
            app.repl_history_down();
            assert!(app.repl_input.is_empty());
            assert_eq!(app.repl_history_cursor, None);
        });
    }

    // ── Start session edge cases ──────────────────────────────────────

    #[test]
    fn start_session_no_configs_sets_error() {
        with_test_app(|app| {
            app.start_session();
            assert_eq!(app.mode, AppMode::NoSession); // unchanged
            assert!(
                app.status_error
                    .as_ref()
                    .unwrap()
                    .contains("No launch configurations")
            );
        });
    }

    // ── Evaluate popup ────────────────────────────────────────────────

    #[test]
    fn open_evaluate_popup_sets_state() {
        with_test_app(|app| {
            app.open_evaluate_popup();
            assert!(app.evaluate_popup_open);
            assert_eq!(app.input_mode, InputMode::EvaluatePopup);
            assert!(app.evaluate_result.is_none());
        });
    }

    #[test]
    fn close_evaluate_popup_resets_state() {
        with_test_app(|app| {
            app.open_evaluate_popup();
            app.evaluate_input = "some_expr".to_string();
            app.evaluate_result = Some(("result".to_string(), false));

            app.close_evaluate_popup();

            assert!(!app.evaluate_popup_open);
            assert!(app.evaluate_input.is_empty());
            assert!(app.evaluate_result.is_none());
            assert_eq!(app.input_mode, InputMode::Normal);
        });
    }

    // ── Word under cursor ─────────────────────────────────────────────

    #[test]
    fn word_under_cursor_returns_trimmed_line() {
        let (_dir, file_path) = write_temp_file("  hello world  \n");

        with_test_app(|app| {
            app.open_file(file_path);
            app.code_view.cursor_line = 0;

            let word = app.word_under_cursor();
            assert_eq!(word.as_deref(), Some("hello world"));
        });
    }

    #[test]
    fn word_under_cursor_returns_none_for_empty_line() {
        let (_dir, file_path) = write_temp_file("  \nhello\n");

        with_test_app(|app| {
            app.open_file(file_path);
            app.code_view.cursor_line = 0;

            assert!(app.word_under_cursor().is_none());
        });
    }

    // ── Initial state ─────────────────────────────────────────────────

    #[test]
    fn app_initial_state() {
        with_test_app(|app| {
            assert_eq!(app.mode, AppMode::NoSession);
            assert_eq!(app.focus, Focus::CallStack);
            assert_eq!(app.bottom_tab, BottomTab::Variables);
            assert_eq!(app.input_mode, InputMode::Normal);
            assert!(!app.should_quit);
            assert!(app.session.is_none());
            assert!(app.ui_breakpoints.is_empty());
            assert!(app.output_lines.is_empty());
            assert!(!app.show_help);
            assert!(!app.zen_mode);
            assert!(app.repl_input.is_empty());
            assert!(app.output_auto_scroll);
        });
    }

    // ── Event draining integration tests ──────────────────────────────

    #[test]
    fn drain_paused_event_sets_mode_and_breakpoints() {
        let (_dir, file_path) = write_temp_file("line1\nline2\nline3\n");
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Running;

            let mut state = make_program_state(file_path.to_str().unwrap(), 2);
            state.breakpoints.push(debugger::Breakpoint {
                name: None,
                path: file_path.clone(),
                line: 1,
            });
            event_tx.send(debugger::Event::Paused(state)).unwrap();

            app.drain_debugger_events();

            assert_eq!(app.mode, AppMode::Paused);
            assert_eq!(app.ui_breakpoints.len(), 1);
            assert!(app.variables_cache.is_empty());
            assert!(app.inline_evaluations.is_empty());
            // Cursor should jump to execution line (line 2, 0-indexed = 1)
            assert_eq!(app.code_view.cursor_line, 1);
        });
    }

    #[test]
    fn drain_running_event_sets_mode_and_clears_inline_evals() {
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Paused;
            app.inline_evaluations.insert(0, "= 42".to_string());

            event_tx.send(debugger::Event::Running).unwrap();
            app.drain_debugger_events();

            assert_eq!(app.mode, AppMode::Running);
            assert!(app.inline_evaluations.is_empty());
            assert_eq!(app.status_message.as_deref(), Some("Running..."));
        });
    }

    #[test]
    fn drain_ended_event_sets_terminated_mode() {
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Running;

            event_tx.send(debugger::Event::Ended).unwrap();
            app.drain_debugger_events();

            assert_eq!(app.mode, AppMode::Terminated);
            assert_eq!(app.status_message.as_deref(), Some("Debugee terminated"));
        });
    }

    #[test]
    fn drain_initialised_event_sets_running_mode() {
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Initialising;

            event_tx.send(debugger::Event::Initialised).unwrap();
            app.drain_debugger_events();

            assert_eq!(app.mode, AppMode::Running);
            assert_eq!(app.status_message.as_deref(), Some("Debugger initialised"));
        });
    }

    #[test]
    fn drain_output_events_append_to_output_lines() {
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Running;

            event_tx
                .send(debugger::Event::Output {
                    category: "stdout".into(),
                    output: "Hello".into(),
                })
                .unwrap();
            event_tx
                .send(debugger::Event::Output {
                    category: "stderr".into(),
                    output: "Oops".into(),
                })
                .unwrap();
            app.drain_debugger_events();

            assert_eq!(app.output_lines.len(), 2);
            assert_eq!(
                app.output_lines[0],
                ("stdout".to_string(), "Hello".to_string())
            );
            assert_eq!(
                app.output_lines[1],
                ("stderr".to_string(), "Oops".to_string())
            );
        });
    }

    #[test]
    fn drain_thread_started_adds_thread() {
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Running;

            event_tx
                .send(debugger::Event::Thread {
                    reason: "started".into(),
                    thread_id: 42,
                })
                .unwrap();
            app.drain_debugger_events();

            assert_eq!(app.threads.len(), 1);
            assert_eq!(app.threads[0], (42, "started".to_string()));
        });
    }

    #[test]
    fn drain_thread_exited_removes_thread() {
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Running;
            app.threads.push((42, "started".to_string()));

            event_tx
                .send(debugger::Event::Thread {
                    reason: "exited".into(),
                    thread_id: 42,
                })
                .unwrap();
            app.drain_debugger_events();

            assert!(app.threads.is_empty());
        });
    }

    #[test]
    fn drain_error_event_sets_status_error() {
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Running;

            event_tx
                .send(debugger::Event::Error("something broke".into()))
                .unwrap();
            app.drain_debugger_events();

            assert_eq!(app.status_error.as_deref(), Some("something broke"));
        });
    }

    #[test]
    fn drain_scope_change_updates_mode_and_clears_variable_cache() {
        let (_dir, file_path) = write_temp_file("line1\nline2\nline3\n");
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Paused;
            app.variables_cache.insert(1, vec![]);

            let state = make_program_state(file_path.to_str().unwrap(), 3);
            event_tx.send(debugger::Event::ScopeChange(state)).unwrap();
            app.drain_debugger_events();

            assert_eq!(app.mode, AppMode::Paused);
            assert!(app.variables_cache.is_empty());
            assert_eq!(app.code_view.cursor_line, 2);
        });
    }

    #[test]
    fn drain_no_session_is_noop() {
        with_test_app(|app| {
            assert!(app.session.is_none());
            // Should not panic
            app.drain_debugger_events();
            assert_eq!(app.mode, AppMode::NoSession);
        });
    }

    #[test]
    fn drain_multiple_events_processes_all() {
        with_test_app(|app| {
            let (session, event_tx) = mock_session();
            app.session = Some(session);
            app.mode = AppMode::Initialising;

            // Send a sequence: Initialised -> Output -> Ended
            event_tx.send(debugger::Event::Initialised).unwrap();
            event_tx
                .send(debugger::Event::Output {
                    category: "stdout".into(),
                    output: "Done".into(),
                })
                .unwrap();
            event_tx.send(debugger::Event::Ended).unwrap();
            app.drain_debugger_events();

            // Final state should be Terminated (last event wins)
            assert_eq!(app.mode, AppMode::Terminated);
            assert_eq!(app.output_lines.len(), 1);
            assert_eq!(app.output_lines[0].1, "Done");
        });
    }

    // ── Breakpoint persistence tests ──────────────────────────────────

    #[test]
    fn persist_and_restore_breakpoints() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state_path = dir.path().join("state.json");
        let state_manager = StateManager::new(&state_path).expect("StateManager");
        let (wakeup_tx, _wakeup_rx) = crossbeam_channel::unbounded();

        let mut app = App::new(
            vec![],
            vec![],
            0,
            std::path::PathBuf::from("/home/user/project"),
            std::path::PathBuf::from("/home/user/project"),
            state_manager,
            wakeup_tx,
            vec![],
            Default::default(),
        );

        // Add a breakpoint and persist
        app.ui_breakpoints.insert(debugger::Breakpoint {
            name: None,
            path: std::path::PathBuf::from("/home/user/project/main.py"),
            line: 42,
        });
        app.persist_breakpoints();

        // Create a new app with the same state manager and collect breakpoints
        let state_manager2 = StateManager::new(&state_path).expect("StateManager2");
        let (wakeup_tx2, _) = crossbeam_channel::unbounded();
        let app2 = App::new(
            vec![],
            vec![],
            0,
            std::path::PathBuf::from("/home/user/project"),
            std::path::PathBuf::from("/home/user/project"),
            state_manager2,
            wakeup_tx2,
            vec![],
            Default::default(),
        );

        let restored = ui_core::breakpoints::collect_all_breakpoints(
            &app2.state_manager,
            &app2.debug_root_dir,
            &app2.ui_breakpoints,
        );
        assert_eq!(restored.len(), 1);
        assert_eq!(
            restored[0].path,
            std::path::PathBuf::from("/home/user/project/main.py")
        );
        assert_eq!(restored[0].line, 42);
    }

    #[test]
    fn collect_all_breakpoints_merges_ui_and_persisted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state_path = dir.path().join("state.json");
        let state_manager = StateManager::new(&state_path).expect("StateManager");
        let (wakeup_tx, _wakeup_rx) = crossbeam_channel::unbounded();

        // Persist a breakpoint via a first app instance
        let mut app1 = App::new(
            vec![],
            vec![],
            0,
            std::path::PathBuf::from("/home/user/project"),
            std::path::PathBuf::from("/home/user/project"),
            state_manager,
            wakeup_tx,
            vec![],
            Default::default(),
        );
        app1.ui_breakpoints.insert(debugger::Breakpoint {
            name: None,
            path: std::path::PathBuf::from("/home/user/project/a.py"),
            line: 10,
        });
        app1.persist_breakpoints();

        // Second app: different ui_breakpoints but same state file
        let state_manager2 = StateManager::new(&state_path).expect("StateManager2");
        let (wakeup_tx2, _) = crossbeam_channel::unbounded();
        let mut app2 = App::new(
            vec![],
            vec![],
            0,
            std::path::PathBuf::from("/home/user/project"),
            std::path::PathBuf::from("/home/user/project"),
            state_manager2,
            wakeup_tx2,
            vec![],
            Default::default(),
        );
        app2.ui_breakpoints.insert(debugger::Breakpoint {
            name: None,
            path: std::path::PathBuf::from("/home/user/project/b.py"),
            line: 20,
        });

        let all = ui_core::breakpoints::collect_all_breakpoints(
            &app2.state_manager,
            &app2.debug_root_dir,
            &app2.ui_breakpoints,
        );
        // Should contain both persisted (a.py:10) and UI (b.py:20)
        assert!(
            all.len() >= 2,
            "expected at least 2 breakpoints, got {}",
            all.len()
        );
        assert!(
            all.iter()
                .any(|bp| bp.path.ends_with("a.py") && bp.line == 10)
        );
        assert!(
            all.iter()
                .any(|bp| bp.path.ends_with("b.py") && bp.line == 20)
        );
    }
}
