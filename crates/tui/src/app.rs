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
    /// Typing into file browser search in sidebar.
    FileBrowser,
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
    ) -> Self {
        Self {
            mode: AppMode::NoSession,
            focus: Focus::CodeView,
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
